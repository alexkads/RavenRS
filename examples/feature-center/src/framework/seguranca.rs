//! Signing in through a company's own login, every feature of the types it takes: a door the
//! application composes itself, with a view beside it and a button of its own; an outside provider
//! that only says who someone is, from issuers accepted one by one; the roles of a person that all
//! have to allow something; and the session of a person driven in code — who they are, what they
//! may do, their password, and losing everything once their outside identity is unlinked.

use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{
    Access, AuditMode, AuditSettings, AuditTrail, Claims, Credentials, DashboardInfo, EntityInfo,
    ExternalSignIn, FieldInfo, FieldKind, IdentityProvider, MemoryDatabase, ObjectSpace, Operation,
    PermissionPolicy, Permissions, Provider, Query, Role, RoleMerge, SECURITY_AUDIT_ENTRY, SECURITY_ROLE,
    SECURITY_USER, SECURITY_USER_ROLE, SecuredObjectSpace, SecurityContext, SpaceError, SpaceMaker,
    UserDirectory, Value, audit_entity, hash_password, security_entities,
};
use ravenrs_app_ui::{App, DoorParts, Language, SecuredApp, Session, SignInAction};

use super::modulos::button;
use super::screen;

/// The control's name, as the session knows it.
pub const ID: &str = "SignInDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Entrada pela empresa";

/// Who vouches for the people of the company.
pub const ISSUER: &str = "https://login.empresa.exemplo";
/// What the company's login knows Lia by.
const LIA: &str = "lia-0042";
/// What Lia types at the door of the portal, and the code the company's login takes.
pub const LIA_LOGIN: &str = "lia@empresa";
/// The code of the company's login for Lia.
pub const LIA_CODE: &str = "2468";
/// Lia's own password, kept by the application besides her company's login.
pub const LIA_PASSWORD: &str = "senha-da-lia";
const CONTRACT: &str = "Contrato";

/// The dashboards of this part.
pub fn dashboards() -> Vec<DashboardInfo> {
    vec![screen(ID, PAGE, "Segurança", "log-in")]
}

/// The controls of [`dashboards`]. The page is made once a session, so the portal inside it stays
/// signed in while the person goes elsewhere and back.
pub fn register(session: &Session) {
    let page = match Page::new() {
        Ok(page) => Rc::new(page),
        Err(why) => {
            session.register_control(ID, move || Box::new(Alert::message(why.clone())) as AnyView);
            return;
        }
    };
    session.register_control(ID, move || Box::new(Shown(page.clone())) as AnyView);
}

/// The company's login, made up: it knows Lia by a code, and Rui only at another company.
struct CompanyLogin;

impl Provider for CompanyLogin {
    fn name(&self) -> &str {
        "Login da empresa"
    }
}

impl IdentityProvider for CompanyLogin {
    fn verify(&self, credentials: &Credentials) -> Result<Claims, SpaceError> {
        match (credentials.name.as_str(), credentials.password.as_str()) {
            (LIA_LOGIN, LIA_CODE) => Ok(Claims::new(ISSUER, LIA).email("lia@empresa.exemplo")),
            ("rui@outra", _) => Ok(Claims::new("https://login.outra.exemplo", "rui")),
            _ => Err(SpaceError::Denied("código errado".into())),
        }
    }
}

fn contract() -> EntityInfo {
    EntityInfo::new(CONTRACT)
        .caption("Contratos")
        .navigation("Portal")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Contrato").default_property())
        .field(FieldInfo::new("amount", FieldKind::Money).caption("Valor"))
}

/// Managers can change the model of the portal, and never delete a contract.
fn roles() -> Vec<Role> {
    vec![
        Role::new("Gerência").policy(PermissionPolicy::AllowAll).can_edit_model().type_permission(
            CONTRACT,
            [Operation::Delete],
            Access::Deny,
        ),
        Role::new("Contratos").policy(PermissionPolicy::ReadOnlyAll),
        Role::new("Administradores").administrative(),
    ]
}

fn wrong(error: SpaceError) -> String {
    error.to_string()
}

/// Lia, a manager who also only reads contracts, and Rui, who administers, with two contracts.
fn seed(space: &mut dyn ObjectSpace) -> Result<(), String> {
    let roles = space.query(SECURITY_ROLE, &Query::all()).map_err(wrong)?;
    let role = |name: &str| {
        roles
            .iter()
            .find(|role| role.get("name") == Value::Text(name.to_owned()))
            .map(|role| role.get("id"))
            .ok_or_else(|| format!("no role {name}"))
    };
    for (name, password, of) in [
        ("lia", LIA_PASSWORD, &["Gerência", "Contratos"][..]),
        ("rui", "senha-do-rui", &["Administradores"][..]),
    ] {
        let mut user = space.new_object(SECURITY_USER).map_err(wrong)?;
        user.set("name", name);
        user.set("password_hash", hash_password(password)?);
        user.set("must_change_password", false);
        let key = user.get("id");
        space.save(user).map_err(wrong)?;
        for role_name in of {
            let mut link = space.new_object(SECURITY_USER_ROLE).map_err(wrong)?;
            link.set("user_id", key.clone());
            link.set("role_id", role(role_name)?);
            space.save(link).map_err(wrong)?;
        }
    }
    for (id, name, amount) in [(1.0, "Manutenção do prédio", 120_000.0), (2.0, "Limpeza", 48_000.0)] {
        let mut record = space.new_object(CONTRACT).map_err(wrong)?;
        record.set("id", id);
        record.set("name", name);
        record.set("amount", amount);
        space.save(record).map_err(wrong)?;
    }
    space.commit().map_err(wrong)
}

/// The portal and the session in code, over one database of their own.
struct Page {
    database: MemoryDatabase,
    directory: UserDirectory,
    portal: Rc<AnyView>,
    said: State<String>,
    session: State<Option<Rc<SecurityContext>>>,
}

/// The door of the portal, arranged by the application: its words, a chip that fills Lia in, the
/// two fields, Sign in beside the button of its own, and what went wrong.
fn door(parts: &DoorParts) -> AnyView {
    let filler = parts.clone();
    let mut row: Vec<AnyView> = vec![Box::new(parts.sign_in_button().style(ButtonStyle::Primary))];
    row.extend(parts.actions().into_iter().map(|action| Box::new(action) as AnyView));
    let mut rows: Vec<AnyView> = vec![
        Box::new(Text::new(parts.title()).style(TextStyle::Title)),
        Box::new(Text::new(format!("Use o login da empresa: {LIA_LOGIN}, código {LIA_CODE}."))),
        Box::new(
            Button::new("Preencher a Lia")
                .style(ButtonStyle::Ghost)
                .on_click(move || filler.fill(LIA_LOGIN, LIA_CODE)),
        ),
        parts.name_field(),
        parts.password_field(),
        Box::new(HStack::new(row).spacing(8.0).wrap()),
    ];
    if let Some(error) = parts.error() {
        rows.push(error);
    }
    Box::new(VStack::new(rows).spacing(12.0).align(Align::Stretch))
}

impl Page {
    fn new() -> Result<Self, String> {
        let mut entities = security_entities();
        entities.push(audit_entity());
        entities.push(contract());
        let database = MemoryDatabase::new(entities).map_err(|e| e.0)?;
        let made = database.clone();
        let maker: SpaceMaker = Rc::new(move || Box::new(made.space()) as Box<dyn ObjectSpace>);
        let directory = UserDirectory::new(maker, roles()).map_err(wrong)?;
        seed(&mut database.space())?;
        let lia = directory.identity_of("lia").map_err(wrong)?.ok_or("no Lia")?;
        directory.link_external(&lia.key, ISSUER, LIA).map_err(wrong)?;

        let said = State::new("Ninguém entrou ainda.".to_owned());
        let told = said.clone();
        let behind = database.clone();
        let mut portal = SecuredApp::new(
            "Portal da empresa",
            move || behind.space(),
            |_person| {
                App::new("Portal da empresa").language(Language::BrazilianPortuguese).entity_info(contract())
            },
        )
        .language(Language::BrazilianPortuguese)
        .authenticator(ExternalSignIn::new(directory.clone(), Rc::new(CompanyLogin)).issuer(ISSUER))
        .merge(RoleMerge::AllRoles)
        .door(door)
        .aside(Card::new(
            VStack::new((
                Text::new("Empresa Exemplo").style(TextStyle::Headline),
                Text::new("Quem entra aqui é quem a empresa diz, e pode o que o papel dele no portal deixa.")
                    .foreground(Semantic::InkMuted),
            ))
            .spacing(6.0),
        ))
        .sign_in_action(
            SignInAction::new("Voltar à escolha da empresa", move || {
                told.set("Voltou à escolha da empresa: nada do que foi digitado ficou.".to_owned());
            })
            .icon(IconName::ArrowLeft),
        );
        for role in roles() {
            portal = portal.role(role);
        }
        let portal = portal.build().map_err(|e| e.0)?;
        Ok(Self { database, directory, portal: Rc::new(portal), said, session: State::new(None) })
    }

    /// Lia, through the company's login, as the portal lets her in, with a session of her own.
    fn sign_in(&self) -> String {
        let external = ExternalSignIn::new(self.directory.clone(), Rc::new(CompanyLogin)).issuer(ISSUER);
        let foreign =
            ravenrs_app::Authenticator::authenticate(&external, &Credentials::new("rui@outra", "x"));
        let identity =
            match ravenrs_app::Authenticator::authenticate(&external, &Credentials::new(LIA_LOGIN, LIA_CODE))
            {
                Ok(identity) => identity,
                Err(why) => return format!("A empresa não deixou: {why}"),
            };
        let database = self.database.clone();
        let trail = Rc::new(AuditTrail::new(AuditSettings::new(AuditMode::Off), || "lia".to_owned()));
        trail.write_with(Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>));
        let context =
            SecurityContext::new(identity, self.directory.clone(), vec![contract()], RoleMerge::AnyRole)
                .signed_in_with(ISSUER, LIA)
                .with_audit_trail(trail)
                .with_denied_message("A Gerência não apaga contratos.");
        let who = context.identity();
        let roles: Vec<&str> = who.roles.iter().map(Role::name).collect();
        self.session.set(Some(Rc::new(context)));
        format!(
            "Entrou {} pela empresa, com {}. Rui, de outra empresa, ficou de fora: {}",
            who.name,
            roles.join(" e "),
            foreign.map_or_else(|why| why.to_string(), |_| "entrou".to_owned())
        )
    }

    fn with_session(&self, then: impl FnOnce(&SecurityContext) -> String) -> String {
        match self.session.get() {
            Some(context) => then(&context),
            None => "Entre primeiro pela empresa.".to_owned(),
        }
    }

    /// What Lia's session allows, as the security system answers it.
    fn allowed(context: &SecurityContext) -> String {
        context.with(|permissions: &Permissions| {
            let roles: Vec<String> = permissions
                .identity()
                .roles
                .iter()
                .map(|role| {
                    format!(
                        "{} ({})",
                        role.name(),
                        if role.is_administrative() { "administra" } else { "não administra" }
                    )
                })
                .collect();
            format!(
                "Papéis: {}. Muda o modelo: {}. Lê contratos: {}. Apaga contratos: {}.",
                if roles.is_empty() { "nenhum".to_owned() } else { roles.join(", ") },
                yes(permissions.can_edit_model()),
                yes(permissions.can(Operation::Read, CONTRACT, None)),
                yes(permissions.can(Operation::Delete, CONTRACT, None)),
            )
        })
    }

    fn delete(&self, context: Rc<SecurityContext>) -> String {
        let mut space = SecuredObjectSpace::new(Box::new(self.database.space()), context);
        match space.delete(CONTRACT, &Value::Number(2.0)).and_then(|()| space.commit()) {
            Ok(()) => "Apagou o contrato.".to_owned(),
            Err(why) => format!("Recusado: {why}"),
        }
    }

    fn trail(&self) -> usize {
        self.database.space().query(SECURITY_AUDIT_ENTRY, &Query::all()).map_or(0, |lines| lines.len())
    }
}

fn yes(on: bool) -> &'static str {
    if on { "sim" } else { "não" }
}

/// The page, over the one made for the session.
struct Shown(Rc<Page>);

impl View for Shown {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let page = &self.0;
        let act = |caption: &str, run: fn(&Page) -> String| {
            let page = page.clone();
            button(caption, move || {
                let told = run(&page);
                page.said.set(told);
            })
        };
        let buttons: Vec<AnyView> = vec![
            act("Entrar pela empresa", Page::sign_in),
            act("O que ela pode", |page| page.with_session(Page::allowed)),
            act("Apagar um contrato", |page| match page.session.get() {
                Some(context) => page.delete(context),
                None => "Entre primeiro pela empresa.".to_owned(),
            }),
            act("Trocar a própria senha", |page| {
                page.with_session(|context| {
                    match context.change_own_password(LIA_PASSWORD, "outra-senha-da-lia") {
                        Ok(()) => format!("Senha trocada; a trilha tem {} anotações.", page.trail()),
                        Err(why) => format!("Não trocou: {why}"),
                    }
                })
            }),
            act("Dar senha provisória ao Rui", |page| {
                page.with_session(|context| {
                    let rui =
                        page.directory.identity_of("rui").ok().flatten().map_or(Value::Null, |rui| rui.key);
                    match context.set_password_of(&rui, "provisoria-123") {
                        Ok(()) => "O Rui vai trocar a senha ao entrar.".to_owned(),
                        Err(why) => format!("Só quem administra dá senha: {why}"),
                    }
                })
            }),
            act("Desligar o login da empresa", |page| match page.directory.unlink_external(ISSUER, LIA) {
                Ok(()) => "O login da empresa não leva mais à Lia; a sessão aberta perde tudo ao recarregar."
                    .to_owned(),
                Err(why) => why.to_string(),
            }),
            act("Recarregar a sessão", |page| {
                page.with_session(|context| match context.reload() {
                    Ok(()) => format!("Recarregada. {}", Page::allowed(context)),
                    Err(why) => why.to_string(),
                })
            }),
        ];
        let portal = Card::new(Kept(page.portal.clone()).height(if narrow { 520.0 } else { 460.0 }));
        let code = VStack::new((
            Text::new("A sessão em código").style(TextStyle::Headline),
            Text::new(
                "A mesma Lia, agora sem tela: quem ela é, o que pode, a senha e o que acontece quando a empresa a desliga.",
            )
            .foreground(Semantic::InkMuted),
            HStack::new(buttons).spacing(8.0).wrap(),
            Text::bound(&page.said, Clone::clone),
        ))
        .spacing(12.0)
        .align(Align::Stretch);
        let view = VStack::new((
            Text::new(
                "O portal abaixo é outra aplicação, com uma porta arrumada por ela: quem entra é quem o login da empresa diz, \
                 e todos os papéis da pessoa precisam deixar.",
            )
            .foreground(Semantic::InkMuted),
            portal,
            code,
        ))
        .spacing(16.0)
        .padding(if narrow { 0.0 } else { 16.0 })
        .align(Align::Stretch);
        cx.child(&view)
    }
}

/// One view built once and shown again on each rebuild, as the portal holds who signed in.
#[derive(Clone)]
struct Kept(Rc<AnyView>);

impl View for Kept {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child(self.0.as_ref().as_ref())
    }
}
