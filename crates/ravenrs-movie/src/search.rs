//! Finds the moment of a film that answers a question, in the words the films
//! already hold. No model and no network: it works the same in the browser.

use std::rc::Rc;

use ravenrs_text::fold;

use crate::movie::{HotspotKind, Movie};

/// How much a word counts in each part of a moment.
const TITLE_WEIGHT: f32 = 2.0;
const TEXT_WEIGHT: f32 = 1.0;
const CHAPTER_WEIGHT: f32 = 0.5;
/// The usual BM25 constants: how fast repeating a word stops helping, and how
/// much a long text is discounted.
const SATURATION: f32 = 1.2;
const LENGTH_DISCOUNT: f32 = 0.75;
/// A chapter begins the lesson, so it goes ahead of a step that says the same.
const CHAPTER_BONUS: f32 = 1.25;
/// How much of the shorter word a shared start has to cover: "compra" is not
/// "computador".
const MOST_OF_THE_WORD: f32 = 0.7;
/// An answer far weaker than the best one is noise.
const FAR_BELOW_THE_BEST: f32 = 0.25;
/// Words that say nothing about the subject, folded, in Portuguese and English.
const STOP_WORDS: &[&str] = &[
    "a", "ao", "aos", "as", "com", "como", "da", "das", "de", "do", "dos", "e", "em", "eu", "faco", "fazer",
    "foi", "ha", "isso", "mais", "meu", "meus", "minha", "minhas", "na", "nas", "no", "nos", "o", "onde",
    "os", "ou", "outra", "outras", "outro", "outros", "para", "pelo", "pela", "por", "posso", "pra", "qual",
    "quando", "que", "quero", "se", "sem", "seu", "sua", "um", "uma", "uns", "umas", "an", "and", "are",
    "can", "does", "for", "how", "i", "in", "is", "it", "my", "of", "on", "or", "the", "to", "what", "where",
    "with",
];

/// What a moment of a film is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MomentKind {
    /// The start of a chapter.
    Chapter,
    /// A step that waits for a click.
    Step,
    /// A note on the screen.
    Note,
}

/// A place in a film that can be asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moment {
    /// The film, by its position in the library.
    pub film: usize,
    /// When, in milliseconds of the film's timeline.
    pub at_ms: u64,
    /// What it is.
    pub kind: MomentKind,
    /// Its title.
    pub title: String,
    /// What it says.
    pub text: String,
}

/// A moment that answers a question.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    /// The moment.
    pub moment: Moment,
    /// How well it answers, comparable only within one search.
    pub score: f32,
}

/// A moment with its text ready to compare.
#[derive(Debug)]
struct Indexed {
    moment: Moment,
    title: Vec<String>,
    text: Vec<String>,
    chapter: Vec<String>,
}

impl Indexed {
    fn length(&self) -> usize {
        self.title.len() + self.text.len()
    }

    /// How much `word` appears here, counting near words for less.
    fn presence(&self, word: &str) -> f32 {
        let in_field = |words: &[String]| words.iter().map(|other| likeness(word, other)).sum::<f32>();
        in_field(&self.title) * TITLE_WEIGHT
            + in_field(&self.text) * TEXT_WEIGHT
            + in_field(&self.chapter) * CHAPTER_WEIGHT
    }
}

#[derive(Debug)]
struct Library {
    films: Vec<(String, Movie)>,
    moments: Vec<Indexed>,
    average_length: f32,
}

/// The films of an app, ready to be asked about. Cloning is cheap.
///
/// ```
/// use ravenrs_core::{Color, Size};
/// use ravenrs_movie::{FilmBuilder, MovieLibrary};
///
/// let mut film = FilmBuilder::new(Size::new(320.0, 200.0), Color::WHITE);
/// film.frame(0, &[]);
/// film.chapter("Pagar uma conta", "Como dar baixa no que venceu.", 0);
/// let library = MovieLibrary::new().film("Manual", film.finish(1000)?);
/// assert_eq!(library.ask("como pago as contas?", 3)[0].moment.title, "Pagar uma conta");
/// # Ok::<(), ravenrs_movie::MovieError>(())
/// ```
#[derive(Clone, Debug)]
pub struct MovieLibrary {
    inner: Rc<Library>,
}

impl MovieLibrary {
    /// A library with no films.
    pub fn new() -> Self {
        Self { inner: Rc::new(Library { films: Vec::new(), moments: Vec::new(), average_length: 0.0 }) }
    }

    /// The library with one more film. Build it once: this reads all the text.
    #[must_use]
    pub fn film(self, title: impl Into<String>, movie: Movie) -> Self {
        let mut films: Vec<(String, Movie)> = self.inner.films.clone();
        films.push((title.into(), movie));
        let moments: Vec<Indexed> =
            films.iter().enumerate().flat_map(|(i, (_, movie))| moments_of(i, movie)).collect();
        let total: usize = moments.iter().map(Indexed::length).sum();
        let average_length = total as f32 / moments.len().max(1) as f32;
        Self { inner: Rc::new(Library { films, moments, average_length }) }
    }

    /// The film at `index`, with its title.
    pub fn get(&self, index: usize) -> Option<(&str, &Movie)> {
        self.inner.films.get(index).map(|(title, movie)| (title.as_str(), movie))
    }

    /// How many films it has.
    pub fn len(&self) -> usize {
        self.inner.films.len()
    }

    /// Whether it has no films.
    pub fn is_empty(&self) -> bool {
        self.inner.films.is_empty()
    }

    /// The moments that best answer `question`, the best first, at most `limit`.
    /// Accents and case do not matter, and near forms of a word count for less:
    /// "pago" finds "pagar". With no word in common, there is no answer.
    pub fn ask(&self, question: &str, limit: usize) -> Vec<Answer> {
        let library = &self.inner;
        let words = question_words(question);
        if words.is_empty() {
            return Vec::new();
        }
        let total = library.moments.len() as f32;
        let mut scores = vec![(0.0_f32, 0_usize); library.moments.len()];
        for word in &words {
            let presences: Vec<f32> = library.moments.iter().map(|m| m.presence(word)).collect();
            let having = presences.iter().filter(|p| **p > 0.0).count() as f32;
            let rarity = (1.0 + (total - having + 0.5) / (having + 0.5)).ln();
            for ((score, matched), (presence, moment)) in
                scores.iter_mut().zip(presences.iter().zip(&library.moments))
            {
                if *presence > 0.0 {
                    let length = moment.length() as f32 / library.average_length.max(1.0);
                    let discount = 1.0 - LENGTH_DISCOUNT + LENGTH_DISCOUNT * length;
                    *score += rarity * presence * (SATURATION + 1.0) / (presence + SATURATION * discount);
                    *matched += 1;
                }
            }
        }
        let mut answers: Vec<Answer> = library
            .moments
            .iter()
            .zip(scores)
            .filter(|(_, (score, _))| *score > 0.0)
            .map(|(indexed, (score, matched))| Answer {
                moment: indexed.moment.clone(),
                // Answering every word beats answering one word very well.
                score: score * matched as f32 / words.len() as f32
                    * if indexed.moment.kind == MomentKind::Chapter { CHAPTER_BONUS } else { 1.0 },
            })
            .collect();
        // On a tie the chapter wins, and then the earlier.
        answers.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| rank(a.moment.kind).cmp(&rank(b.moment.kind)))
                .then_with(|| (a.moment.film, a.moment.at_ms).cmp(&(b.moment.film, b.moment.at_ms)))
        });
        let best = answers.first().map_or(0.0, |answer| answer.score);
        answers.retain(|answer| answer.score >= best * FAR_BELOW_THE_BEST);
        answers.truncate(limit);
        answers
    }
}

impl Default for MovieLibrary {
    fn default() -> Self {
        Self::new()
    }
}

fn rank(kind: MomentKind) -> u8 {
    match kind {
        MomentKind::Chapter => 0,
        MomentKind::Step => 1,
        MomentKind::Note => 2,
    }
}

fn moments_of(film: usize, movie: &Movie) -> Vec<Indexed> {
    let chapters = movie.chapters().iter().map(|chapter| {
        let moment = Moment {
            film,
            at_ms: chapter.at_ms,
            kind: MomentKind::Chapter,
            title: chapter.title.clone(),
            text: chapter.text.clone(),
        };
        indexed(moment, Vec::new())
    });
    let spots = movie.hotspots().iter().map(|spot| {
        let moment = Moment {
            film,
            at_ms: spot.from_ms,
            kind: if spot.kind == HotspotKind::Stop { MomentKind::Step } else { MomentKind::Note },
            title: spot.title.clone(),
            text: spot.text.clone(),
        };
        let chapter = movie.chapter_at(spot.from_ms).and_then(|i| movie.chapters().get(i));
        indexed(moment, chapter.map_or_else(Vec::new, |chapter| words_of(&chapter.title)))
    });
    chapters.chain(spots).collect()
}

fn indexed(moment: Moment, chapter: Vec<String>) -> Indexed {
    let (title, text) = (words_of(&moment.title), words_of(&moment.text));
    Indexed { moment, title, text, chapter }
}

/// The folded words of a text, leaving out the ones that say nothing.
fn words_of(text: &str) -> Vec<String> {
    all_words(text).into_iter().filter(|word| !STOP_WORDS.contains(&word.as_str())).collect()
}

/// The words of a question. A question made only of small words still asks.
fn question_words(question: &str) -> Vec<String> {
    let words = words_of(question);
    if words.is_empty() { all_words(question) } else { words }
}

fn all_words(text: &str) -> Vec<String> {
    let folded: String = text.chars().flat_map(fold).collect();
    folded.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}

/// How close two folded words are, from 0 to 1.
fn likeness(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let (shorter, longer) = (a.len().min(b.len()), a.len().max(b.len()));
    let singular = |word: &[char]| -> usize { word.len() - usize::from(word.last() == Some(&'s')) };
    if a[..singular(&a)] == b[..singular(&b)] {
        return 0.95;
    }
    if shorter >= 5 && one_edit_apart(&a, &b) {
        return 0.7;
    }
    if root(&a) == root(&b) {
        return 0.6;
    }
    // A long shared start: "pagar" and "pagamento", or a word still being typed.
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if common >= 4 && common as f32 >= shorter as f32 * MOST_OF_THE_WORD {
        0.3 + 0.5 * common as f32 / longer as f32
    } else {
        0.0
    }
}

/// A word without the plural and the usual endings of verbs, so "pago", "pague"
/// and "pagar" meet. This is not a real stemmer: it only has to bring near forms
/// together, and a root under three letters is left alone.
fn root(word: &[char]) -> &[char] {
    let word = word.strip_suffix(&['s']).unwrap_or(word);
    let ending = [
        &['i', 'n', 'g'][..],
        &['a', 'r'],
        &['e', 'r'],
        &['i', 'r'],
        &['o', 'u'],
        &['e', 'i'],
        &['u', 'e'],
        &['e', 'd'],
        &['a'],
        &['e'],
        &['i'],
        &['o'],
    ]
    .into_iter()
    .find(|ending| word.ends_with(ending) && word.len() >= ending.len() + 3);
    ending.map_or(word, |ending| &word[..word.len() - ending.len()])
}

/// One letter wrong, missing, extra, or two swapped: the usual slips of typing.
fn one_edit_apart(a: &[char], b: &[char]) -> bool {
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let start = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let (rest_a, rest_b) = (&a[start..], &b[start..]);
    let tail = |skip_a: usize, skip_b: usize| rest_a.get(skip_a..) == rest_b.get(skip_b..);
    let swapped = rest_a.len() >= 2
        && rest_b.len() >= 2
        && rest_a[0] == rest_b[1]
        && rest_a[1] == rest_b[0]
        && tail(2, 2);
    tail(1, 1) || tail(1, 0) || tail(0, 1) || swapped
}

#[cfg(test)]
mod tests {
    use super::{all_words, likeness, words_of};

    #[test]
    fn words_are_folded_and_small_words_go_away() {
        assert_eq!(all_words("Lançar: uma DESPESA!"), ["lancar", "uma", "despesa"]);
        assert_eq!(words_of("Como lançar uma despesa?"), ["lancar", "despesa"]);
    }

    #[test]
    fn near_words_count_less_than_the_same_word() {
        assert_eq!(likeness("conta", "conta"), 1.0);
        assert!(likeness("conta", "contas") > 0.9);
        assert!(likeness("despeza", "despesa") > 0.5, "one wrong letter");
        assert!(likeness("despsea", "despesa") > 0.5, "two swapped letters");
        assert!(likeness("pagar", "pague") > 0.0, "the same root");
        assert!(likeness("pagar", "pagamento") > 0.0, "the same root, longer");
        assert_eq!(likeness("carro", "conta"), 0.0);
        assert!(likeness("salvo", "salvar") > 0.0, "the same root");
        assert!(likeness("saving", "save") > 0.0, "the same root");
        assert!(likeness("lanc", "lancamentos") > 0.0, "still being typed");
        assert_eq!(likeness("saldo", "salvar"), 0.0, "sharing a start is not sharing a meaning");
        assert_eq!(likeness("saldo", "salve"), 0.0);
        assert_eq!(likeness("compra", "computador"), 0.0);
    }
}
