//! The door of a secured application, with every feature of `SecuredApp` (#382): a password
//! policy of the application's own, how many wrong passwords make an account wait and for how
//! long, a session that ends when nobody does anything, a door arranged by the application, a
//! view beside it, a button that starts over, how roles come together and access for a while.
//! It runs a real `SecuredApp` over its own `MemoryDatabase`, inside the page.
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use ravenrs::prelude::*;
use ravenrs_app::{
    EntityInfo, FieldInfo, FieldKind, MemoryDatabase, PermissionPolicy, Role, RoleMerge,
    TemporaryAccessPolicy, security_entities,
};
use ravenrs_app_ui::{App, DoorParts, Language, SecuredApp, SignInAction};

/// The control registered in the center.
pub const ID: &str = "SecureDoorDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Entrada segura";
/// How long the page's session lasts with nothing done, in seconds.
pub const IDLE_SECONDS: u64 = 120;

/// What the page asks of a password: a digit, and not the person's name.
fn policy(name: &str, password: &str) -> Result<(), String> {
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("Use ao menos um número.".to_owned());
    }
    if !name.is_empty() && password.to_lowercase().contains(&name.to_lowercase()) {
        return Err("A senha não pode conter o nome.".to_owned());
    }
    Ok(())
}

fn notes() -> EntityInfo {
    EntityInfo::new("DoorNote")
        .caption("Recados")
        .navigation("Portaria")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("text", FieldKind::Text).caption("Recado").default_property())
}

/// The door as the page arranges it: its own words, the fields and Sign in beside Start over.
fn door(parts: &DoorParts) -> AnyView {
    let mut row: Vec<AnyView> = vec![Box::new(parts.sign_in_button())];
    row.extend(parts.actions().into_iter().map(|button| Box::new(button) as AnyView));
    Box::new(
        VStack::new((
            Text::new("PORTARIA DE DEMONSTRAÇÃO").style(TextStyle::Caption),
            Text::new(parts.title()).style(TextStyle::Title),
            parts.name_field(),
            parts.password_field(),
            parts.error(),
            HStack::new(row).spacing(8.0),
        ))
        .spacing(12.0)
        .align(Align::Stretch),
    )
}

/// A secured application of its own, over a new database: nobody in it yet.
fn secured(restart: Rc<dyn Fn()>) -> AnyView {
    let mut entities = security_entities();
    entities.push(notes());
    let database = MemoryDatabase::new(entities).unwrap_or_else(|e| panic!("the page's model: {e}"));
    SecuredApp::new("Portaria", move || database.space(), |_| App::new("Portaria").entity_info(notes()))
        .language(Language::BrazilianPortuguese)
        .role(Role::new("Visitantes").policy(PermissionPolicy::ReadOnlyAll))
        .merge(RoleMerge::AnyRole)
        .temporary_access(TemporaryAccessPolicy::new("Visitantes").max_hours(8))
        .password_policy(policy)
        .lockout(3, 10)
        .idle_timeout(Duration::from_secs(IDLE_SECONDS))
        .door(door)
        .aside(
            VStack::new((
                Text::new("Regras desta portaria").style(TextStyle::Headline),
                Text::new("Senha com número e sem o nome."),
                Text::new("3 senhas erradas: 10 minutos de espera."),
                Text::new(format!("{} s sem mexer: volta para a entrada.", IDLE_SECONDS)),
            ))
            .spacing(6.0)
            .padding(16.0)
            .align(Align::Start),
        )
        .sign_in_action(
            SignInAction::new("Recomeçar a portaria", move || restart()).icon(IconName::RotateCcw),
        )
        .into_view()
}

/// The application of the page, kept from one build to the next.
struct Kept(Rc<AnyView>);

impl View for Kept {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child(&**self.0)
    }
}

/// Every public feature of SecuredApp on one page.
#[derive(Debug, Default)]
pub struct SecureDoorPage;

impl View for SecureDoorPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let round = cx.use_state(|| 0_u64);
        let asked = cx.use_state(|| Rc::new(Cell::new(false)));
        let made = cx.use_state(|| None::<(u64, Rc<AnyView>)>);
        let current = round.get();
        let view = match made.get() {
            Some((kept, view)) if kept == current => view,
            _ => {
                let (again, flag) = (round.clone(), asked.get());
                let restart: Rc<dyn Fn()> = Rc::new(move || {
                    flag.set(true);
                    again.update(|n| *n += 1);
                });
                let view = Rc::new(secured(restart));
                made.set(Some((current, view.clone())));
                view
            }
        };
        let said = if asked.get().get() {
            format!("Portaria recomeçada {current} vez(es): ninguém cadastrado.")
        } else {
            "Portaria nova: crie o administrador.".to_owned()
        };
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Uma aplicação protegida dentro da página. A política de senha vale para o primeiro administrador, para a troca da própria senha e para a senha dada pelo administrador. Três senhas erradas fazem a conta esperar, e o administrador pode desbloquear sem trocar a senha. Sem mexer por dois minutos, a sessão volta para a entrada.")
                    .foreground(Semantic::InkMuted),
                Text::new(said),
                Keyed::new(current, Kept(view)).height(560.0),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(1100.0)
            .align(Align::Stretch),
        )
    }
}
