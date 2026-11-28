//! The summary of what falls due: an entity says its due date, what is pending, what names a
//! record and the amount, and each person gets what is overdue and what falls due in the days they
//! chose, with the amounts only if their role reads that field (#860). Today is fixed at
//! 21/11/2026, so the page reads the same every day.
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{
    Access, DigestPreference, DigestWords, DueDigest, EntityInfo, FieldInfo, FieldKind, Identity,
    MemoryDatabase, ObjectSpace, Operation, PermissionPolicy, Permissions, Role, RoleMerge,
    SecuredObjectSpace, SecurityContext, SpaceMaker, UserDirectory, Value, due_digest_for, security_entities,
};
use ravenrs_criteria::DateTime;

/// The control registered in the center.
pub const ID: &str = "DueDigestDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Resumo de vencimentos";

fn day(day: u32) -> DateTime {
    DateTime::from_ymd(2026, 11, day).unwrap_or(DateTime { days: 0, seconds: 0 })
}

fn invoice() -> EntityInfo {
    EntityInfo::new("Invoice")
        .caption("Contas")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("description", FieldKind::Text))
        .field(FieldInfo::new("due_on", FieldKind::Date))
        .field(FieldInfo::new("amount", FieldKind::Money))
        .field(FieldInfo::new("status", FieldKind::Text))
        .due_digest(
            DueDigest::new("due_on")
                .pending("[status] = 'open'")
                .label("description")
                .amount("amount")
                .words("conta", "contas")
                .named(3)
                .hidden_amounts("Os valores ficam com a Tesouraria."),
        )
}

fn entities() -> Vec<EntityInfo> {
    let mut all = vec![invoice()];
    all.extend(security_entities());
    all
}

/// The roles the page switches between.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Profile {
    Treasury,
    Office,
    Visitor,
}

impl Profile {
    fn role(self) -> Role {
        match self {
            Self::Treasury => Role::new("Tesouraria").policy(PermissionPolicy::ReadOnlyAll),
            Self::Office => Role::new("Secretaria").policy(PermissionPolicy::ReadOnlyAll).member_permission(
                "Invoice",
                ["amount"],
                None,
                [Operation::Read],
                Access::Deny,
            ),
            Self::Visitor => Role::new("Visitante").policy(PermissionPolicy::DenyAll),
        }
    }

    fn caption(self) -> &'static str {
        match self {
            Self::Treasury => "Tesouraria",
            Self::Office => "Secretaria, sem ler o valor",
            Self::Visitor => "Visitante, sem ler contas",
        }
    }
}

#[derive(Clone)]
struct Demo {
    database: MemoryDatabase,
    directory: UserDirectory,
    profile: Profile,
    preference: DigestPreference,
}

impl Demo {
    fn new() -> Self {
        let database = MemoryDatabase::new(entities()).unwrap_or_else(|e| panic!("the page's model: {e}"));
        let raw = database.clone();
        let maker: SpaceMaker = Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
        let directory =
            UserDirectory::new(maker, Vec::new()).unwrap_or_else(|e| panic!("the page's roles: {e}"));
        let mut space = database.space();
        for (id, name, due, cents, status) in [
            (1.0, "Luz", 20, 48_000.0, "open"),
            (2.0, "Água", 23, 9_000.0, "open"),
            (3.0, "Internet", 27, 12_000.0, "open"),
            (4.0, "Gás", 19, 7_000.0, "paid"),
            (5.0, "Aluguel", 18, 150_000.0, "open"),
            (6.0, "Seguro", 22, 30_000.0, "open"),
        ] {
            let mut record = invoice().new_object();
            record.set("id", id);
            record.set("description", name);
            record.set("due_on", Value::Date(day(due)));
            record.set("amount", cents);
            record.set("status", status);
            let _ = space.insert(record);
        }
        let _ = space.commit();
        Self { database, directory, profile: Profile::Treasury, preference: DigestPreference::default() }
    }

    fn lines(&self) -> Vec<String> {
        let identity = Identity::new("bia", "bia", vec![self.profile.role()]);
        let context = Rc::new(SecurityContext::new(
            identity.clone(),
            self.directory.clone(),
            entities(),
            RoleMerge::AnyRole,
        ));
        let space = SecuredObjectSpace::new(Box::new(self.database.space()), context);
        let permissions = Permissions::new(identity, entities(), RoleMerge::AnyRole);
        let mut lines = vec![
            format!("Perfil: {}", self.profile.caption()),
            format!(
                "Preferência: {}, {} dia(s) antes",
                if self.preference.enabled { "ligado" } else { "desligado" },
                self.preference.advance_days
            ),
        ];
        // The words of the catalog, with its money format, as the session takes them.
        let words = DigestWords::from_texts(&ravenrs_app_i18n::Texts::new(
            ravenrs_app_i18n::Language::BrazilianPortuguese,
        ));
        match due_digest_for(&space, &invoice(), &permissions, day(21), self.preference, &words) {
            Some(digest) => {
                lines.push(format!("Aviso: {}", digest.title));
                lines.push(format!("Texto: {}", digest.body));
            }
            None => lines.push("Aviso: nenhum".to_owned()),
        }
        lines
    }
}

/// Every public feature of DueDigest on one page.
#[derive(Debug, Default)]
pub struct DueDigestPage;

impl View for DueDigestPage {
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
                Text::new("Hoje é 21/11/2026. O aviso diz o que venceu e o que vence nos dias escolhidos, e só mostra valor a quem lê o campo de valor.")
                    .foreground(Semantic::InkMuted),
                HStack::new((
                    step("Tesouraria", |d| d.profile = Profile::Treasury),
                    step("Secretaria", |d| d.profile = Profile::Office),
                    step("Visitante", |d| d.profile = Profile::Visitor),
                ))
                .spacing(8.0),
                HStack::new((
                    step("No dia", |d| d.preference = DigestPreference::new(true, 0)),
                    step("3 dias antes", |d| d.preference = DigestPreference::new(true, 3)),
                    step("7 dias antes", |d| d.preference = DigestPreference::new(true, 7)),
                    step("Desligar", |d| d.preference = DigestPreference::new(false, d.preference.advance_days)),
                ))
                .spacing(8.0),
                VStack::new(lines).spacing(4.0).align(Align::Start),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
