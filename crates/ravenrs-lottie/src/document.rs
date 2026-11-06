//! A Lottie file, read.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::sync::Arc;

use ravenrs_core::{ImageData, PathFill};
use serde_json::Value;

use crate::player;
use crate::property::{Numbers, Shape, numbers};

/// Why a file could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LottieError {
    /// It is not JSON.
    NotJson(String),
    /// It is JSON, but not a Lottie: this field is missing.
    Missing(&'static str),
}

impl fmt::Display for LottieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotJson(why) => write!(f, "the file is not JSON: {why}"),
            Self::Missing(field) => write!(f, "the file is not a Lottie: it has no `{field}`"),
        }
    }
}

impl std::error::Error for LottieError {}

#[derive(Clone, Debug)]
pub(crate) struct Transform {
    pub anchor: Numbers,
    pub position: Numbers,
    pub split: Option<(Numbers, Numbers)>,
    pub scale: Numbers,
    pub rotation: Numbers,
    pub opacity: Numbers,
    pub skew: Numbers,
    pub skew_axis: Numbers,
    /// Turns the layer along its path of motion.
    pub orient: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct GradientPaint {
    pub radial: bool,
    pub start: Numbers,
    pub end: Numbers,
    pub stops: Numbers,
    pub count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct StrokeLook {
    pub width: Numbers,
    pub cap: i64,
    pub join: i64,
    pub miter_limit: f32,
    pub dashes: Vec<(String, Numbers)>,
}

#[derive(Clone, Debug)]
pub(crate) enum Item {
    Group(Vec<Item>, Option<Transform>),
    Path(Shape),
    Rect {
        position: Numbers,
        size: Numbers,
        radius: Numbers,
    },
    Ellipse {
        position: Numbers,
        size: Numbers,
    },
    Star {
        polygon: bool,
        position: Numbers,
        rotation: Numbers,
        points: Numbers,
        inner: Numbers,
        outer: Numbers,
        inner_round: Numbers,
        outer_round: Numbers,
    },
    Fill {
        color: Numbers,
        opacity: Numbers,
        even_odd: bool,
    },
    Stroke {
        color: Numbers,
        opacity: Numbers,
        look: StrokeLook,
    },
    GradientFill {
        paint: GradientPaint,
        opacity: Numbers,
        even_odd: bool,
    },
    GradientStroke {
        paint: GradientPaint,
        opacity: Numbers,
        look: StrokeLook,
    },
    Trim {
        start: Numbers,
        end: Numbers,
        offset: Numbers,
        together: bool,
    },
    Repeater {
        copies: Numbers,
        offset: Numbers,
        transform: Transform,
        start_opacity: Numbers,
        end_opacity: Numbers,
    },
    RoundedCorners(Numbers),
}

#[derive(Clone, Debug)]
pub(crate) enum LayerKind {
    Shape(Vec<Item>),
    Solid { width: f32, height: f32, color: [f32; 3] },
    Image { image: ImageData, width: f32, height: f32 },
    Precomp { reference: String, width: f32, height: f32, remap: Option<Numbers> },
    Empty,
}

#[derive(Clone, Debug)]
pub(crate) struct Mask {
    pub path: Shape,
    pub subtract: bool,
    pub inverted: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Layer {
    pub kind: LayerKind,
    pub index: Option<i64>,
    pub parent: Option<i64>,
    pub transform: Transform,
    pub in_frame: f32,
    pub out_frame: f32,
    pub start: f32,
    pub stretch: f32,
    pub hidden: bool,
    pub masks: Vec<Mask>,
    /// How the layer above it, or the one `matte_index` names, masks it: 1 and 3 keep
    /// what it covers, 2 and 4 what it does not.
    pub matte_mode: i64,
    pub matte_index: Option<i64>,
    pub is_matte: bool,
    /// The Fill effect, which paints everything in the layer one color and opacity.
    pub fill_effect: Option<(Numbers, Numbers)>,
}

#[derive(Debug)]
pub(crate) struct Document {
    pub width: f32,
    pub height: f32,
    pub frame_rate: f32,
    pub in_frame: f32,
    pub out_frame: f32,
    pub layers: Vec<Layer>,
    pub precomps: HashMap<String, Vec<Layer>>,
    pub unsupported: BTreeSet<String>,
}

/// A Lottie animation. Cloning is cheap, and it can be drawn at any moment.
///
/// ```
/// use ravenrs_lottie::Animation;
///
/// let json = br#"{"v":"5.5.7","fr":30,"ip":0,"op":30,"w":100,"h":100,"layers":[]}"#;
/// let animation = Animation::from_json(json).expect("a Lottie");
/// assert_eq!(animation.duration(), 1.0);
/// assert!(animation.paths_at(0.5, 1.0).is_empty());
/// ```
#[derive(Clone, Debug)]
pub struct Animation(pub(crate) Arc<Document>);

fn number(value: Option<&Value>, default: f32) -> f32 {
    value.and_then(Value::as_f64).map_or(default, |v| v as f32)
}

struct Reader {
    unsupported: BTreeSet<String>,
    images: HashMap<String, (ImageData, f32, f32)>,
}

impl Reader {
    /// The color and opacity of a Fill effect, which go by their place among its controls.
    fn fill_effect(&mut self, effect: &Value) -> (Numbers, Numbers) {
        let controls = effect.get("ef").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
        let control = |index: i64| {
            controls
                .iter()
                .find(|c| c.get("ix").and_then(Value::as_i64) == Some(index))
                .and_then(|c| c.get("v"))
        };
        if control(1).and_then(|v| v.get("k")).and_then(Value::as_f64).is_some_and(|mask| mask != 0.0) {
            self.note("efeito Fill limitado a uma máscara, aplicado à camada toda");
        }
        (self.numbers(control(3)), self.numbers(control(7)))
    }

    fn note(&mut self, what: &str) {
        self.unsupported.insert(what.to_owned());
    }

    /// Reads a property and notes an expression, whose saved value is what is drawn.
    fn numbers(&mut self, property: Option<&Value>) -> Numbers {
        if property.and_then(|p| p.get("x")).and_then(Value::as_str).is_some_and(|x| !x.is_empty()) {
            self.note("expressões");
        }
        Numbers::parse(property)
    }

    fn transform(&mut self, value: Option<&Value>) -> Transform {
        let empty = Value::Null;
        let v = value.unwrap_or(&empty);
        let position = v.get("p");
        let split = position
            .filter(|p| {
                p.get("s").and_then(Value::as_bool) == Some(true)
                    || p.get("s").and_then(Value::as_i64) == Some(1)
            })
            .map(|p| (self.numbers(p.get("x")), self.numbers(p.get("y"))));
        if v.get("rx").is_some() || v.get("ry").is_some() {
            self.note("rotação em 3D");
        }
        Transform {
            anchor: self.numbers(v.get("a")),
            position: self.numbers(position),
            split,
            scale: self.numbers(v.get("s")),
            rotation: self.numbers(v.get("r").or_else(|| v.get("rz"))),
            opacity: self.numbers(v.get("o")),
            skew: self.numbers(v.get("sk")),
            skew_axis: self.numbers(v.get("sa")),
            orient: false,
        }
    }

    fn stroke_look(&mut self, item: &Value) -> StrokeLook {
        let dashes = item
            .get("d")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .map(|dash| {
                        (
                            dash.get("n").and_then(Value::as_str).unwrap_or("d").to_owned(),
                            self.numbers(dash.get("v")),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        StrokeLook {
            width: self.numbers(item.get("w")),
            cap: item.get("lc").and_then(Value::as_i64).unwrap_or(1),
            join: item.get("lj").and_then(Value::as_i64).unwrap_or(1),
            miter_limit: number(item.get("ml"), 4.0),
            dashes,
        }
    }

    fn gradient(&mut self, item: &Value) -> GradientPaint {
        if item
            .get("h")
            .is_some_and(|h| !numbers(h.get("k").unwrap_or(&Value::Null)).iter().all(|v| *v == 0.0))
        {
            self.note("gradiente com foco deslocado");
        }
        let g = item.get("g");
        GradientPaint {
            radial: item.get("t").and_then(Value::as_i64) == Some(2),
            start: self.numbers(item.get("s")),
            end: self.numbers(item.get("e")),
            stops: self.numbers(g.and_then(|g| g.get("k"))),
            count: g.and_then(|g| g.get("p")).and_then(Value::as_u64).unwrap_or(0) as usize,
        }
    }

    fn items(&mut self, list: Option<&Value>) -> Vec<Item> {
        let Some(list) = list.and_then(Value::as_array) else { return Vec::new() };
        let mut items = Vec::with_capacity(list.len());
        for item in list {
            if item.get("hd").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let even_odd = item.get("r").and_then(Value::as_i64) == Some(2);
            let kind = item.get("ty").and_then(Value::as_str).unwrap_or("");
            let read = match kind {
                "gr" => {
                    let inner = item.get("it").and_then(Value::as_array);
                    let transform = inner
                        .and_then(|list| {
                            list.iter().find(|i| i.get("ty").and_then(Value::as_str) == Some("tr"))
                        })
                        .map(|tr| self.transform(Some(tr)));
                    Some(Item::Group(self.items(item.get("it")), transform))
                }
                // Read with its group.
                "tr" => None,
                "sh" => Some(Item::Path(Shape::parse(item.get("ks")))),
                "rc" => Some(Item::Rect {
                    position: self.numbers(item.get("p")),
                    size: self.numbers(item.get("s")),
                    radius: self.numbers(item.get("r")),
                }),
                "el" => Some(Item::Ellipse {
                    position: self.numbers(item.get("p")),
                    size: self.numbers(item.get("s")),
                }),
                "sr" => Some(Item::Star {
                    polygon: item.get("sy").and_then(Value::as_i64) == Some(2),
                    position: self.numbers(item.get("p")),
                    rotation: self.numbers(item.get("r")),
                    points: self.numbers(item.get("pt")),
                    inner: self.numbers(item.get("ir")),
                    outer: self.numbers(item.get("or")),
                    inner_round: self.numbers(item.get("is")),
                    outer_round: self.numbers(item.get("os")),
                }),
                "fl" => Some(Item::Fill {
                    color: self.numbers(item.get("c")),
                    opacity: self.numbers(item.get("o")),
                    even_odd,
                }),
                "st" => Some(Item::Stroke {
                    color: self.numbers(item.get("c")),
                    opacity: self.numbers(item.get("o")),
                    look: self.stroke_look(item),
                }),
                "gf" => Some(Item::GradientFill {
                    paint: self.gradient(item),
                    opacity: self.numbers(item.get("o")),
                    even_odd,
                }),
                "gs" => Some(Item::GradientStroke {
                    paint: self.gradient(item),
                    opacity: self.numbers(item.get("o")),
                    look: self.stroke_look(item),
                }),
                "tm" => Some(Item::Trim {
                    start: self.numbers(item.get("s")),
                    end: self.numbers(item.get("e")),
                    offset: self.numbers(item.get("o")),
                    together: item.get("m").and_then(Value::as_i64) == Some(2),
                }),
                "rp" => {
                    let tr = item.get("tr");
                    Some(Item::Repeater {
                        copies: self.numbers(item.get("c")),
                        offset: self.numbers(item.get("o")),
                        transform: self.transform(tr),
                        start_opacity: self.numbers(tr.and_then(|t| t.get("so"))),
                        end_opacity: self.numbers(tr.and_then(|t| t.get("eo"))),
                    })
                }
                "rd" => Some(Item::RoundedCorners(self.numbers(item.get("r")))),
                "mm" => {
                    self.note("juntar caminhos (merge paths)");
                    None
                }
                "tw" => {
                    self.note("torcer (twist)");
                    None
                }
                "pb" => {
                    self.note("inflar e contrair (pucker and bloat)");
                    None
                }
                "op" => {
                    self.note("deslocar caminho (offset path)");
                    None
                }
                other => {
                    self.note(&format!("forma do tipo {other}"));
                    None
                }
            };
            items.extend(read);
        }
        items
    }

    fn layers(&mut self, list: Option<&Value>) -> Vec<Layer> {
        let Some(list) = list.and_then(Value::as_array) else { return Vec::new() };
        let mut layers = Vec::with_capacity(list.len());
        for layer in list {
            let kind = match layer.get("ty").and_then(Value::as_i64).unwrap_or(3) {
                0 => {
                    let remap = layer.get("tm").map(|tm| self.numbers(Some(tm)));
                    LayerKind::Precomp {
                        reference: layer.get("refId").and_then(Value::as_str).unwrap_or("").to_owned(),
                        width: number(layer.get("w"), 0.0),
                        height: number(layer.get("h"), 0.0),
                        remap,
                    }
                }
                1 => LayerKind::Solid {
                    width: number(layer.get("sw"), 0.0),
                    height: number(layer.get("sh"), 0.0),
                    color: solid_color(layer.get("sc").and_then(Value::as_str).unwrap_or("#000000")),
                },
                2 => {
                    let found = layer.get("refId").and_then(Value::as_str).and_then(|id| self.images.get(id));
                    match found {
                        Some((image, width, height)) => {
                            LayerKind::Image { image: image.clone(), width: *width, height: *height }
                        }
                        None => {
                            self.note("imagens fora do arquivo");
                            LayerKind::Empty
                        }
                    }
                }
                3 => LayerKind::Empty,
                4 => LayerKind::Shape(self.items(layer.get("shapes"))),
                5 => {
                    self.note("texto");
                    LayerKind::Empty
                }
                other => {
                    self.note(&format!("camada do tipo {other}"));
                    LayerKind::Empty
                }
            };
            if layer.get("bm").and_then(Value::as_i64).unwrap_or(0) != 0 {
                self.note("modos de mistura");
            }
            let mut fill_effect = None;
            for effect in layer.get("ef").and_then(Value::as_array).into_iter().flatten() {
                if effect.get("en").and_then(Value::as_i64) == Some(0) {
                    continue;
                }
                match effect.get("ty").and_then(Value::as_i64) {
                    Some(21) => fill_effect = Some(self.fill_effect(effect)),
                    _ => self.note("efeitos do After Effects"),
                }
            }
            if layer.get("ddd").and_then(Value::as_i64) == Some(1) {
                self.note("camadas em 3D");
            }
            let matte_mode = layer.get("tt").and_then(Value::as_i64).unwrap_or(0);
            if matte_mode >= 3 {
                self.note("matte de luminosidade, tratado como alfa");
            }
            let masks = layer
                .get("masksProperties")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(|mask| {
                            let mode = mask.get("mode").and_then(Value::as_str).unwrap_or("a");
                            if mode == "n" {
                                return None;
                            }
                            if mode != "a" && mode != "s" {
                                self.note("máscara que não soma nem subtrai, tratada como soma");
                            }
                            Some(Mask {
                                path: Shape::parse(mask.get("pt")),
                                subtract: mode == "s",
                                inverted: mask.get("inv").and_then(Value::as_bool).unwrap_or(false),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let start = number(layer.get("st"), 0.0);
            layers.push(Layer {
                kind,
                index: layer.get("ind").and_then(Value::as_i64),
                parent: layer.get("parent").and_then(Value::as_i64),
                transform: Transform {
                    orient: layer.get("ao").and_then(Value::as_i64) == Some(1),
                    ..self.transform(layer.get("ks"))
                },
                in_frame: number(layer.get("ip"), f32::MIN),
                out_frame: number(layer.get("op"), f32::MAX),
                start,
                stretch: {
                    let stretch = number(layer.get("sr"), 1.0);
                    if stretch == 0.0 { 1.0 } else { stretch }
                },
                hidden: layer.get("hd").and_then(Value::as_bool).unwrap_or(false),
                masks,
                matte_mode,
                matte_index: layer.get("tp").and_then(Value::as_i64),
                is_matte: layer.get("td").and_then(Value::as_i64).unwrap_or(0) != 0,
                fill_effect,
            });
        }
        layers
    }
}

/// The images a file carries inside it, by name, with the size each shows at.
fn images_of(root: &Value) -> HashMap<String, (ImageData, f32, f32)> {
    let mut images = HashMap::new();
    for asset in root.get("assets").and_then(Value::as_array).into_iter().flatten() {
        let (Some(id), Some(path)) =
            (asset.get("id").and_then(Value::as_str), asset.get("p").and_then(Value::as_str))
        else {
            continue;
        };
        let Some(image) = path.split_once(";base64,").and_then(|(_, data)| decoded_image(data)) else {
            continue;
        };
        let width = number(asset.get("w"), image.width() as f32);
        let height = number(asset.get("h"), image.height() as f32);
        images.insert(id.to_owned(), (image, width, height));
    }
    images
}

fn decoded_image(base64: &str) -> Option<ImageData> {
    let picture = image::load_from_memory(&from_base64(base64)?).ok()?.into_rgba8();
    ImageData::from_rgba8(picture.width(), picture.height(), picture.into_raw())
}

fn from_base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut bits, mut count) = (0u32, 0u32);
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    Some(out)
}

fn solid_color(text: &str) -> [f32; 3] {
    let hex = text.trim_start_matches('#');
    let part = |at: usize| {
        hex.get(at..at + 2).and_then(|h| u8::from_str_radix(h, 16).ok()).map_or(0.0, |v| f32::from(v) / 255.0)
    };
    [part(0), part(2), part(4)]
}

impl Animation {
    /// Reads the JSON of a Lottie file.
    pub fn from_json(bytes: &[u8]) -> Result<Self, LottieError> {
        let root: Value = serde_json::from_slice(bytes).map_err(|e| LottieError::NotJson(e.to_string()))?;
        let field =
            |name: &'static str| root.get(name).and_then(Value::as_f64).ok_or(LottieError::Missing(name));
        let (width, height) = (field("w")? as f32, field("h")? as f32);
        let frame_rate = field("fr")? as f32;
        let (in_frame, out_frame) = (field("ip")? as f32, field("op")? as f32);
        if root.get("layers").is_none() {
            return Err(LottieError::Missing("layers"));
        }
        let mut reader = Reader { unsupported: BTreeSet::new(), images: images_of(&root) };
        let layers = reader.layers(root.get("layers"));
        let mut precomps = HashMap::new();
        if let Some(assets) = root.get("assets").and_then(Value::as_array) {
            for asset in assets {
                if let (Some(id), Some(_)) = (asset.get("id").and_then(Value::as_str), asset.get("layers")) {
                    let inner = reader.layers(asset.get("layers"));
                    precomps.insert(id.to_owned(), inner);
                }
            }
        }
        Ok(Self(Arc::new(Document {
            width,
            height,
            frame_rate: frame_rate.max(1.0),
            in_frame,
            out_frame: out_frame.max(in_frame),
            layers,
            precomps,
            unsupported: reader.unsupported,
        })))
    }

    /// The width of its composition.
    pub fn width(&self) -> f32 {
        self.0.width
    }

    /// The height of its composition.
    pub fn height(&self) -> f32 {
        self.0.height
    }

    /// How long it runs, in seconds.
    pub fn duration(&self) -> f64 {
        f64::from(self.0.out_frame - self.0.in_frame) / f64::from(self.0.frame_rate)
    }

    /// Its frames per second, which only says how it was authored: it can be drawn at
    /// any moment in between.
    pub fn frame_rate(&self) -> f32 {
        self.0.frame_rate
    }

    /// What the file asks for that is not drawn as asked, in words a person reads.
    /// Empty when nothing is missing.
    pub fn unsupported(&self) -> Vec<String> {
        self.0.unsupported.iter().cloned().collect()
    }

    /// The shapes to draw at this moment, in painter's order and in the coordinates of
    /// the composition. `scale` is how many device pixels a unit will take: curves that
    /// become straight pieces, as in a stroke, are cut finely enough for it.
    pub fn paths_at(&self, seconds: f64, scale: f32) -> Vec<PathFill> {
        player::paths_at(&self.0, seconds, scale)
    }
}
