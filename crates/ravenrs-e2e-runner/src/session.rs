//! What a run drives and how each step goes: the app under test (in this process or apart, over
//! the test channel), the step a block stands for, why a step failed in words a person reads
//! (Portuguese, by the stable code) and how each block went.

use ravenrs_e2e::E2e;
use ravenrs_e2e_wire::{Block, Client, Command, Kind};

/// The app a run drives: compiled into the tool, or opened apart in a child process with no
/// window and driven over the test channel (`ravenrs-e2e-wire`). A step runs the same either way.
#[derive(Debug)]
pub enum Session {
    /// An app compiled into the tool, such as the sample form or another Studio.
    Here(Box<E2e>),
    /// An app opened in a child process, such as Raven Finanças; it ends with the run.
    Apart(Client),
}

impl Session {
    /// Runs `command` on the app, as `Command::apply` answers.
    pub fn run(&mut self, command: &Command) -> Result<Option<String>, ravenrs_e2e_wire::Failure> {
        match self {
            Self::Here(e2e) => command.apply(e2e),
            Self::Apart(client) => client.send(command),
        }
    }

    /// The screen of the app as text, when it can still say.
    pub fn screen(&mut self) -> Option<String> {
        self.run(&Command::Snapshot).ok().flatten()
    }
}

/// Opens the app a scenario's Open block names: `None` when there is no app by that name, the
/// reason when there is one and it could not be opened.
pub type Opener = fn(&str) -> Option<Result<Session, Box<Failure>>>;

/// One step a block stands for.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Opens the app under test by name, from nothing.
    Open(String),
    /// Anything done on the app once it is open: click, type, check or mention, with its target
    /// written as on the block (`#id` or the text shown).
    Do(Command),
}

impl Step {
    /// The step `block` stands for, or why it cannot run, said to whoever wrote it.
    pub fn of(block: &Block) -> Result<Self, String> {
        if block.kind == Kind::Open {
            return Ok(Self::Open(block.target.clone()));
        }
        if block.target.trim().is_empty() {
            return Err("Diga o alvo: #id ou o texto que aparece".to_owned());
        }
        let needs_text = matches!(
            block.kind,
            Kind::Type | Kind::Check | Kind::Mention | Kind::Fill | Kind::Select | Kind::Drag
        );
        if needs_text && block.text.is_none() {
            return Err(if block.kind == Kind::Drag {
                "Diga para onde arrastar: #de = #até".to_owned()
            } else {
                "Diga o texto depois do alvo: #alvo = texto".to_owned()
            });
        }
        block.command().map(Self::Do).ok_or_else(|| format!("Bloco desconhecido: {}", block.kind.word()))
    }
}

/// Why a block failed: the message, and from `ravenrs-e2e` its stable code, how to fix it and
/// what came close on the screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Failure {
    /// What went wrong, starting with the code when there is one: `[E2E_NOT_FOUND] …`.
    pub message: String,
    /// The stable code, such as `E2E_NOT_FOUND`.
    pub code: Option<String>,
    /// What the failure means, in Portuguese, from its code.
    pub meaning: Option<String>,
    /// The detail `ravenrs-e2e` gave: which query, what it found.
    pub detail: Option<String>,
    /// How to fix it, when there is an obvious way.
    pub fix: Option<String>,
    /// Up to three nodes that came close to what the block asked for.
    pub close: Vec<String>,
    /// The screen of the app when it failed, when the failure brought it.
    pub screen: Option<String>,
}

impl Failure {
    /// A failure with only a message, for what is not a step of the app: no app to open, a
    /// block that cannot run.
    pub fn plain(message: impl Into<String>) -> Self {
        Self { message: message.into(), ..Self::default() }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

/// What a failure means and how to fix it, in Portuguese, by its stable code. The detail from
/// `ravenrs-e2e` (which query, what was found) follows the meaning.
fn in_portuguese(code: &str) -> (&'static str, Option<&'static str>) {
    match code {
        "E2E_NOT_FOUND" => (
            "nada na tela corresponde ao alvo",
            Some("confira a tela do app testado; uma parte sem nome pode ganhar um ViewExt::test_id"),
        ),
        "E2E_AMBIGUOUS" => (
            "mais de uma parte corresponde ao alvo",
            Some("dê um test_id à parte, restrinja com within ou escolha uma com nth"),
        ),
        "E2E_DISABLED" => (
            "o alvo está desabilitado",
            Some("faça antes o que o habilita, ou confira que está desabilitado"),
        ),
        "E2E_COVERED" => {
            ("outra coisa está por cima do alvo", Some("feche ou mova antes o que está por cima"))
        }
        "E2E_NOT_VISIBLE" => ("o alvo está fora da janela", Some("role até ele ou aumente a janela")),
        "E2E_PROTECTED_VALUE" => {
            ("o valor é secreto e nunca é mostrado", Some("confira o que o app faz com o valor, como entrar"))
        }
        "E2E_EXPECTATION" => ("a conferência não bateu", None),
        "E2E_APP_BUILD" => (
            "o app testado não compilou",
            Some("corrija os erros do Cargo/Rust no arquivo e na linha indicados e rode o cenário de novo"),
        ),
        "E2E_APP_START" => (
            "o app testado não abriu ou fechou no meio",
            Some(
                "veja no fim da mensagem o que o app escreveu ao sair; confira que ele foi compilado com o canal de teste",
            ),
        ),
        "E2E_TIMEOUT" => (
            "o passo excedeu o tempo limite",
            Some(
                "confira no detalhe se a condição esperada não ficou pronta ou se o canal deixou de responder",
            ),
        ),
        "E2E_WIRE" => ("o app testado respondeu fora do protocolo do canal de teste", None),
        "E2E_NO_FILM" => (
            "o app testado não filmou",
            Some(
                "sirva o app com ravenrs_e2e_wire::serve_filmed e ravenrs_movie::ChannelFilm, ou rode sem filmar",
            ),
        ),
        "E2E_NO_FRAME" => (
            "o app testado não forneceu o quadro vetorial",
            Some("sirva o app com ravenrs_movie::ChannelFilm para fornecer quadros pelo canal"),
        ),
        "E2E_NO_WINDOW" => (
            "o app testado não pôde mostrar a janela",
            Some("rode sem --headed, ou atualize o app para o canal com janela, numa sessão com tela"),
        ),
        "E2E_WINDOW_CLOSED" => (
            "a janela do app foi fechada durante o teste",
            Some("deixe a janela aberta até o fim, ou pare o teste com q no terminal"),
        ),
        "E2E_NO_SOURCE" => (
            "o alvo não tem origem declarada",
            Some("ligue e2e-source no app testado e marque o alvo com test_id"),
        ),
        _ => ("o passo falhou", None),
    }
}

impl From<ravenrs_e2e_wire::Failure> for Failure {
    fn from(f: ravenrs_e2e_wire::Failure) -> Self {
        let (meaning, fix) = in_portuguese(&f.code);
        Self {
            message: format!("[{}] {meaning}: {}", f.code, f.message),
            meaning: Some(meaning.to_owned()),
            detail: Some(f.message),
            fix: fix.map(str::to_owned).or(f.fix),
            close: f.close,
            screen: (!f.screen.is_empty()).then_some(f.screen),
            code: Some(f.code),
        }
    }
}

/// The failure of an Open block that names no app this run knows.
fn no_app(name: &str) -> Failure {
    let (meaning, _) = in_portuguese("E2E_APP_START");
    Failure {
        message: format!("[E2E_APP_START] {meaning}: não há app chamado {name:?} para abrir"),
        code: Some("E2E_APP_START".to_owned()),
        meaning: Some(meaning.to_owned()),
        detail: Some(format!("não há app chamado {name:?}")),
        fix: Some(
            "o nome no bloco abrir é o app que um pacote declara em [package.metadata.ravenrs.e2e]; rode dentro do projeto dele, ou o abra pelo Studio, que tem apps compilados nele"
                .to_owned(),
        ),
        ..Failure::default()
    }
}

/// Runs `step` on the app in `session`, opening apps with `open`.
pub fn perform(session: &mut Option<Session>, step: &Step, open: Opener) -> Result<(), Box<Failure>> {
    let command = match step {
        Step::Open(name) => {
            // The app a previous Open block started ends before the next one starts.
            *session = None;
            let opened = open(name).ok_or_else(|| Box::new(no_app(name)))?;
            *session = Some(opened?);
            return Ok(());
        }
        Step::Do(command) => command,
    };
    let Some(app) = session.as_mut() else {
        return Err(Box::new(Failure::plain("O app não está aberto: comece o cenário por um bloco Abrir")));
    };
    app.run(command).map(|_| ()).map_err(|f| Box::new(Failure::from(f)))
}

/// What a step of a run did, for the log and for whoever called the Studio.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The block's id in the scenario.
    pub id: String,
    /// The block's kind: `open`, `click`, `type`, `check` or `mention`.
    pub kind: String,
    /// The block's title.
    pub title: String,
    /// The block's target and text, as written on it.
    pub line: String,
    /// `None` when it passed, else what went wrong, starting with its code when it has one.
    pub error: Option<String>,
    /// The stable code of the failure, such as `E2E_NOT_FOUND`, when it has one.
    pub code: Option<String>,
    /// What the failure means, in Portuguese, from its code.
    pub meaning: Option<String>,
    /// The detail `ravenrs-e2e` gave: which query, what it found.
    pub detail: Option<String>,
    /// How to fix the failure, when there is an obvious way.
    pub fix: Option<String>,
    /// Up to three nodes that came close to what the block asked for.
    pub close: Vec<String>,
    /// The screen of the app under test when the block failed, as `E2e::snapshot` writes it.
    pub screen: Option<String>,
    /// How long the block took, in milliseconds, waits included.
    pub ms: u64,
}

/// One line that sums up a run: how many blocks passed out of the path.
pub fn summary(outcomes: &[Outcome], total: usize) -> String {
    let passed = outcomes.iter().filter(|o| o.error.is_none()).count();
    match outcomes.iter().position(|o| o.error.is_some()) {
        Some(i) => format!("Falhou no passo {} de {total}", i + 1),
        None if passed == total => format!("{passed} de {total} passos passaram"),
        None => format!("{passed} de {total} passos até agora"),
    }
}
