//! Someone types a question and the right moment of a training film opens.

use ravenrs_layout::{Align, Axis, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, TextStyle};
use ravenrs_widgets::{
    BuildCx, Card, IconName, ListItem, NodeId, Paint, Placement, SegmentedControl, Select, Text, TextField,
    VStack, View, ViewExt,
};

use crate::narration::Narration;
use crate::player::{MovieCue, MovieLabels, MoviePlayer, clock_label};
use crate::search::{Answer, Moment, MomentKind, MovieLibrary};

/// How many answers it offers: more than this and it is a list to read, not an answer.
const ANSWERS: usize = 4;
const SPACING: f32 = 12.0;
/// Below this width the names of the films do not fit side by side.
const TABS_FROM: f32 = 860.0;
const PICKER_WIDTH: f32 = 280.0;
/// What goes around the film: the question, the choice of film and the controls.
const AROUND_THE_FILM: f32 = 184.0;

/// The words a [`MovieAssistant`] shows. English by default.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct AssistantLabels {
    /// Inside the empty question field.
    pub placeholder: String,
    /// When no film answers the question.
    pub nothing_found: String,
    /// The name of the choice of film, for screen readers.
    pub film: String,
    /// The player's own words.
    pub player: MovieLabels,
}

impl Default for AssistantLabels {
    fn default() -> Self {
        Self {
            placeholder: "Ask how to do something".into(),
            nothing_found: "No film talks about that. Try other words.".into(),
            film: "Film".into(),
            player: MovieLabels::default(),
        }
    }
}

impl AssistantLabels {
    /// The words in Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self {
            placeholder: "Pergunte como fazer alguma coisa".into(),
            nothing_found: "Nenhum filme fala sobre isso. Tente outras palavras.".into(),
            film: "Assunto".into(),
            player: MovieLabels::pt_br(),
        }
    }
}

/// A question field over the films of a [`MovieLibrary`]. Each answer is a moment
/// of a film, and choosing one opens the film right there.
///
/// It looks only at the words the films hold, on this machine, with no model.
///
/// ```
/// use ravenrs_movie::{MovieAssistant, MovieLibrary};
///
/// let assistant = MovieAssistant::new(MovieLibrary::new()).width(960.0);
/// ```
#[derive(Debug)]
pub struct MovieAssistant {
    library: MovieLibrary,
    width: f32,
    height: Option<f32>,
    labels: AssistantLabels,
    cue: Option<MovieCue>,
    narration: Option<Narration>,
}

impl MovieAssistant {
    /// An assistant over `library`, with an empty question.
    pub fn new(library: MovieLibrary) -> Self {
        Self {
            library,
            width: 960.0,
            height: None,
            labels: AssistantLabels::default(),
            cue: None,
            narration: None,
        }
    }

    /// The width of the whole assistant, in logical pixels (960 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The height it should fit in: the film gets smaller rather than pushing its
    /// controls out of view. With none, the film takes the whole width.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Recordings of what the films say, in place of the system's voice.
    #[must_use]
    pub fn narration(mut self, narration: Narration) -> Self {
        self.narration = Some(narration);
        self
    }

    /// The words it shows.
    pub fn labels(mut self, labels: AssistantLabels) -> Self {
        self.labels = labels;
        self
    }

    /// Lets something outside it, such as a [`MovieChat`](crate::MovieChat), open a
    /// moment of the film it shows.
    #[must_use]
    pub fn cue(mut self, cue: MovieCue) -> Self {
        self.cue = Some(cue);
        self
    }
}

/// Without the steps inside what it also found: "Pagar uma conta" and "Pague uma
/// conta" read as the same thing twice. Only steps are kept when nothing else came up.
pub(crate) fn whole_answers(answers: Vec<Answer>) -> Vec<Answer> {
    let whole: Vec<Answer> =
        answers.iter().filter(|answer| answer.moment.kind != MomentKind::Step).cloned().collect();
    if whole.is_empty() { answers } else { whole }
}

/// A row that says which moment an answer is and opens it.
pub(crate) fn moment_row(
    library: &MovieLibrary,
    answer: &Answer,
    open: impl Fn(&Moment) + 'static,
) -> ListItem {
    let moment = answer.moment.clone();
    let film = library.get(moment.film).map_or("", |(title, _)| title);
    let when = clock_label(moment.at_ms);
    let trailing = if library.len() > 1 { format!("{film} · {when}") } else { when };
    let chosen = moment.clone();
    ListItem::new(moment.title.clone())
        .subtitle(moment.text.clone())
        .icon(IconName::Play)
        .trailing(trailing)
        .on_click(move || open(&chosen))
}

/// The same in a narrow place, such as a chat: the name and where it is, one under the other.
pub(crate) fn short_moment_row(
    library: &MovieLibrary,
    answer: &Answer,
    open: impl Fn(&Moment) + 'static,
) -> ListItem {
    let moment = answer.moment.clone();
    let film = library.get(moment.film).map_or("", |(title, _)| title);
    let when = clock_label(moment.at_ms);
    let chosen = moment.clone();
    ListItem::new(moment.title.clone())
        .subtitle(if library.len() > 1 { format!("{film} · {when}") } else { when })
        .icon(IconName::Play)
        .on_click(move || open(&chosen))
}

/// What choosing an answer does. The answers then step aside, so the film is in
/// view, until the question changes.
#[derive(Clone)]
struct Open {
    film: State<usize>,
    question: State<String>,
    answered: State<Option<String>>,
    cue: MovieCue,
}

impl Open {
    fn moment(&self, moment: &Moment) {
        self.film.set(moment.film);
        self.answered.set(Some(self.question.get()));
        self.cue.go_to(moment.at_ms);
    }
}

impl View for MovieAssistant {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let question: State<String> = cx.use_state(String::new);
        let film: State<usize> = cx.use_state(|| 0);
        let answered: State<Option<String>> = cx.use_state(|| None);
        let own_cue = cx.use_state(MovieCue::new).get();
        let cue = self.cue.clone().unwrap_or(own_cue);
        if let Some(asked) = cue.take_film() {
            film.set(asked);
        }
        let open = Open {
            film: film.clone(),
            question: question.clone(),
            answered: answered.clone(),
            cue: cue.clone(),
        };

        let asked = question.get();
        let waiting = answered.get().as_ref() != Some(&asked);
        let answers = if waiting { whole_answers(self.library.ask(&asked, ANSWERS)) } else { Vec::new() };
        let (library, first) = (self.library.clone(), open.clone());
        let field = TextField::new(question.binding())
            .placeholder(self.labels.placeholder.clone())
            .icon(IconName::Search)
            .on_submit(move |text| {
                if let Some(best) = library.ask(&text, 1).first() {
                    first.moment(&best.moment);
                }
            });
        let rows: Vec<ListItem> = answers
            .iter()
            .map(|answer| {
                let open = open.clone();
                moment_row(&self.library, answer, move |moment| open.moment(moment))
            })
            .collect();
        let nothing = (waiting && !asked.trim().is_empty() && rows.is_empty()).then(|| {
            Text::new(self.labels.nothing_found.clone())
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted)
        });
        let field = cx.child(&field.width(self.width));
        // The answers open over the film, which stays where it is.
        if !rows.is_empty() || nothing.is_some() {
            let answers = Card::new(
                VStack::new((VStack::new(rows).spacing(2.0).align(Align::Stretch), nothing))
                    .align(Align::Stretch),
            )
            .padding(8.0)
            .width(self.width);
            let (close, asked) = (answered.clone(), asked.clone());
            let dismiss = std::rc::Rc::new(move || close.set(Some(asked.clone())));
            cx.overlay(field, Placement::Below, &answers, Some(dismiss));
        }
        let mut children = vec![field];

        if self.library.len() > 1 {
            let titles: Vec<String> =
                (0..self.library.len()).filter_map(|i| self.library.get(i)).map(|f| f.0.to_owned()).collect();
            children.push(if self.width >= TABS_FROM {
                cx.child(&SegmentedControl::new(film.binding(), titles).label(self.labels.film.clone()))
            } else {
                cx.child(
                    &Select::new(film.binding(), titles)
                        .label(self.labels.film.clone())
                        .width(PICKER_WIDTH.min(self.width)),
                )
            });
        }
        let shown = film.get();
        if let Some((_, movie)) = self.library.get(shown) {
            let mut player =
                MoviePlayer::new(movie.clone()).width(self.width).labels(self.labels.player.clone()).cue(cue);
            if let Some(height) = self.height {
                player = player.max_film_height(height - AROUND_THE_FILM);
            }
            if let Some(narration) = &self.narration {
                player = player.narration(narration.clone());
            }
            // Each film keeps its own place.
            children.push(cx.keyed_child(shown as u64, &player));
        }
        cx.container(
            LayoutStyle { spacing: SPACING, ..LayoutStyle::stack(Axis::Vertical) },
            &children,
            Paint::None,
        )
    }
}
