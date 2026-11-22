//! The services of the system (#883): what `ravenrs-app-update` writes for a server that runs with
//! the machine (`ServiceSpec`) and what it asks the system's manager (`ServiceManager`), on the
//! example app of the news, in a dry run that writes only in a folder of its own and runs no command.
use std::path::PathBuf;

use ravenrs::prelude::*;
use ravenrs_app_update::{ServiceManager, ServiceSpec, ServiceSystem};

use super::novidades::APP;

/// The control registered in the center.
pub const ID: &str = "ServiceDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Serviço do sistema";

/// Where the dry run writes its files.
fn folder() -> PathBuf {
    std::env::temp_dir().join(format!("raven-feature-center-servico-{}", std::process::id()))
}

/// The service of the example app, with what the page chose.
fn spec(limit: bool, whole_system: bool, by_hand: bool) -> ServiceSpec {
    let program = PathBuf::from("/opt/raven-exemplo/raven-exemplo");
    let mut spec = if by_hand {
        ServiceSpec::new(
            "raven-exemplo-servidor",
            "dev.ravenrs.raven-exemplo.servidor",
            "Raven Exemplo (servidor)",
            program,
        )
    } else {
        ServiceSpec::for_app(&APP, program)
    };
    spec = spec
        .args(["servir", "--endereco", "0.0.0.0:8080"])
        .env("RUST_LOG", "info")
        .logs(folder().join("logs"))
        .system(whole_system);
    if limit {
        spec = spec.memory_max("1G");
    }
    spec
}

fn system_of(index: u32) -> ServiceSystem {
    match index {
        1 => ServiceSystem::Launchd,
        2 => ServiceSystem::TaskScheduler,
        _ => ServiceSystem::Systemd,
    }
}

/// Every public feature of ServiceSpec and ServiceManager on one page.
#[derive(Debug, Default)]
pub struct ServicePage;

impl View for ServicePage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let system = cx.use_state(|| 0_u32);
        let limit = cx.use_state(|| true);
        let whole = cx.use_state(|| false);
        let by_hand = cx.use_state(|| false);
        let said = cx.use_state(|| "Nada planejado".to_owned());
        let chosen = system_of(system.get());
        let made = spec(limit.get(), whole.get(), by_hand.get());
        let checked = match made.check() {
            Ok(()) => "Conferido: o serviço pode ser instalado".to_owned(),
            Err(why) => format!("Recusado: {why}"),
        };
        let file = match chosen {
            ServiceSystem::Systemd => made.systemd_unit(),
            ServiceSystem::Launchd => made.launchd_plist(),
            ServiceSystem::TaskScheduler => made.windows_task_xml(),
        };
        let manager = ServiceManager::new(chosen).dry_run(folder());
        let here =
            ServiceManager::current().map_or_else(|| "nenhum".to_owned(), |m| m.system().words().to_owned());
        let place = format!("Arquivo: {}", manager.file_of(&made).display());
        let (install, state, remove) = (manager.clone(), manager.clone(), manager);
        let (install_spec, state_spec, remove_spec) = (made.clone(), made.clone(), made);
        let system_button = |label: &'static str, index: u32| {
            let pick = system.clone();
            Button::new(label)
                .style(if system.get() == index { ButtonStyle::Primary } else { ButtonStyle::Secondary })
                .on_click(move || pick.set(index))
        };
        let flip = |state: &State<bool>| -> Box<dyn Fn()> {
            let state = state.clone();
            Box::new(move || state.update(|on| *on = !*on))
        };
        let (said_install, said_state, said_remove) = (said.clone(), said.clone(), said.clone());
        let toggle = |label: &'static str, on: bool, flip: Box<dyn Fn()>| {
            Button::new(format!("{label}: {}", if on { "sim" } else { "não" }))
                .style(ButtonStyle::Secondary)
                .on_click(move || flip())
        };
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "Um app que é servidor roda como serviço do sistema: sobe com a máquina, volta quando cai e para com um sinal. O mesmo ServiceSpec dá a unidade do systemd, o plist do launchd e a tarefa do Windows.",
                )
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
                Text::new(format!("Gerenciador deste computador: {here}")).test_id("estado-local"),
                VStack::new((
                Text::new("Sistema").style(TextStyle::Headline),
                HStack::new((
                    system_button("systemd", 0),
                    system_button("launchd", 1),
                    system_button("Windows", 2),
                ))
                .spacing(8.0),
                Text::new("O serviço").style(TextStyle::Headline),
                HStack::new((
                    toggle("Limite de 1G", limit.get(), flip(&limit)),
                    toggle("Do sistema todo", whole.get(), flip(&whole)),
                    toggle("Nome à mão", by_hand.get(), flip(&by_hand)),
                ))
                .spacing(8.0),
                Text::new(checked).test_id("conferido"),
                Text::new(place).test_id("arquivo"),
                Text::new(file).test_id("conteudo"),
                ))
                .spacing(12.0)
                .align(Align::Stretch),
                VStack::new((
                Text::new("Instalar, ver e remover (sem rodar nada)").style(TextStyle::Headline),
                HStack::new((
                    Button::new("Planejar a instalação").on_click(move || {
                        said_install.set(match install.install(&install_spec) {
                            Ok(done) => {
                                let commands: Vec<String> = done.commands.iter().map(|c| c.join(" ")).collect();
                                let mut text = format!("Comandos: {}", commands.join(" · "));
                                for note in done.notes {
                                    text.push_str(&format!(" · aviso: {note}"));
                                }
                                text
                            }
                            Err(why) => format!("Recusado: {why}"),
                        });
                    }),
                    Button::new("Ver o estado").style(ButtonStyle::Secondary).on_click(move || {
                        said_state.set(match state.status(&state_spec) {
                            Ok(status) => format!(
                                "Estado: {} · consultaria com: {}",
                                if status.installed { "instalado" } else { "não instalado" },
                                status.said
                            ),
                            Err(why) => format!("Recusado: {why}"),
                        });
                    }),
                    Button::new("Remover").style(ButtonStyle::Secondary).on_click(move || {
                        said_remove.set(match remove.uninstall(&remove_spec) {
                            Ok(commands) => format!(
                                "Removido · {}",
                                commands.iter().map(|c| c.join(" ")).collect::<Vec<_>>().join(" · ")
                            ),
                            Err(why) => format!("Recusado: {why}"),
                        });
                    }),
                ))
                .spacing(8.0),
                Text::new(said.get()).test_id("resultado"),
                ))
                .spacing(12.0)
                .align(Align::Stretch),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Stretch),
        )
    }
}
