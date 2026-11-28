//! Closed records: a sealed lesson that changes only by a justified correction (#856), a closing
//! amendment that ends an enrollment, and a term approved with its document's hash, which freezes
//! its lessons until it is reopened with a reason (#855). Everything runs on a real
//! `MemoryDatabase`, so what the page says is what the commit refused or kept.
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{
    Amendment, EntityInfo, FieldInfo, FieldKind, MemoryAttachmentStore, MemoryDatabase, ObjectSpace,
    PeriodClosing, Seal, Value, approve, closing_versions, reopen,
};

/// The control registered in the center.
pub const ID: &str = "ClosingDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Fechamento de período";

const TERM: f64 = 1.0;
const LESSON: f64 = 2.0;
const ENROLLMENT: f64 = 3.0;

fn closing() -> PeriodClosing {
    PeriodClosing::new("TermClosing")
        .freezes("Lesson", "[term_id]")
        .requires_none("Lesson", "[status] = 'draft'")
        .reopen_reason(10)
}

fn entities() -> Vec<EntityInfo> {
    let term = EntityInfo::new("Term")
        .caption("Semestre")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text))
        .period_closing(closing());
    let versions = closing().versions_entity(&term);
    vec![
        term,
        versions,
        EntityInfo::new("Lesson")
            .caption("Aula")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("term_id", FieldKind::Reference("Term".into())))
            .field(FieldInfo::new("status", FieldKind::Text))
            .field(FieldInfo::new("present", FieldKind::Integer))
            .field(FieldInfo::new("corrected", FieldKind::Boolean))
            .sealed(Seal::when("[status] = 'closed'")),
        EntityInfo::new("LessonCorrection")
            .caption("Correção de aula")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("lesson_id", FieldKind::Reference("Lesson".into())))
            .field(FieldInfo::new("present", FieldKind::Integer))
            .field(FieldInfo::new("reason", FieldKind::LongText))
            .field(FieldInfo::new("previous", FieldKind::LongText))
            .amends(
                Amendment::correction("Lesson", "lesson_id")
                    .copies(["present"])
                    .reason("reason", 10)
                    .before("previous")
                    .marks("corrected"),
            ),
        EntityInfo::new("Enrollment")
            .caption("Matrícula")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("class", FieldKind::Text))
            .field(FieldInfo::new("ended", FieldKind::Boolean))
            .sealed(Seal::when("[ended] = true")),
        EntityInfo::new("Transfer")
            .caption("Transferência")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("enrollment_id", FieldKind::Reference("Enrollment".into())))
            .field(FieldInfo::new("to_class", FieldKind::Text))
            .field(FieldInfo::new("reason", FieldKind::LongText))
            .amends(
                Amendment::closing("Enrollment", "enrollment_id").reason("reason", 10).marks("ended").apply(
                    |transfer, _origin, space| {
                        let mut next = space.new_object("Enrollment").map_err(|e| e.to_string())?;
                        next.set("class", transfer.get("to_class"));
                        next.set("ended", false);
                        space.insert(next).map_err(|e| e.to_string())
                    },
                ),
            ),
    ]
}

/// The database of the page and what the last step said.
#[derive(Clone)]
struct Demo {
    database: MemoryDatabase,
    files: Rc<MemoryAttachmentStore>,
    said: String,
}

impl Demo {
    fn new() -> Self {
        let database = MemoryDatabase::new(entities()).unwrap_or_else(|e| panic!("the page's model: {e}"));
        let mut space = database.space();
        let mut put = |entity: &str, values: &[(&str, Value)]| {
            if let Ok(mut object) = space.new_object(entity) {
                for (field, value) in values {
                    object.set(field, value.clone());
                }
                let _ = space.insert(object);
            }
        };
        put("Term", &[("id", Value::Number(TERM)), ("name", "1º semestre".into())]);
        put(
            "Lesson",
            &[
                ("id", Value::Number(LESSON)),
                ("term_id", Value::Number(TERM)),
                ("status", "draft".into()),
                ("present", Value::Number(3.0)),
                ("corrected", Value::Bool(false)),
            ],
        );
        put(
            "Enrollment",
            &[("id", Value::Number(ENROLLMENT)), ("class", "Juniores".into()), ("ended", Value::Bool(false))],
        );
        let _ = space.commit();
        Self { database, files: Rc::new(MemoryAttachmentStore::default()), said: "Pronto.".to_owned() }
    }

    fn answer(&mut self, done: Result<String, String>) {
        self.said = match done {
            Ok(what) => what,
            Err(why) => format!("Recusado: {why}"),
        };
    }

    /// Writes the lesson directly, as a form would.
    fn lesson_to(&mut self, status: &str, present: f64) {
        let mut space = self.database.space();
        let done = space
            .find("Lesson", &Value::Number(LESSON))
            .map_err(|e| e.to_string())
            .and_then(|found| found.ok_or_else(|| "a aula sumiu".to_owned()))
            .and_then(|mut lesson| {
                lesson.set("status", status);
                lesson.set("present", present);
                space.save(lesson).and_then(|()| space.commit()).map_err(|e| e.to_string())
            })
            .map(|()| format!("Aula gravada: {status}, {present} presentes."));
        self.answer(done);
    }

    fn correct(&mut self, reason: &str) {
        let mut space = self.database.space();
        let done = space
            .new_object("LessonCorrection")
            .and_then(|mut correction| {
                correction.set("lesson_id", Value::Number(LESSON));
                correction.set("present", 5.0);
                correction.set("reason", reason);
                space.insert(correction)?;
                space.commit()
            })
            .map(|()| "Correção gravada na aula, no mesmo commit.".to_owned())
            .map_err(|e| e.to_string());
        self.answer(done);
    }

    fn transfer(&mut self) {
        let mut space = self.database.space();
        let done = space
            .new_object("Transfer")
            .and_then(|mut transfer| {
                transfer.set("enrollment_id", Value::Number(ENROLLMENT));
                transfer.set("to_class", "Adolescentes");
                transfer.set("reason", "mudou de faixa etária");
                space.insert(transfer)?;
                space.commit()
            })
            .map(|()| "Matrícula encerrada e nova matrícula criada.".to_owned())
            .map_err(|e| e.to_string());
        self.answer(done);
    }

    fn approve(&mut self) {
        let mut space = self.database.space();
        let document = format!("Relatório do 1º semestre, versão {}", self.versions().len() + 1);
        let done =
            approve(&mut space, &*self.files, "Term", &Value::Number(TERM), document.as_bytes(), "ana")
                .map(|version| {
                    format!("Semestre aprovado: versão {}.", version.integer("version").unwrap_or_default())
                })
                .map_err(|e| e.to_string());
        self.answer(done);
    }

    fn reopen(&mut self, reason: &str) {
        let mut space = self.database.space();
        let done = reopen(&mut space, "Term", &Value::Number(TERM), reason)
            .map(|_| "Semestre reaberto.".to_owned())
            .map_err(|e| e.to_string());
        self.answer(done);
    }

    fn versions(&self) -> Vec<ravenrs_app::Object> {
        closing_versions(&self.database.space(), "Term", &Value::Number(TERM)).unwrap_or_default()
    }

    fn lines(&self) -> Vec<String> {
        let space = self.database.space();
        let lesson = space.find("Lesson", &Value::Number(LESSON)).ok().flatten();
        let mut lines = Vec::new();
        if let Some(lesson) = lesson {
            let present = lesson.integer("present").unwrap_or_default();
            let corrected = if lesson.get("corrected") == Value::Bool(true) { ", corrigida" } else { "" };
            lines.push(format!("Aula: {}, {present} presentes{corrected}", lesson.text("status")));
        }
        let ended = space
            .query("Enrollment", &ravenrs_app::Query::all())
            .map(|all| all.iter().filter(|e| e.get("ended") == Value::Bool(true)).count())
            .unwrap_or_default();
        lines.push(format!("Matrículas encerradas: {ended}"));
        let versions = self.versions();
        if versions.is_empty() {
            lines.push("Versões: nenhuma".to_owned());
        }
        for version in &versions {
            let status = if version.text("status") == "current" { "vigente" } else { "reaberta" };
            let digest: String = version.text("digest").chars().take(12).collect();
            let reason = version.text("reopen_reason");
            let reason = if reason.is_empty() { String::new() } else { format!(" ({reason})") };
            lines.push(format!(
                "Versão {}: {status}, hash {digest}…{reason}",
                version.integer("version").unwrap_or_default()
            ));
        }
        let fields: Vec<String> =
            closing().versions_entity(&entities()[0]).fields.iter().map(|field| field.name.clone()).collect();
        lines.push(format!("Campos das versões: {}", fields.join(", ")));
        lines
    }
}

/// Every public feature of PeriodClosing and Amendment on one page.
#[derive(Debug, Default)]
pub struct ClosingPage;

impl View for ClosingPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let demo = cx.use_state(Demo::new);
        let now = demo.get();
        let step = |label: &str, run: fn(&mut Demo)| {
            let demo = demo.clone();
            Button::new(label).on_click(move || demo.update(run))
        };
        let lines: Vec<Text> = now.lines().into_iter().map(Text::new).collect();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Uma aula fechada só muda por correção justificada, no mesmo commit. Um semestre aprovado guarda o documento com o hash numa versão numerada e congela as aulas até ser reaberto com justificativa.")
                    .foreground(Semantic::InkMuted),
                HStack::new((
                    step("Fechar a aula", |d| d.lesson_to("closed", 3.0)),
                    step("Mudar a aula direto", |d| d.lesson_to("closed", 9.0)),
                    step("Corrigir sem motivo", |d| d.correct("errei")),
                    step("Corrigir a aula", |d| d.correct("lista de presença errada")),
                    step("Transferir", Demo::transfer),
                ))
                .spacing(8.0),
                HStack::new((
                    step("Aprovar o semestre", Demo::approve),
                    step("Reabrir sem motivo", |d| d.reopen("curto")),
                    step("Reabrir o semestre", |d| d.reopen("faltou uma presença")),
                    step("Recomeçar", |d| *d = Demo::new()),
                ))
                .spacing(8.0),
                Text::new(now.said.clone()),
                VStack::new(lines).spacing(4.0).align(Align::Start),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
