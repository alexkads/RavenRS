//! The training films as a conversation: each answer is a card with the moments.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use ravenrs_reactive::Binding;
use ravenrs_widgets::{BuildCx, Chat, ChatMessage, List, ListItem, NodeId, UiWaker, View};

use crate::assistant::moment_row;
use crate::search::{Answer, Moment, MovieLibrary};

/// Writes an answer about the moments that a question found. It gets the question,
/// the moments, where to write the text, what to do when the text moves, and what
/// to write if it fails. See [`MovieChat::writer`].
pub type MovieWriter = Rc<dyn Fn(&str, &[Moment], Arc<Mutex<String>>, UiWaker, String)>;

/// How many moments an answer offers.
const MOMENTS: usize = 3;

/// The words a [`MovieChat`] shows. English by default.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct MovieChatLabels {
    /// Inside the empty field.
    pub placeholder: String,
    /// What it says before the first question.
    pub intro: String,
    /// Over the moments that answer.
    pub found: String,
    /// When no film answers the question.
    pub nothing_found: String,
}

impl Default for MovieChatLabels {
    fn default() -> Self {
        Self {
            placeholder: "Ask how to do something".into(),
            intro: "Ask how to do something and I will open the moment of the training film that shows it."
                .into(),
            found: "These moments of the films show it:".into(),
            nothing_found: "No film talks about that. Try other words.".into(),
        }
    }
}

impl MovieChatLabels {
    /// The words in Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self {
            placeholder: "Pergunte como fazer alguma coisa".into(),
            intro:
                "Pergunte como fazer alguma coisa, e eu abro o momento do filme de treinamento que mostra."
                    .into(),
            found: "Estes momentos dos filmes mostram isso:".into(),
            nothing_found: "Nenhum filme fala sobre isso. Tente outras palavras.".into(),
        }
    }
}

/// A [`Chat`] that answers from the films of a [`MovieLibrary`]. It looks only at
/// the words the films hold, on this machine, with no model.
///
/// The messages are the app's, so that they outlive a closed panel, and `on_open`
/// says what opening a moment means there, usually going to the film with a
/// [`MovieCue`](crate::MovieCue).
///
/// ```
/// use ravenrs_movie::{MovieChat, MovieLibrary};
/// use ravenrs_reactive::State;
/// use ravenrs_widgets::ChatMessage;
///
/// let messages = State::new(Vec::<ChatMessage>::new());
/// let chat = MovieChat::new(MovieLibrary::new(), messages.binding(), |moment| {
///     println!("open {}", moment.title);
/// });
/// ```
pub struct MovieChat {
    library: MovieLibrary,
    messages: Binding<Vec<ChatMessage>>,
    on_open: Rc<dyn Fn(&Moment)>,
    labels: MovieChatLabels,
    examples: Vec<String>,
    writer: Option<MovieWriter>,
}

impl std::fmt::Debug for MovieChat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MovieChat").field("library", &self.library).finish_non_exhaustive()
    }
}

impl MovieChat {
    /// A conversation over `library`, keeping `messages`.
    pub fn new(
        library: MovieLibrary,
        messages: Binding<Vec<ChatMessage>>,
        on_open: impl Fn(&Moment) + 'static,
    ) -> Self {
        Self {
            library,
            messages,
            on_open: Rc::new(on_open),
            labels: MovieChatLabels::default(),
            examples: Vec::new(),
            writer: None,
        }
    }

    /// Something that writes the answer in its own words, such as a local model,
    /// instead of a fixed sentence. Finding the moments stays with the search, which
    /// answered better than a small model when measured, and a question that finds
    /// nothing is not passed on, so it cannot make something up.
    #[must_use]
    pub fn writer(mut self, writer: Option<MovieWriter>) -> Self {
        self.writer = writer;
        self
    }

    /// The words it shows.
    #[must_use]
    pub fn labels(mut self, labels: MovieChatLabels) -> Self {
        self.labels = labels;
        self
    }

    /// Things to ask, offered before the first question.
    #[must_use]
    pub fn examples(mut self, examples: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.examples = examples.into_iter().map(Into::into).collect();
        self
    }
}

impl MovieAnswers {
    /// What to say to a question: the moments that answer it, or that nothing does.
    /// For an app whose chat does more than talk about films.
    pub fn answer(&self, question: &str, waker: &UiWaker) -> ChatMessage {
        let answers: Vec<Answer> = self.library.ask(question, MOMENTS);
        if answers.is_empty() {
            return ChatMessage::assistant(self.labels.nothing_found.clone());
        }
        let message = if let Some(writer) = &self.writer {
            let moments: Vec<Moment> = answers.iter().map(|answer| answer.moment.clone()).collect();
            let text = Arc::new(Mutex::new(String::new()));
            writer(question, &moments, text.clone(), waker.clone(), self.labels.found.clone());
            ChatMessage::assistant_writing(text)
        } else {
            ChatMessage::assistant(self.labels.found.clone())
        };
        let (library, on_open) = (self.library.clone(), self.on_open.clone());
        message.card(move || {
            let rows: Vec<ListItem> = answers
                .iter()
                .map(|answer| {
                    let on_open = on_open.clone();
                    moment_row(&library, answer, move |moment| on_open(moment))
                })
                .collect();
            Box::new(List::new(rows))
        })
    }
}

/// What answering a question about the films takes. See [`MovieAnswers::answer`].
#[derive(Clone)]
pub struct MovieAnswers {
    /// The films.
    pub library: MovieLibrary,
    /// The words of the answers.
    pub labels: MovieChatLabels,
    /// What opening a moment means in the app.
    pub on_open: Rc<dyn Fn(&Moment)>,
    /// Who writes the answer in its own words, if anyone.
    pub writer: Option<MovieWriter>,
}

impl std::fmt::Debug for MovieAnswers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MovieAnswers").field("library", &self.library).finish_non_exhaustive()
    }
}

impl View for MovieChat {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let answering = MovieAnswers {
            library: self.library.clone(),
            labels: self.labels.clone(),
            on_open: self.on_open.clone(),
            writer: self.writer.clone(),
        };
        let (messages, waker) = (self.messages.clone(), cx.ui_waker());
        let chat = Chat::new(self.messages.clone())
            .placeholder(self.labels.placeholder.clone())
            .intro(self.labels.intro.clone())
            .examples(self.examples.clone())
            .on_send(move |question| {
                let reply = answering.answer(&question, &waker);
                messages.update(|all| all.push(reply));
            });
        chat.build(cx)
    }
}
