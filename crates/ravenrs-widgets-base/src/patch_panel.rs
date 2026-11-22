//! [`PatchPanel`]: a panel of jacks where cables are plugged and unplugged with the
//! pointer or the keys, as on the back of an amplifier, a rack of effects, a modular
//! synthesizer or an old telephone switchboard (#578).
//!
//! ```ignore
//! let links = cx.use_state(Vec::<PatchLink>::new);
//! cx.child(
//!     &PatchPanel::new(
//!         vec![
//!             Jack::output("out", "Saída L").signal("áudio").at(40.0, 40.0),
//!             Jack::input("in", "Entrada 1").signal("áudio").at(40.0, 200.0),
//!         ],
//!         links.binding(),
//!     )
//!     .on_connect(|link| println!("{} → {}", link.from, link.to)),
//! );
//! ```
//!
//! The cables are [`CablePlug`]s: they hang, swing and settle, the one in the hand
//! becomes the plugged one without a jump, and the panel asks for frames only while some
//! cable moves. A cable goes from an output to an input of the same signal, one to an
//! input; an output takes many. Dragging the plug of a plugged input takes it out to plug
//! it elsewhere, and letting it go on the panel's empty space unplugs it. With the keys,
//! a jack picks up a cable with Space, the arrows go to another jack and Enter plugs it
//! there; Delete unplugs the jack's cables and Ctrl+Z undoes.

use std::collections::HashMap;
use std::rc::Rc;

use ravenrs_core::cable::{CableRelease, CableTexture};
use ravenrs_core::{Border, Color, Key, Modifiers, Point, Role};
use ravenrs_layout::{Axis, LayoutStyle};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::TextStyle;
use ravenrs_text::TextAlign;

use crate::cable_plug::{CablePlug, CableSide};
use crate::context::{BuildCx, DragPayload, NodeId, Paint, QuadPaint, Semantics};
use crate::{Button, ButtonStyle, Text, View};

/// Which way cables go through a [`Jack`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum JackFlow {
    /// Cables leave it: one jack feeds many.
    Output,
    /// Cables arrive at it: one at a time.
    Input,
}

/// One jack of a [`PatchPanel`].
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Jack {
    /// Its name for the app, unique in the panel.
    pub id: String,
    /// What people read under it.
    pub label: String,
    /// Output or input.
    pub flow: JackFlow,
    /// What it carries; a cable joins jacks of the same signal. Empty takes any.
    pub signal: String,
    /// Its middle, in the panel.
    pub at: Point,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Jack {
    /// An output called `label`.
    pub fn output(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, label, JackFlow::Output)
    }

    /// An input called `label`.
    pub fn input(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(id, label, JackFlow::Input)
    }

    fn new(id: impl Into<String>, label: impl Into<String>, flow: JackFlow) -> Self {
        Self { id: id.into(), label: label.into(), flow, signal: String::new(), at: Point::ZERO }
    }

    /// What it carries, such as "áudio" or "controle".
    pub fn signal(mut self, signal: impl Into<String>) -> Self {
        self.signal = signal.into();
        self
    }

    /// Where its middle is, in the panel.
    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.at = Point::new(x, y);
        self
    }
}

/// A cable plugged from the output `from` to the input `to`, by their ids.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct PatchLink {
    /// The output.
    pub from: String,
    /// The input.
    pub to: String,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl PatchLink {
    /// The cable from `from` to `to`.
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self { from: from.into(), to: to.into() }
    }
}

type Rule = Rc<dyn Fn(&Jack, &Jack) -> Result<(), String>>;
type LinkHandler = Rc<dyn Fn(&PatchLink)>;
type JoinHandler = Rc<dyn Fn(&str, &str)>;
type Painter = Rc<dyn Fn(&str) -> Color>;
type Covering = Rc<dyn Fn(&str) -> CableTexture>;

/// What is carried while a cable is in the hand: the jack it hangs from and, when it was
/// taken out of an input, the link it belonged to.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pull {
    from: usize,
    link: Option<usize>,
}

/// A panel of jacks joined by cables. See the [module docs](self).
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct PatchPanel {
    jacks: Rc<Vec<Jack>>,
    links: Binding<Vec<PatchLink>>,
    width: f32,
    height: f32,
    fit: Option<f32>,
    front: Option<Rc<dyn View>>,
    flipped: Option<Binding<bool>>,
    color_of: Option<Painter>,
    texture_of: Option<Covering>,
    thickness: f32,
    retract: bool,
    pairs: Vec<(String, String)>,
    accept: Option<Rule>,
    on_connect: Option<LinkHandler>,
    on_disconnect: Option<LinkHandler>,
    on_join: Option<JoinHandler>,
    label: String,
    background: Option<Color>,
}

impl std::fmt::Debug for PatchPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatchPanel")
            .field("jacks", &self.jacks.len())
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

/// The colors of the signals, in the order they first appear.
const SIGNALS: [u32; 6] = [0x4C66EE, 0xE08A2E, 0x2F9E6A, 0xD64545, 0x8E5BD0, 0x2A9BB5];

/// How wide the name under a jack is.
const LABEL: f32 = 96.0;
const JACK: f32 = 22.0;

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl PatchPanel {
    /// A panel with these `jacks`, whose cables are `links`.
    pub fn new(jacks: Vec<Jack>, links: Binding<Vec<PatchLink>>) -> Self {
        Self {
            jacks: Rc::new(jacks),
            links,
            width: 480.0,
            height: 300.0,
            fit: None,
            front: None,
            flipped: None,
            color_of: None,
            texture_of: None,
            thickness: 6.0,
            retract: false,
            pairs: Vec::new(),
            accept: None,
            on_connect: None,
            on_disconnect: None,
            on_join: None,
            label: "Painel de tomadas".into(),
            background: None,
        }
    }

    /// The panel's size, in logical pixels (480 × 300 by default).
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Shows the panel smaller when it is wider than `width`, cables and pointer included:
    /// for a phone.
    pub fn fit(mut self, width: f32) -> Self {
        self.fit = Some(width);
        self
    }

    /// The equipment's front, shown when it is turned around; a button turns it.
    pub fn front(mut self, front: impl View + 'static) -> Self {
        self.front = Some(Rc::new(front));
        self
    }

    /// Whether the front shows, kept by the app; without it the panel keeps it.
    pub fn flipped(mut self, flipped: Binding<bool>) -> Self {
        self.flipped = Some(flipped);
        self
    }

    /// The color of the cables of each signal, instead of the panel's own.
    pub fn color_of(mut self, color: impl Fn(&str) -> Color + 'static) -> Self {
        self.color_of = Some(Rc::new(color));
        self
    }

    /// The texture of the cables of each signal (smooth by default).
    pub fn texture_of(mut self, texture: impl Fn(&str) -> CableTexture + 'static) -> Self {
        self.texture_of = Some(Rc::new(texture));
        self
    }

    /// How thick the cables are (6 by default).
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    /// A cable let go of goes back to the jack it came from, pulled as by a weight,
    /// instead of falling: the cords of a switchboard.
    pub fn retract(mut self, retract: bool) -> Self {
        self.retract = retract;
        self
    }

    /// Two outputs that work together, as the answering and calling cords of a
    /// switchboard: once both are plugged, [`Self::on_join`] hears the inputs they joined.
    pub fn pair(mut self, a: impl Into<String>, b: impl Into<String>) -> Self {
        self.pairs.push((a.into(), b.into()));
        self
    }

    /// Whether a cable may go from the first jack to the second, and why not, instead of
    /// the panel's rule (output to input, same signal, one cable an input).
    pub fn accept(mut self, rule: impl Fn(&Jack, &Jack) -> Result<(), String> + 'static) -> Self {
        self.accept = Some(Rc::new(rule));
        self
    }

    /// Called when a cable is plugged.
    pub fn on_connect(mut self, handler: impl Fn(&PatchLink) + 'static) -> Self {
        self.on_connect = Some(Rc::new(handler));
        self
    }

    /// Called when a cable is unplugged.
    pub fn on_disconnect(mut self, handler: impl Fn(&PatchLink) + 'static) -> Self {
        self.on_disconnect = Some(Rc::new(handler));
        self
    }

    /// Called with the two inputs a [pair](Self::pair) joined, once both its cords are in.
    pub fn on_join(mut self, handler: impl Fn(&str, &str) + 'static) -> Self {
        self.on_join = Some(Rc::new(handler));
        self
    }

    /// The name screen readers announce for the panel.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// The color of the panel's face, instead of the theme's.
    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }
}

/// What every jack of one panel shares while it is built.
struct Shared {
    panel: PatchPanel,
    kind: Rc<str>,
    history: State<Vec<Vec<PatchLink>>>,
    redo: State<Vec<Vec<PatchLink>>>,
    keys: State<HashMap<PatchLink, u64>>,
    next_key: State<u64>,
    /// A cable picked up with the keys, and the jack it is held to now.
    picked: State<Option<Pull>>,
    aim: State<Option<usize>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Shared {
    /// What pressing jack `i` takes: the plug of a plugged input, else a new cable from it.
    fn pull_at(&self, i: usize) -> Pull {
        let jacks = &self.panel.jacks;
        let links = self.panel.links.get();
        match (jacks[i].flow, links.iter().position(|l| l.to == jacks[i].id)) {
            (JackFlow::Input, Some(k)) => {
                Pull { from: self.index(&links[k].from).unwrap_or(i), link: Some(k) }
            }
            _ => Pull { from: i, link: None },
        }
    }

    /// The nearest jack from `at` along `(dx, dy)` that takes the cable in the hand.
    fn next(&self, pull: Pull, at: usize, (dx, dy): (f32, f32)) -> Option<usize> {
        let jacks = &self.panel.jacks;
        let here = jacks[at].at;
        jacks
            .iter()
            .enumerate()
            .filter(|&(k, j)| {
                let (x, y) = (j.at.x - here.x, j.at.y - here.y);
                k != at
                    && x * dx + y * dy > 0.5
                    && (k == pull.from || self.check(pull.from, k, pull.link).is_ok())
            })
            .min_by(|(_, a), (_, b)| {
                let d = |p: Point| {
                    let (x, y) = (p.x - here.x, p.y - here.y);
                    // Straight along the arrow counts more than across it.
                    let along = x * dx + y * dy;
                    along + (x * dy - y * dx).abs() * 2.0
                };
                d(a.at).total_cmp(&d(b.at))
            })
            .map(|(k, _)| k)
    }

    /// A key on jack `i` while a cable is picked up with the keys, or to pick one up.
    fn key(&self, i: usize, key: &Key) -> bool {
        let picked = self.picked.get();
        let arrow = match key {
            Key::ArrowLeft => Some((-1.0, 0.0)),
            Key::ArrowRight => Some((1.0, 0.0)),
            Key::ArrowUp => Some((0.0, -1.0)),
            Key::ArrowDown => Some((0.0, 1.0)),
            _ => None,
        };
        match (key, picked, arrow) {
            (Key::Enter, None, _) => {
                self.picked.set(Some(self.pull_at(i)));
                self.aim.set(Some(i));
                true
            }
            (Key::Enter, Some(pull), _) => {
                // Focus stays where it is: nothing pulls it back once the cable is down.
                self.picked.set(None);
                self.aim.set(None);
                if i != pull.from && self.check(pull.from, i, pull.link).is_ok() {
                    self.drop_on(pull, Some(i));
                }
                true
            }
            (Key::Escape, Some(_), _) => {
                self.picked.set(None);
                self.aim.set(None);
                true
            }
            (_, Some(pull), Some(direction)) => {
                if let Some(k) = self.next(pull, i, direction) {
                    self.aim.set(Some(k));
                }
                true
            }
            _ => false,
        }
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.panel.jacks.iter().position(|j| j.id == id)
    }

    fn signals(&self) -> Vec<&str> {
        let mut seen: Vec<&str> = Vec::new();
        for j in self.panel.jacks.iter() {
            if !j.signal.is_empty() && !seen.contains(&j.signal.as_str()) {
                seen.push(&j.signal);
            }
        }
        seen
    }

    fn color(&self, signal: &str) -> Color {
        if let Some(color) = &self.panel.color_of {
            return color(signal);
        }
        let at = self.signals().iter().position(|s| *s == signal);
        Color::hex(at.map_or(0x9AA3B5, |i| SIGNALS[i % SIGNALS.len()]))
    }

    /// The jack pair as output first, or why the cable cannot go between them.
    fn check(&self, a: usize, b: usize, moving: Option<usize>) -> Result<(usize, usize), String> {
        let jacks = &self.panel.jacks;
        let (out, inn) = match (jacks[a].flow, jacks[b].flow) {
            (JackFlow::Output, JackFlow::Input) => (a, b),
            (JackFlow::Input, JackFlow::Output) => (b, a),
            (JackFlow::Output, JackFlow::Output) => return Err("Duas saídas não se ligam".into()),
            (JackFlow::Input, JackFlow::Input) => return Err("Duas entradas não se ligam".into()),
        };
        if let Some(rule) = &self.panel.accept {
            rule(&jacks[out], &jacks[inn])?;
        } else {
            let (x, y) = (&jacks[out].signal, &jacks[inn].signal);
            if !x.is_empty() && !y.is_empty() && x != y {
                return Err(format!("{} não recebe {}", jacks[inn].label, x));
            }
        }
        let links = self.panel.links.get();
        let taken = links.iter().enumerate().any(|(i, l)| Some(i) != moving && l.to == jacks[inn].id);
        if taken {
            return Err(format!("{} já está ocupada", jacks[inn].label));
        }
        Ok((out, inn))
    }

    /// Changes the links, keeping the old ones to undo and telling the app.
    fn change(&self, edit: impl FnOnce(&mut Vec<PatchLink>)) {
        let before = self.panel.links.get();
        let mut after = before.clone();
        edit(&mut after);
        if after == before {
            return;
        }
        self.history.update(|h| h.push(before.clone()));
        self.redo.set(Vec::new());
        self.panel.links.set(after.clone());
        self.tell(&before, &after);
    }

    fn tell(&self, before: &[PatchLink], after: &[PatchLink]) {
        if let Some(handler) = &self.panel.on_disconnect {
            before.iter().filter(|l| !after.contains(l)).for_each(|l| handler(l));
        }
        if let Some(handler) = &self.panel.on_connect {
            after.iter().filter(|l| !before.contains(l)).for_each(|l| handler(l));
        }
        if let Some(handler) = &self.panel.on_join {
            for (a, b) in &self.panel.pairs {
                let to = |from: &str, links: &[PatchLink]| {
                    links.iter().find(|l| l.from == from).map(|l| l.to.clone())
                };
                let now = (to(a, after), to(b, after));
                if now != (to(a, before), to(b, before))
                    && let (Some(x), Some(y)) = now
                {
                    handler(&x, &y);
                }
            }
        }
    }

    fn undo(&self, back: bool) {
        let (from, to) = if back { (&self.history, &self.redo) } else { (&self.redo, &self.history) };
        let mut stack = from.get();
        let Some(links) = stack.pop() else { return };
        from.set(stack);
        let now = self.panel.links.get();
        to.update(|s| s.push(now.clone()));
        self.panel.links.set(links.clone());
        self.tell(&now, &links);
    }

    /// Plugs what is carried into `target`, or unplugs a moved link let go of on the
    /// panel (`target` None). A new link keeps the key of the cable in the hand, so the
    /// same cable goes on swinging.
    fn drop_on(&self, pull: Pull, target: Option<usize>) {
        let jacks = &self.panel.jacks;
        let hand = self.next_key.get();
        match (pull.link, target) {
            (Some(i), None) => self.change(|l| {
                if i < l.len() {
                    l.remove(i);
                }
            }),
            (link, Some(t)) => {
                let Ok((out, inn)) = self.check(pull.from, t, link) else { return };
                let made = PatchLink::new(&jacks[out].id, &jacks[inn].id);
                let old = link.and_then(|i| self.panel.links.get().get(i).cloned());
                let key = old.as_ref().and_then(|o| self.keys.get().get(o).copied()).unwrap_or(hand);
                self.keys.update(|k| {
                    if let Some(o) = &old {
                        k.remove(o);
                    }
                    k.insert(made.clone(), key);
                });
                if key == hand {
                    self.next_key.set(hand + 1);
                }
                self.change(|l| match link {
                    Some(i) if i < l.len() => l[i] = made,
                    _ => l.push(made),
                });
            }
            (None, None) => {}
        }
    }
}

/// One jack, its own widget so drops and focus find the right one.
struct Socket {
    index: usize,
    shared: Rc<Shared>,
}

impl View for Socket {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (i, shared) = (self.index, &self.shared);
        let jack = &shared.panel.jacks[i];
        let links = shared.panel.links.get();
        let color = shared.color(&jack.signal);
        let order = shared.signals().iter().position(|s| *s == jack.signal).unwrap_or(0);
        // The signal shows in the shape too: round, square, round with a dot...
        let radius = if order % 2 == 0 { JACK / 2.0 } else { 5.0 };
        let fill = match jack.flow {
            JackFlow::Output => Color::hex(0x2F3546),
            JackFlow::Input => Color::hex(0x0B0D12),
        };
        let carrying = cx.carrying().and_then(|c| {
            (c.payload.kind() == &*shared.kind).then(|| c.payload.data::<Pull>().copied()).flatten()
        });
        let verdict =
            carrying.or(shared.picked.get()).map(|p| p.from == i || shared.check(p.from, i, p.link).is_ok());
        let border = match verdict {
            Some(true) => Color::hex(0x3CCF8E),
            Some(false) => Color::hex(0x6F7890),
            None => color,
        };
        let paint = Paint::Quad(QuadPaint {
            fill: ravenrs_core::Brush::Solid(fill),
            corner_radius: radius,
            border: Some(Border { width: 3.0, color: border }),
            shadow: None,
        });
        let style = LayoutStyle {
            absolute: Some((jack.at.x - JACK / 2.0, jack.at.y - JACK / 2.0)),
            width: Some(JACK),
            height: Some(JACK),
            ..LayoutStyle::leaf()
        };
        let node = cx.container(style, &[], paint);
        let others: Vec<&str> = links
            .iter()
            .filter_map(|l| {
                if l.from == jack.id {
                    shared.index(&l.to)
                } else if l.to == jack.id {
                    shared.index(&l.from)
                } else {
                    None
                }
            })
            .map(|k| shared.panel.jacks[k].label.as_str())
            .collect();
        let way = match jack.flow {
            JackFlow::Output => "saída",
            JackFlow::Input => "entrada",
        };
        let kind = if jack.signal.is_empty() { String::new() } else { format!(" de {}", jack.signal) };
        let mut semantics = Semantics::new(Role::Button);
        semantics.name = Some(jack.label.clone());
        semantics.description = Some(if others.is_empty() {
            format!("{way}{kind}, livre")
        } else {
            format!("{way}{kind}, ligada a {}", others.join(", "))
        });
        cx.semantics(node, semantics);
        // Found by its id, which the app gives and the cables name.
        cx.test_id(node, &format!("jack:{}", jack.id));
        cx.focusable(node, radius);
        if shared.aim.get() == Some(i) && !cx.is_focused() {
            cx.request_focus();
        }
        // Pressed: an input that has a cable gives its plug; any other jack a new cable.
        let pull = shared.pull_at(i);
        cx.drag_source(node, DragPayload::new(&shared.kind, &jack.label, pull));
        let (check, plug) = (shared.clone(), shared.clone());
        let kind_check = shared.kind.clone();
        cx.drop_target(
            node,
            move |payload| {
                if payload.kind() != &*kind_check {
                    return Err("Outro painel".into());
                }
                let Some(&p) = payload.data::<Pull>() else { return Err("Outro painel".into()) };
                check.check(p.from, i, p.link).map(|_| ())
            },
            move |payload, _| {
                if let Some(&p) = payload.data::<Pull>() {
                    plug.drop_on(p, Some(i));
                }
            },
        );
        let keys = shared.clone();
        cx.on_key(
            node,
            Rc::new(move |key: &Key, m: Modifiers| {
                let command = m.ctrl || m.meta;
                if keys.key(i, key) {
                    return true;
                }
                match key {
                    Key::Delete | Key::Backspace => {
                        let id = keys.panel.jacks[i].id.clone();
                        keys.change(|l| l.retain(|x| x.from != id && x.to != id));
                        true
                    }
                    Key::Character(c) if command && c.eq_ignore_ascii_case("z") => {
                        keys.undo(!m.shift);
                        true
                    }
                    _ => false,
                }
            }),
        );
        node
    }
}

impl View for PatchPanel {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let own_flip = cx.use_state(|| false);
        let flipped = self.flipped.clone().unwrap_or_else(|| own_flip.binding());
        let id = cx.widget_id().raw();
        let shared = Rc::new(Shared {
            panel: self.clone(),
            kind: Rc::from(format!("patch-panel-{id}")),
            history: cx.use_state(Vec::new),
            redo: cx.use_state(Vec::new),
            keys: cx.use_state(HashMap::new),
            next_key: cx.use_state(|| 1_u64 << 32),
            picked: cx.use_state(|| None),
            aim: cx.use_state(|| None),
        });
        let theme_face = self.background.unwrap_or(Color::hex(0x1B1F29));
        let mut column = Vec::new();
        if self.front.is_some() {
            let flip = flipped.clone();
            let label = if flipped.get() { "Ver a traseira" } else { "Virar" };
            column.push(cx.child(
                &Button::new(label).style(ButtonStyle::Secondary).on_click(move || flip.set(!flip.get())),
            ));
        }
        let face = match (&self.front, flipped.get()) {
            (Some(front), true) => {
                let front = cx.child(front.as_ref());
                let style = LayoutStyle {
                    width: Some(self.width),
                    height: Some(self.height),
                    ..LayoutStyle::stack(Axis::Vertical)
                };
                cx.container(style, &[front], rounded(theme_face))
            }
            _ => self.back(cx, &shared, theme_face),
        };
        let face = match self.fit {
            Some(room) if room < self.width => {
                let k = room / self.width;
                cx.scale(face, k);
                let placed = LayoutStyle { absolute: Some((0.0, 0.0)), ..LayoutStyle::stack(Axis::Vertical) };
                let placed = cx.container(placed, &[face], Paint::None);
                let style = LayoutStyle {
                    width: Some(self.width * k),
                    height: Some(self.height * k),
                    ..LayoutStyle::stack(Axis::Vertical)
                };
                cx.container(style, &[placed], Paint::None)
            }
            _ => face,
        };
        column.push(face);
        let node = cx.container(
            LayoutStyle { spacing: 8.0, ..LayoutStyle::stack(Axis::Vertical) },
            &column,
            Paint::None,
        );
        let mut semantics = Semantics::new(Role::Group);
        semantics.name = Some(self.label.clone());
        cx.semantics(node, semantics);
        node
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl PatchPanel {
    /// The back: the jacks, their names and the cables.
    fn back(&self, cx: &mut BuildCx<'_>, shared: &Rc<Shared>, face: Color) -> NodeId {
        let links = self.links.get();
        let mut children = Vec::new();
        let mut nodes = Vec::new();
        for (i, jack) in self.jacks.iter().enumerate() {
            let socket = Socket { index: i, shared: shared.clone() };
            let node = cx.keyed_child(i as u64, &socket);
            let text = Text::new(jack.label.clone())
                .style(TextStyle::Caption)
                .foreground(Color::hex(0xC7CEDB))
                .align(TextAlign::Center);
            let label = cx.child(&text);
            let at = LayoutStyle {
                absolute: Some((jack.at.x - LABEL / 2.0, jack.at.y + JACK / 2.0 + 4.0)),
                width: Some(LABEL),
                ..LayoutStyle::leaf()
            };
            children.push(cx.container(at, &[label], Paint::None));
            children.push(node);
            nodes.push(node);
        }
        let by_pointer = cx
            .carrying()
            .filter(|c| c.payload.kind() == &*shared.kind)
            .and_then(|c| c.payload.data::<Pull>().copied());
        let by_keys = shared.picked.get();
        let carrying = by_pointer.or(by_keys);
        let keys = shared.keys.get();
        let release = if self.retract { CableRelease::Retract } else { CableRelease::Fall };
        let look = |plug: CablePlug, signal: &str| {
            let mut plug =
                plug.color(shared.color(signal)).thickness(self.thickness).plugs(true).release(release);
            if let Some(texture) = &self.texture_of {
                plug = plug.texture(texture(signal));
            }
            plug
        };
        for (k, link) in links.iter().enumerate() {
            if carrying.is_some_and(|p| p.link == Some(k)) {
                continue;
            }
            let (Some(a), Some(b)) = (shared.index(&link.from), shared.index(&link.to)) else { continue };
            // A link the panel did not see being made shows at rest; one plugged here
            // goes on with the cable that was in the hand.
            let (key, fresh) = match keys.get(link) {
                Some(&key) => (key, true),
                None => (key_of(link), false),
            };
            look(CablePlug::between(nodes[a], nodes[b]), &self.jacks[a].signal)
                .sides(CableSide::Center, CableSide::Center)
                .key(key)
                .grow(fresh)
                .build(cx);
        }
        if let Some(pull) = carrying {
            let key = pull
                .link
                .and_then(|k| links.get(k))
                .and_then(|l| keys.get(l).copied().or(Some(key_of(l))))
                .unwrap_or_else(|| shared.next_key.get());
            let hand = match (by_pointer, shared.aim.get()) {
                // With the keys the plug hangs over the jack chosen with the arrows.
                (None, Some(aim)) if aim != pull.from => CablePlug::between(nodes[pull.from], nodes[aim])
                    .sides(CableSide::Center, CableSide::Center),
                (None, _) => CablePlug::between(nodes[pull.from], nodes[pull.from])
                    .sides(CableSide::Center, CableSide::Center),
                (Some(_), _) => CablePlug::from(nodes[pull.from]).sides(CableSide::Center, CableSide::Center),
            };
            look(hand, &self.jacks[pull.from].signal).key(key).build(cx);
        }
        let style = LayoutStyle {
            width: Some(self.width),
            height: Some(self.height),
            ..LayoutStyle::stack(Axis::Vertical)
        };
        let panel = cx.container(style, &children, rounded(face));
        // Let go of on the panel's empty space: a plug taken out of an input unplugs.
        let (check, drop) = (shared.kind.clone(), shared.clone());
        cx.drop_target(
            panel,
            move |payload| if payload.kind() == &*check { Ok(()) } else { Err("Outro painel".into()) },
            move |payload, _| {
                if let Some(&p) = payload.data::<Pull>() {
                    drop.drop_on(p, None);
                }
            },
        );
        panel
    }
}

/// The key of a link's cable when the panel did not see it being made.
fn key_of(link: &PatchLink) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    link.hash(&mut h);
    h.finish() >> 1
}

/// The panel's face: `color`, with round corners.
fn rounded(color: Color) -> Paint {
    Paint::Quad(QuadPaint {
        fill: ravenrs_core::Brush::Solid(color),
        corner_radius: 12.0,
        border: None,
        shadow: None,
    })
}
