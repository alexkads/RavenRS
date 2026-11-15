//! The screens of an application put together in code, every feature of the types that make them:
//! an application with its own brand, themes, counts, controls, list editors and records no storage
//! keeps (`App`); a window put together by hand over an application, with a template of its own
//! (`AppShell`, `ActionContainer`, `ActionButton`); and the pieces of a screen an application made
//! itself (`ListView`, `EntityPane`, `PaneWords`, `NavigationCount`).

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{
    Action, ActionCategory, Application, DashboardInfo, EntityInfo, EnumOption, FieldInfo, FieldKind,
    MemoryAttachmentStore, MemoryDatabase, MemoryModelStore, MemorySettings, MemoryStorage, Object,
    ObjectSpace, Selection, Value, ViewController, ViewInfo, ViewVariant,
};
use ravenrs_app_ui::{
    ActionButton, ActionContainer, App, AppShell, EditorRegistry, EntityPane, FrameTemplate, Language,
    ListView, NavigationCount, PaneWords, Session, SystemModule, TemplateParts, Texts, Words,
};
use ravenrs_widgets::{IconTile, SidebarState};

use super::modulos::button;
use super::screen;

/// The group of the navigation of the screens put together in code.
pub const GROUP: &str = "Telas em código";

/// The page of an application written in code.
pub const APP_PAGE: &str = "Aplicação em código";
const APP_ID: &str = "AppInCodeDemo";
/// The page of a window put together by hand.
pub const SHELL_PAGE: &str = "Janela montada à mão";
const SHELL_ID: &str = "AppShellDemo";
/// The page of the pieces of a screen.
pub const PARTS_PAGE: &str = "Peças de uma tela";
const PARTS_ID: &str = "ScreenPartsDemo";

/// The dashboards of this part.
pub fn dashboards() -> Vec<DashboardInfo> {
    vec![
        screen(APP_ID, APP_PAGE, GROUP, "layout-dashboard"),
        screen(SHELL_ID, SHELL_PAGE, GROUP, "monitor"),
        screen(PARTS_ID, PARTS_PAGE, GROUP, "shapes"),
    ]
}

/// The controls of [`dashboards`], each made once a session, so what is inside stays as it was
/// left while the person goes elsewhere and back.
pub fn register(session: &Session) {
    let kept = |page: Result<AnyView, String>| -> Rc<AnyView> {
        Rc::new(page.unwrap_or_else(|why| Box::new(Alert::message(why)) as AnyView))
    };
    for (id, page) in
        [(APP_ID, kept(AppInCode::page())), (SHELL_ID, kept(ByHand::page())), (PARTS_ID, kept(Parts::page()))]
    {
        session.register_control(id, move || Box::new(Kept(page.clone())) as AnyView);
    }
}

/// What this part adds to the application: nothing, its screens bring their own.
pub fn extend(app: App) -> App {
    app
}

/// One view built once and shown again on each rebuild.
#[derive(Clone)]
struct Kept(Rc<AnyView>);

impl View for Kept {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child(self.0.as_ref().as_ref())
    }
}

/// What every page here has: its words, a line of what happened, its buttons and what it shows.
fn page_of(intro: &str, said: &State<String>, buttons: Vec<AnyView>, shown: AnyView, tall: f32) -> AnyView {
    Box::new(
        VStack::new((
            Text::new(intro.to_owned()).foreground(Semantic::InkMuted),
            HStack::new(buttons).spacing(8.0).wrap(),
            Text::bound(said, Clone::clone),
            Card::new(Kept(Rc::new(shown)).height(tall)),
        ))
        .spacing(12.0)
        .align(Align::Stretch),
    )
}

fn key() -> FieldInfo {
    FieldInfo::new("id", FieldKind::Integer).key()
}

fn record(entity: &str, values: &[(&str, Value)]) -> Object {
    Object::from_values(
        entity,
        values.iter().map(|(name, value)| ((*name).to_owned(), value.clone())).collect::<BTreeMap<_, _>>(),
    )
}

// ---------------------------------------------------------------------------------------------
// An application written in code.

/// A clinic's patients, kept; today's appointments and each patient's visits, which come from
/// elsewhere and no storage keeps.
fn patient() -> EntityInfo {
    EntityInfo::new("Patient")
        .caption("Pacientes")
        .navigation("Clínica")
        .icon("users")
        .field(key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
        .field(
            FieldInfo::new(
                "plan",
                FieldKind::Enum(vec![
                    EnumOption::new("basic", "Básico"),
                    EnumOption::new("full", "Completo"),
                ]),
            )
            .caption("Plano"),
        )
        .field(FieldInfo::new("rating", FieldKind::Integer).caption("Satisfação").editor(STARS))
        .field(
            FieldInfo::new("visits", FieldKind::Collection("Visit".into()))
                .caption("Visitas")
                .back_reference("patient_id"),
        )
        .variant(ViewVariant::new("grid", "Lista"))
        .variant(ViewVariant::new("cards", "Fichas").editor(CARDS))
}

fn appointment() -> EntityInfo {
    EntityInfo::new("Appointment")
        .caption("Consultas de hoje")
        .navigation("Clínica")
        .icon("calendar")
        .field(key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Paciente").default_property())
        .field(FieldInfo::new("at", FieldKind::Text).caption("Hora"))
}

fn visit() -> EntityInfo {
    EntityInfo::new("Visit")
        .caption("Visitas")
        .field(key())
        .field(FieldInfo::new("patient_id", FieldKind::Reference("Patient".into())).caption("Paciente"))
        .field(FieldInfo::new("name", FieldKind::Text).caption("Motivo").default_property())
}

/// The control of the field Satisfação, by the name the model gives it.
const STARS: &str = "Stars";
/// The list editor of the variant Fichas.
const CARDS: &str = "PatientCards";

/// A row of five buttons that set the value, the control of [`STARS`].
fn stars() -> EditorRegistry {
    let mut editors = EditorRegistry::default();
    editors.register(STARS, |request| {
        let value = request.value.clone();
        let now = value.get().as_number().unwrap_or(0.0) as usize;
        let row: Vec<AnyView> = (1..=5)
            .map(|at| {
                let value = value.clone();
                Box::new(
                    Button::new(if at <= now { "\u{2605}" } else { "\u{2606}" })
                        .style(ButtonStyle::Ghost)
                        .accessible_name(format!("{at} de 5"))
                        .on_click(move || value.set(Value::Number(at as f64))),
                ) as AnyView
            })
            .collect();
        Box::new(HStack::new(row).spacing(2.0)) as AnyView
    });
    editors
}

/// The patients as cards, one line each, the editor of the variant Fichas.
fn cards(request: &ravenrs_app_ui::ListEditorRequest) -> AnyView {
    let lines: Vec<AnyView> = request
        .objects
        .iter()
        .map(|patient| {
            let (open, key) = (request.on_open.clone(), patient.get("id"));
            Box::new(Card::new(
                HStack::new((
                    Text::new(format!("Ficha de {}", patient.get("name").to_text())),
                    Button::new("Abrir").style(ButtonStyle::Ghost).on_click(move || open(&key)),
                ))
                .spacing(12.0),
            )) as AnyView
        })
        .collect();
    Box::new(VStack::new(lines).spacing(8.0).align(Align::Stretch))
}

fn seed_patients(space: &mut dyn ObjectSpace) -> Result<(), String> {
    for (id, name, plan, rating) in [(1.0, "Marta Reis", "full", 4.0), (2.0, "Otávio Lima", "basic", 2.0)] {
        let mut patient = space.new_object("Patient").map_err(|e| e.to_string())?;
        patient.set("id", id);
        patient.set("name", name);
        patient.set("plan", plan);
        patient.set("rating", rating);
        space.save(patient).map_err(|e| e.to_string())?;
    }
    space.commit().map_err(|e| e.to_string())
}

fn appointments() -> Vec<Object> {
    [(1.0, "Marta Reis", "09:00"), (2.0, "Otávio Lima", "10:30"), (3.0, "Paula Nunes", "14:00")]
        .into_iter()
        .map(|(id, name, at)| {
            record("Appointment", &[("id", Value::Number(id)), ("name", name.into()), ("at", at.into())])
        })
        .collect()
}

/// The visits of a patient, from the clinic's old system.
fn visits_of(patient: &Object) -> Vec<Object> {
    let reasons: &[&str] = if patient.get("id") == Value::Number(1.0) {
        &["Retorno", "Exame de rotina"]
    } else {
        &["Primeira consulta"]
    };
    reasons
        .iter()
        .enumerate()
        .map(|(at, reason)| {
            record(
                "Visit",
                &[
                    ("id", Value::Number(at as f64 + 1.0)),
                    ("patient_id", patient.get("id")),
                    ("name", (*reason).into()),
                ],
            )
        })
        .collect()
}

/// The clinic, an application of its own inside the center.
struct AppInCode;

impl AppInCode {
    /// The clinic as declared, kept one of two ways: in a storage of the browser's kind, or in a
    /// database made from the model as it ends up.
    fn clinic(said: &State<String>, made_from_model: bool) -> App {
        let told = said.clone();
        let app = App::new("Clínica Vida")
            .language(Language::BrazilianPortuguese)
            .entity_info(patient())
            .entity_info(appointment())
            .entity_info(visit())
            .brand(IconTile::new(IconName::Heart))
            .brand_subtitle("Unidade Centro")
            .themes([Theme::raven_light(), Theme::raven_glass()])
            .navigation_count("Appointment", |_| Some(NavigationCount::new("3").spoken("3 consultas hoje")))
            .records_of("Appointment", appointments)
            .collection_of("Patient", "visits", visits_of)
            .editors(stars())
            .list_editor(CARDS, cards)
            .attachment_store(MemoryAttachmentStore::default())
            .signed_in_remotely("Dra. Lia".to_owned(), move || {
                told.set("A Dra. Lia saiu da clínica.".to_owned())
            });
        if made_from_model {
            app.storage_for(|entities| {
                let database = MemoryDatabase::new(entities.to_vec()).ok();
                let seeded = Cell::new(false);
                move || {
                    let mut space = database
                        .as_ref()
                        .map(MemoryDatabase::space)
                        .expect("the model has a key for each entity");
                    if !seeded.replace(true) {
                        let _ = seed_patients(&mut space);
                    }
                    space
                }
            })
            .model_store(MemoryModelStore::default())
            .settings(MemorySettings::default())
        } else {
            app.keep_in(MemoryStorage::default()).seed(seed_patients)
        }
    }

    fn page() -> Result<AnyView, String> {
        let said =
            State::new("A clínica abriu guardando tudo num armazenamento, como o do navegador.".to_owned());
        let kept = Self::clinic(&said, false).build().map_err(|e| e.0)?;
        let made = Self::clinic(&said, true).build().map_err(|e| e.0)?;
        Ok(Box::new(AppPage { said, kept: Rc::new(kept), made: Rc::new(made) }))
    }
}

struct AppPage {
    said: State<String>,
    kept: Rc<AnyView>,
    made: Rc<AnyView>,
}

impl View for AppPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let from_model = cx.use_state(|| false);
        let choose = |caption: &str, to: bool, words: &'static str| {
            let (from_model, said) = (from_model.clone(), self.said.clone());
            button(caption, move || {
                from_model.set(to);
                said.set(words.to_owned());
            })
        };
        let buttons = vec![
            choose(
                "Guardar num armazenamento",
                false,
                "A clínica guarda registros e preferências num armazenamento.",
            ),
            choose(
                "Banco feito do modelo",
                true,
                "A clínica guarda num banco feito das entidades do modelo, com preferências à parte.",
            ),
        ];
        let shown = if from_model.get() { self.made.clone() } else { self.kept.clone() };
        let intro = "Outra aplicação, toda declarada em código: marca, temas próprios, uma contagem ao lado de Consultas, \
                     consultas e visitas que vêm de fora, um controle de estrelas e as fichas como outra visão da lista.";
        let view = page_of(intro, &self.said, buttons, Box::new(Kept(shown)), 520.0);
        cx.child(view.as_ref())
    }
}

// ---------------------------------------------------------------------------------------------
// A window put together by hand.

fn stock_item() -> EntityInfo {
    EntityInfo::new("StockItem")
        .caption("Itens")
        .navigation("Almoxarifado")
        .icon("package")
        .field(key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Item").default_property())
        .field(FieldInfo::new("quantity", FieldKind::Integer).caption("Quantidade"))
        .field(FieldInfo::new("unit", FieldKind::Text).caption("Unidade"))
        .field(FieldInfo::new("active", FieldKind::Boolean).caption("Ativo"))
}

/// The actions of the stock: one that opens the supplier's page, and one only for what is active.
struct StockActions;

impl ViewController for StockActions {
    fn id(&self) -> &str {
        "StockActions"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::url("StockSupplier", "Abrir no fornecedor", "https://fornecedor.exemplo/?item={name}")
                .target_entity("StockItem")
                .selection(Selection::RequireSingle),
            Action::simple("StockRetire", "Retirar do estoque")
                .target_entity("StockItem")
                .category(ActionCategory::Edit)
                .selection(Selection::RequireSingle)
                .target_criteria("[active] = True", ravenrs_app::CriteriaMode::TrueForAll)
                .on_execute(|cx| {
                    cx.message(ravenrs_app::Message::new("Retirado.", ravenrs_app::MessageKind::Success));
                    Ok(())
                }),
        ]
    }
}

#[derive(Debug)]
struct StockModule;

impl ravenrs_app::Module for StockModule {
    fn name(&self) -> &str {
        "Stock"
    }

    fn entities(&self) -> Vec<EntityInfo> {
        vec![stock_item()]
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(StockActions)]
    }
}

/// The stock as an application, with what every one needs, over a database with three items.
fn stock() -> Result<(Application, MemoryDatabase), String> {
    let application = Application::builder("Almoxarifado")
        .module(SystemModule::with_words(Words::pt_br()))
        .module(StockModule)
        .build()
        .map_err(|e| e.0)?;
    let database = MemoryDatabase::new(application.entities().to_vec()).map_err(|e| e.0)?;
    let mut space = database.space();
    for (id, name, quantity, active) in
        [(1.0, "Luvas", 120.0, true), (2.0, "Máscaras", 40.0, true), (3.0, "Seringas antigas", 0.0, false)]
    {
        let mut item = space.new_object("StockItem").map_err(|e| e.to_string())?;
        for (field, value) in [
            ("id", Value::Number(id)),
            ("name", name.into()),
            ("quantity", Value::Number(quantity)),
            ("unit", "Centro".into()),
            ("active", Value::Bool(active)),
        ] {
            item.set(field, value);
        }
        space.save(item).map_err(|e| e.to_string())?;
    }
    space.commit().map_err(|e| e.to_string())?;
    Ok((application, database))
}

/// The commands of the window in a bar of their own, on top, with the button of the main one at
/// its end; the navigation on the side, and the view under it all.
struct Workbench(Session);

impl FrameTemplate for Workbench {
    fn arrange(&self, parts: TemplateParts) -> AnyView {
        let commands = HStack::new((
            ActionContainer::new(self.0.clone(), [ActionCategory::Edit]).keys_elsewhere(false),
            ActionButton::new(self.0.clone(), "StockRetire")
                .style(ButtonStyle::Primary)
                .size(ButtonSize::Small)
                .hide_when_off(),
        ))
        .spacing(8.0);
        Box::new(
            HStack::new((
                parts.navigation,
                VStack::new((commands, parts.content.grow(1.0))).spacing(8.0).align(Align::Stretch).grow(1.0),
            ))
            .spacing(0.0)
            .align(Align::Stretch),
        )
    }
}

struct ByHand;

impl ByHand {
    fn page() -> Result<AnyView, String> {
        let said = State::new("Nada exportado nem aberto ainda.".to_owned());
        let (application, database) = stock()?;
        let (file_said, url_said) = (said.clone(), said.clone());
        let shell = AppShell::with_options(
            application,
            move || database.space(),
            Words::pt_br(),
            EditorRegistry::default(),
        )
        .brand(IconTile::new(IconName::Package))
        .brand_subtitle("Unidade Centro")
        .navigation_count("StockItem", |session| {
            let view = ViewInfo::from_model(&session.model(), "StockItem_ListView").ok()?;
            let low = session
                .list_records(&view)
                .iter()
                .filter(|item| item.get("quantity").as_number() == Some(0.0))
                .count();
            (low > 0).then(|| NavigationCount::new(low.to_string()).spoken(format!("{low} item sem estoque")))
        })
        .themes([Theme::raven_dark(), Theme::raven_light()])
        .theme(Theme::raven_light())
        .without_theme_menu()
        .sidebar(SidebarState::new(true))
        .on_file(move |name, bytes| {
            file_said.set(format!(
                "A janela entregou {name}, com {} bytes, sem perguntar onde salvar.",
                bytes.len()
            ))
        })
        .on_url(move |address| {
            url_said.set(format!("A janela entregou o endereço {address}, sem abrir o navegador."))
        });
        let session = shell.session();
        let sidebar = shell.sidebar_state();
        let shell = shell.template(Workbench(session.clone()));
        Ok(Box::new(ShellPage { said, session, sidebar, shell: Rc::new(Box::new(shell)) }))
    }
}

struct ShellPage {
    said: State<String>,
    session: Session,
    sidebar: SidebarState,
    shell: Rc<AnyView>,
}

impl View for ShellPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let picker = cx.file_picker();
        let (session, sidebar, said) = (self.session.clone(), self.sidebar.clone(), self.said.clone());
        let buttons = vec![
            button("Abrir ou fechar a navegação", move || {
                sidebar.toggle();
                said.set(
                    if sidebar.is_open() { "Navegação aberta." } else { "Navegação fechada." }.to_owned(),
                );
            }),
            button("Exportar os itens", move || {
                let Ok(view) = ViewInfo::from_model(&session.model(), "StockItem_ListView") else { return };
                let lines: Vec<String> =
                    session.list_records(&view).iter().map(|item| item.get("name").to_text()).collect();
                session.hand_file("itens.csv".to_owned(), lines.join("\n").into_bytes(), &picker);
            }),
        ];
        let intro = "Uma janela montada peça por peça sobre uma aplicação: marca, contagem do que está sem estoque, \
                     um tema só, sem menu de temas, os comandos numa barra própria, e arquivos e endereços entregues à aplicação.";
        let view = page_of(intro, &self.said, buttons, Box::new(Kept(self.shell.clone())), 460.0);
        cx.child(view.as_ref())
    }
}

// ---------------------------------------------------------------------------------------------
// The pieces of a screen an application made itself.

fn contact() -> EntityInfo {
    EntityInfo::new("PaneContact")
        .caption("Contatos")
        .field(key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
        .field(FieldInfo::new("phone", FieldKind::Text).caption("Telefone"))
        .rule(ravenrs_app::Rule::required("name").message("Dê um nome ao contato"))
}

struct Parts;

impl Parts {
    fn page() -> Result<AnyView, String> {
        let said = State::new("Abra uma linha ou cadastre um contato.".to_owned());
        let (application, database) = stock()?;
        let stock_space = database.clone();
        let session = Session::embedded(application, move || stock_space.space(), Words::pt_br());
        session.show_list("StockItem_ListView");

        let contacts = MemoryDatabase::new(vec![contact()]).map_err(|e| e.0)?;
        let changes = Rc::new(Cell::new(0_u64));
        let (done, failed, counted) = (said.clone(), said.clone(), changes.clone());
        let texts = Texts::new(Language::BrazilianPortuguese);
        let pane = EntityPane::new(
            contact(),
            move || contacts.space(),
            Words::pt_br(),
            PaneWords::new(
                "Novo contato",
                "Contato",
                "Cadastrar",
                "Apagar o contato?",
                "Ele sai da lista.",
                "Apagar",
            )
            .with_texts(&texts),
        )
        .map_err(|e| e.0)?
        .on_done(move |what| {
            counted.set(counted.get() + 1);
            done.set(format!("O painel avisou: {what}."));
        })
        .on_failed(move |why| failed.set(format!("O painel recusou: {why}")))
        .cell_text(|field, contact| {
            (field == "phone").then(|| format!("\u{260e} {}", contact.get("phone").to_text()))
        })
        .beside_from(900.0)
        .version(move || changes.get());
        Ok(Box::new(PartsPage { said, session, pane: Rc::new(Box::new(pane)) }))
    }
}

struct PartsPage {
    said: State<String>,
    session: Session,
    pane: Rc<AnyView>,
}

impl PartsPage {
    /// The items, as this screen wants them: its own words in the cells, a wider name, striped,
    /// one row at a time, sorted by quantity, with no unit column and no filter builder.
    fn items(&self) -> AnyView {
        let Ok(view) = ViewInfo::from_model(&self.session.model(), "StockItem_ListView") else {
            return Box::new(Text::new("Sem a lista de itens."));
        };
        let said = self.said.clone();
        Box::new(
            ListView::new(self.session.clone(), self.session.frame(), view)
                .cell_text(Rc::new(|field, item| {
                    (field == "quantity").then(|| format!("{} un.", item.get("quantity").to_text()))
                }))
                .column_shape(|field, column| if field.name == "name" { column.width(220.0) } else { column })
                .grid_shape(|grid| grid.striped(true))
                .implied("unit")
                .plain_toolbar()
                .single_selection()
                .sort("quantity", false)
                .on_open(Rc::new(move |key: &Value| said.set(format!("Abriu o item {}.", key.to_text()))))
                .height(260.0),
        )
    }

    /// Today's deliveries, which no storage keeps, only as tall as they need.
    fn deliveries(&self) -> AnyView {
        let Ok(view) = ViewInfo::from_model(&self.session.model(), "StockItem_ListView") else {
            return Box::new(Text::new("Sem a lista de itens."));
        };
        let arriving = || {
            vec![record(
                "StockItem",
                &[
                    ("id", Value::Number(9.0)),
                    ("name", "Aventais, chegando hoje".into()),
                    ("quantity", Value::Number(30.0)),
                ],
            )]
        };
        Box::new(
            ListView::new(self.session.clone(), self.session.frame(), view)
                .source(Rc::new(arriving))
                .compact(),
        )
    }
}

impl View for PartsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let view = VStack::new((
            Text::new(
                "Uma tela feita pela aplicação, com as peças do framework: os comandos da lista, o botão de uma Action, \
                 a lista do jeito que a tela quer, outra que não vem do armazenamento e um painel de lista com formulário.",
            )
            .foreground(Semantic::InkMuted),
            Text::bound(&self.said, Clone::clone),
            HStack::new((
                ActionContainer::new(self.session.clone(), [ActionCategory::Edit]).keys_elsewhere(false),
                ActionButton::new(self.session.clone(), "StockRetire").hide_when_off(),
            ))
            .spacing(8.0),
            Kept(Rc::new(self.items())),
            Text::new("Entregas de hoje").style(TextStyle::Headline),
            Kept(Rc::new(self.deliveries())),
            Text::new("Contatos").style(TextStyle::Headline),
            Card::new(Kept(self.pane.clone()).height(if narrow { 560.0 } else { 360.0 })),
        ))
        .spacing(12.0)
        .padding(if narrow { 0.0 } else { 16.0 })
        .align(Align::Stretch);
        cx.child(&view)
    }
}
