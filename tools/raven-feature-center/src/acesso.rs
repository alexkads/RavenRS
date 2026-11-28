//! Access for a while: an administrator grants support a user with a reason and a deadline, the
//! clock of the page moves past it and the user is turned off by themselves, or the administrator
//! revokes it before (#857). Everything runs on a real `MemoryDatabase` and `UserDirectory`.
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use ravenrs::prelude::*;
use ravenrs_app::{
    Authenticator, Credentials, Identity, MemoryDatabase, ObjectSpace, PermissionPolicy, Query, Role,
    RoleMerge, SECURITY_TEMPORARY_ACCESS, SECURITY_USER, SecurityContext, SpaceMaker, TemporaryAccessPolicy,
    UserDirectory, Value, security_entities,
};
use ravenrs_criteria::DateTime;

/// The control registered in the center.
pub const ID: &str = "TemporaryAccessDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Acesso temporário";

/// The database of the page, its administrator, how far its clock moved and what was said.
#[derive(Clone)]
struct Demo {
    database: MemoryDatabase,
    directory: UserDirectory,
    admin: Identity,
    hours: Rc<Cell<u32>>,
    last: Option<(String, String, Value)>,
    said: String,
}

impl Demo {
    fn new() -> Self {
        let database =
            MemoryDatabase::new(security_entities()).unwrap_or_else(|e| panic!("the page's model: {e}"));
        let raw = database.clone();
        let maker: SpaceMaker = Rc::new(move || Box::new(raw.space()) as Box<dyn ObjectSpace>);
        let support = Role::new("Suporte").policy(PermissionPolicy::ReadOnlyAll);
        let directory =
            UserDirectory::new(maker, vec![support]).unwrap_or_else(|e| panic!("the page's roles: {e}"));
        let admin = directory
            .create_first_administrator("ana", "correct horse")
            .unwrap_or_else(|e| panic!("the page's administrator: {e}"));
        Self {
            database,
            directory,
            admin,
            hours: Rc::new(Cell::new(0)),
            last: None,
            said: "Pronto.".to_owned(),
        }
    }

    /// The policy of the page, whose clock is now plus the hours the page moved it by.
    fn policy(&self) -> TemporaryAccessPolicy {
        let hours = self.hours.clone();
        TemporaryAccessPolicy::new("Suporte")
            .max_hours(72)
            .min_reason(10)
            .name_prefix("suporte-")
            .check_every(Duration::from_secs(60))
            .clock(move || {
                let now = ravenrs_app::local_now();
                let seconds = i64::from(now.seconds) + i64::from(hours.get()) * 3600;
                DateTime {
                    days: now.days + seconds.div_euclid(86_400),
                    seconds: seconds.rem_euclid(86_400) as u32,
                }
            })
    }

    fn admin(&self) -> SecurityContext {
        SecurityContext::new(
            self.admin.clone(),
            self.directory.clone(),
            security_entities(),
            RoleMerge::AnyRole,
        )
    }

    fn grant(&mut self, reason: &str) {
        let policy = self.policy();
        self.said = match self.admin().grant_temporary_access(&policy, reason, 2) {
            Ok(access) => {
                let said = format!("Concedido a {} por 2 h (senha mostrada uma vez).", access.user_name);
                self.last = Some((access.user_name, access.password, access.user_key));
                said
            }
            Err(why) => format!("Recusado: {why}"),
        };
    }

    fn pass_hours(&mut self) {
        self.hours.set(self.hours.get() + 3);
        let now = self.policy().now();
        self.said = match self.directory.end_expired(now) {
            Ok(ended) => format!("Passaram 3 horas: {ended} acesso(s) vencido(s)."),
            Err(why) => format!("Recusado: {why}"),
        };
    }

    fn sign_in(&mut self) {
        let Some((name, password, _)) = &self.last else {
            self.said = "Conceda um acesso antes.".to_owned();
            return;
        };
        self.said = match self.directory.authenticate(&Credentials::new(name, password)) {
            Ok(identity) => format!("{} entrou.", identity.name),
            Err(_) => format!("{name} não entrou."),
        };
    }

    fn revoke(&mut self) {
        let Some((name, _, key)) = self.last.clone() else {
            self.said = "Conceda um acesso antes.".to_owned();
            return;
        };
        self.said = match self.admin().revoke_user(&key) {
            Ok(()) => format!("{name} revogado."),
            Err(why) => format!("Recusado: {why}"),
        };
    }

    fn revoke_self(&mut self) {
        let key = self.admin.key.clone();
        self.said = match self.admin().revoke_user(&key) {
            Ok(()) => "ana revogada.".to_owned(),
            Err(why) => format!("Recusado: {why}"),
        };
    }

    fn lines(&self) -> Vec<String> {
        let space = self.database.space();
        let users = space.query(SECURITY_USER, &Query::all()).unwrap_or_default();
        let mut lines: Vec<String> = users
            .iter()
            .map(|user| {
                let active = if user.get("active") == Value::Bool(true) { "ativo" } else { "inativo" };
                let until = if matches!(user.get("expires_at"), Value::Date(_)) { ", com prazo" } else { "" };
                format!("Usuário {}: {active}{until}", user.text("name"))
            })
            .collect();
        lines.sort();
        for access in space.query(SECURITY_TEMPORARY_ACCESS, &Query::all()).unwrap_or_default() {
            let ended = access.text("ended_reason");
            let state = if ended.is_empty() { "em aberto".to_owned() } else { format!("encerrado: {ended}") };
            lines.push(format!(
                "Acesso de {}: {}, por {}, {state}",
                access.text("granted_by"),
                access.text("reason"),
                "2 h"
            ));
        }
        lines
    }
}

/// Every public feature of TemporaryAccessPolicy on one page.
#[derive(Debug, Default)]
pub struct TemporaryAccessPage;

impl View for TemporaryAccessPage {
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
                Text::new("O administrador concede ao suporte um usuário com motivo e prazo. Vencido o prazo, ele não entra mais e a sessão aberta perde as permissões. Revogar desliga o usuário, as chaves de API e o acesso, nunca o próprio administrador.")
                    .foreground(Semantic::InkMuted),
                HStack::new((
                    step("Conceder sem motivo", |d| d.grant("curto")),
                    step("Conceder 2 horas", |d| d.grant("suporte do caixa")),
                    step("Entrar como suporte", Demo::sign_in),
                    step("Passar 3 horas", Demo::pass_hours),
                ))
                .spacing(8.0),
                HStack::new((
                    step("Revogar o suporte", Demo::revoke),
                    step("Revogar a si mesma", Demo::revoke_self),
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
