//! The command line: the manager of plugins (`raven-ci plugins …`) and the commands plugins bring.

use std::io::Write;
use std::path::PathBuf;

use raven_ci_core::{Ci, Memory, Origin, PLUGINS_DIR, PluginState, PluginView};
use raven_ci_plugin_api::{CiError, Permission};

use crate::versao;

/// What `raven-ci` says when it is asked wrong.
pub const USAGE: &str = "uso: raven-ci [--dados PASTA] [--plugins PASTA]… [--format json] <comando>
  plugins listar          todos os plugins, com o estado de cada um
  plugins info NOME       versão, dependências, permissões, configuração e memória
  plugins ligar NOME      liga o plugin
  plugins desligar NOME   desliga o plugin e os que precisam dele
  plugins compilar PASTA  compila um plugin em Rust (traits padrão) e o instala
                          [--debug] no perfil dev, mais rápido de compilar
  servir [--endereco HOST:PORTA] [--executores N]
                          o servidor: HTTP com login, executores e gatilhos
  servir ... --painel PASTA   serve o painel do navegador em /painel/
  usuario criar NOME [--papel Executores|Leitores|Administradores]
                          cria quem pode entrar; a senha vem na entrada
  credencial criar NOME --tipo token|user-password|ssh-key|arquivo [--descricao TEXTO]
                          guarda no cofre (ou gira); os campos vêm em JSON na entrada
  credencial listar       as credenciais, sem os valores, e quem as usa
  credencial apagar NOME  revoga e apaga
                          (os três vão ao servidor com --servidor URL ou RAVEN_CI_SERVIDOR)
  rodar JOB [--param NOME=VALOR]... [--prioridade N] [--esperar]
                          pede um build ao servidor; --esperar segue o log até o fim
  status JOB [NUMERO]     o build (o último, sem número): estado, etapas e anexos
  log JOB NUMERO [--seguir]
                          o log do build; --seguir até ele acabar
  resumo                  cada job com o último build, e a fila
  evidencia JOB NUMERO PASTA
                          baixa os artefatos e o log do build para PASTA
                          (estes cinco falam com --servidor URL ou RAVEN_CI_SERVIDOR,
                          com a chave em RAVEN_CI_TOKEN)
  aplicar ARQUIVO [--remover] [--servidor URL]
                          a configuração como código: jobs, pastas e plugins do arquivo
  servir --config ARQUIVO aplica o arquivo ao subir
  servico instalar|remover|estado [--memoria 1G] [--endereco HOST:PORTA] [--executores N] [--sistema]
                          o servidor como serviço do sistema (systemd, launchd ou Windows)
  agentes listar|token|remover|modelos|modelo
                          os agentes que rodam os builds fora do servidor
  <comando> ...           um comando que um plugin ligado traz
  --self-test             confere o núcleo com um plugin falso de cada ponto
  --versao                a versão";

/// What was asked on the command line.
#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    data: PathBuf,
    plugin_dirs: Vec<PathBuf>,
    json: bool,
    words: Vec<String>,
}

impl Invocation {
    /// Reads `args`, the words after the program's name.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut data = std::env::var_os("RAVEN_CI_HOME").map(PathBuf::from);
        let mut plugin_dirs = Vec::new();
        let mut json = false;
        let mut words = Vec::new();
        let mut rest = args.iter();
        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "--dados" => data = Some(rest.next().ok_or("--dados precisa de uma pasta")?.into()),
                "--plugins" => plugin_dirs.push(rest.next().ok_or("--plugins precisa de uma pasta")?.into()),
                "--format" => match rest.next().map(String::as_str) {
                    Some("json") => json = true,
                    Some("text") => json = false,
                    other => {
                        return Err(format!("--format aceita json ou text, não {}", other.unwrap_or("nada")));
                    }
                },
                // What comes after a command belongs to it, flags included.
                _ if !words.is_empty() && words[0] != "plugins" => words.push(arg.clone()),
                // The one option of a word of plugins: plugins compilar PASTA --debug.
                "--debug" if words.first().is_some_and(|w| w == "plugins") => words.push(arg.clone()),
                flag if flag.starts_with("--") => return Err(format!("opção desconhecida: {flag}")),
                _ => words.push(arg.clone()),
            }
        }
        if words.is_empty() {
            return Err("falta o comando".to_owned());
        }
        Ok(Self { data: data.unwrap_or_else(|| versao::APP.home()), plugin_dirs, json, words })
    }

    /// Runs it, writing to `out`; the exit code.
    pub fn run(&self, out: &mut dyn Write) -> Result<i32, CiError> {
        // The commands that speak to a server open no core on this disk.
        if let Some(said) = crate::remote::run(&self.words, self.json, out) {
            return said;
        }
        if let [first, rest @ ..] = self.words.as_slice()
            && first == "servico"
        {
            let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
            return crate::service::run(&self.data, &rest, self.json, out);
        }
        let ci = Ci::open(&self.data)?;
        for plugin in crate::plugins::builtin() {
            ci.add_plugin(plugin);
        }
        ci.discover_plugins(&self.data.join(PLUGINS_DIR));
        for dir in &self.plugin_dirs {
            ci.discover_plugins(dir);
        }
        let problems = ci.registry().problems();
        ci.apply_plugins()?;
        let write = |out: &mut dyn Write, text: &str| {
            out.write_all(text.as_bytes()).map_err(|e| CiError::failed(e.to_string()))
        };
        let words: Vec<&str> = self.words.iter().map(String::as_str).collect();
        match words.as_slice() {
            ["plugins", "listar"] => {
                let views = ci.registry().list()?;
                if self.json {
                    write(out, &json(&views)?)?;
                } else {
                    write(out, &list(&views, &problems))?;
                }
                Ok(0)
            }
            ["plugins", "info", name] => match ci.registry().info(name) {
                Ok(view) => {
                    let audit = ci.store().audit_of(name)?;
                    if self.json {
                        let mut value =
                            serde_json::to_value(&view).map_err(|e| CiError::failed(e.to_string()))?;
                        value["audit"] =
                            serde_json::to_value(&audit).map_err(|e| CiError::failed(e.to_string()))?;
                        write(out, &format!("{value:#}\n"))?;
                    } else {
                        write(out, &info(&view, audit.len()))?;
                    }
                    Ok(0)
                }
                Err(why) => {
                    write(out, &format!("{why}\n"))?;
                    Ok(2)
                }
            },
            ["plugins", verb @ ("ligar" | "desligar"), name] => {
                let changes = if *verb == "ligar" { ci.enable_plugin(name) } else { ci.disable_plugin(name) };
                match changes {
                    Ok(changes) => {
                        let state = ci.registry().info(name)?.state;
                        let mut text = format!("{name}: {}\n", state.words());
                        for change in changes.iter().filter(|c| !c.starts_with(&format!("{name} "))) {
                            text.push_str(&format!("  também: {change}\n"));
                        }
                        write(out, &text)?;
                        Ok(0)
                    }
                    Err(why) => {
                        write(out, &format!("{why}\n"))?;
                        Ok(2)
                    }
                }
            }
            ["plugins", "compilar", folder] => {
                crate::compilar::run(std::path::Path::new(folder), &self.data, false, out)
            }
            ["plugins", "compilar", "--debug", folder] | ["plugins", "compilar", folder, "--debug"] => {
                crate::compilar::run(std::path::Path::new(folder), &self.data, true, out)
            }
            ["plugins", ..] => {
                write(out, &format!("{USAGE}\n"))?;
                Ok(2)
            }
            ["servir", args @ ..] => crate::server::serve(ci, &self.data, args, out),
            ["aplicar", args @ ..] => crate::apply::local(&ci, args, self.json, out),
            ["usuario", "criar", name, args @ ..] => {
                let stdin = std::io::stdin();
                crate::server::create_user(&self.data, name, args, &mut stdin.lock(), out)
            }
            ["credencial", "criar", name, args @ ..] => {
                let flag = |name: &str| args.iter().position(|a| *a == name).and_then(|at| args.get(at + 1));
                let Some(kind) = flag("--tipo") else {
                    write(out, "diga o tipo: --tipo token, user-password, ssh-key ou arquivo\n")?;
                    return Ok(2);
                };
                let description = flag("--descricao").copied();
                let mut text = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                    .map_err(|e| CiError::failed(e.to_string()))?;
                let Ok(fields) = serde_json::from_str::<serde_json::Value>(&text) else {
                    write(
                        out,
                        "os campos vêm em JSON na entrada: echo '{\"token\": \"...\"}' | raven-ci credencial criar NOME --tipo token\n",
                    )?;
                    return Ok(2);
                };
                match ci.save_credential(name, kind, &fields, description, std::env::var("USER").ok().as_deref()) {
                    Ok(()) => {
                        write(out, &format!("{name}: guardada no cofre, do tipo {kind}\n"))?;
                        Ok(0)
                    }
                    Err(why) => {
                        write(out, &format!("{why}\n"))?;
                        Ok(2)
                    }
                }
            }
            ["credencial", "listar"] => {
                let list = ci.credentials()?;
                if self.json {
                    write(out, &json(&list)?)?;
                } else if list.is_empty() {
                    write(out, "nenhuma credencial\n")?;
                } else {
                    for credential in &list {
                        let used = if credential.used_by.is_empty() {
                            String::new()
                        } else {
                            format!("  usada por: {}", credential.used_by.join(", "))
                        };
                        write(
                            out,
                            &format!(
                                "{:<24}{:<16}{}{used}\n",
                                credential.name,
                                credential.kind,
                                credential.description.as_deref().unwrap_or_default()
                            ),
                        )?;
                    }
                }
                Ok(0)
            }
            ["credencial", "apagar", name] => match ci.delete_credential(name, std::env::var("USER").ok().as_deref()) {
                Ok(()) => {
                    write(out, &format!("{name}: revogada e apagada\n"))?;
                    Ok(0)
                }
                Err(why) => {
                    write(out, &format!("{why}\n"))?;
                    Ok(2)
                }
            },
            ["credencial", ..] => {
                write(out, &format!("{USAGE}\n"))?;
                Ok(2)
            }
            ["usuario", ..] => {
                write(out, &format!("{USAGE}\n"))?;
                Ok(2)
            }
            ["agentes", args @ ..] => crate::agentes::command(&ci, args, self.json, out),
            [name, args @ ..] => {
                let command = ci
                    .registry()
                    .on()
                    .iter()
                    .flat_map(|plugin| plugin.cli_commands())
                    .find(|c| c.name() == *name);
                match command {
                    Some(command) => {
                        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
                        command.run(&args, out)
                    }
                    None => {
                        let known: Vec<String> = ci
                            .registry()
                            .on()
                            .iter()
                            .flat_map(|plugin| plugin.cli_commands())
                            .map(|c| format!("  {:<22}{}", c.name(), c.summary()))
                            .collect();
                        let mut text = format!("não há o comando {name}: nenhum plugin ligado o traz\n");
                        if !known.is_empty() {
                            text.push_str("os comandos dos plugins ligados:\n");
                            text.push_str(&known.join("\n"));
                            text.push('\n');
                        }
                        write(out, &text)?;
                        Ok(2)
                    }
                }
            }
            [] => Ok(2),
        }
    }
}

fn json(value: &impl serde::Serialize) -> Result<String, CiError> {
    serde_json::to_string_pretty(value).map(|text| text + "\n").map_err(|e| CiError::failed(e.to_string()))
}

fn origin(view: &PluginView) -> &'static str {
    match view.origin {
        Origin::Embedded => "embutido",
        Origin::External(_) => "externo",
    }
}

fn points(view: &PluginView) -> String {
    if view.points.is_empty() {
        "-".to_owned()
    } else {
        view.points.iter().map(|p| p.name()).collect::<Vec<_>>().join(", ")
    }
}

fn permissions(list: &[Permission]) -> String {
    if list.is_empty() {
        "nenhuma".to_owned()
    } else {
        list.iter().map(|p| p.name()).collect::<Vec<_>>().join(", ")
    }
}

/// The list of plugins, one per line, with the problems found looking for them.
fn list(views: &[PluginView], problems: &[String]) -> String {
    let mut text = String::new();
    if views.is_empty() {
        text.push_str("nenhum plugin: o núcleo sobe, mas não tem o que rodar\n");
    }
    // The widest name and two spaces: `format-jenkinsfile` has 18 letters and ran into its version.
    let width = views.iter().map(|view| view.name.chars().count()).max().unwrap_or(0) + 2;
    for view in views {
        text.push_str(&format!(
            "{:<width$}{:<9}{:<10}{:<11}{}\n",
            view.name,
            view.version.as_deref().unwrap_or("-"),
            origin(view),
            match view.state {
                PluginState::On => "ligado",
                PluginState::Off => "desligado",
                PluginState::Refused(_) => "recusado",
            },
            points(view)
        ));
        if let PluginState::Refused(why) = &view.state {
            text.push_str(&format!("  {why}\n"));
        }
    }
    for problem in problems {
        text.push_str(&format!("não carregado: {problem}\n"));
    }
    text
}

/// One plugin in full.
fn info(view: &PluginView, audited: usize) -> String {
    let mut text = format!("{} {}\n", view.name, view.version.as_deref().unwrap_or("(sem versão)"));
    if !view.summary.is_empty() {
        text.push_str(&format!("  {}\n", view.summary));
    }
    let from = match &view.origin {
        Origin::Embedded => "embutido no servidor".to_owned(),
        Origin::External(folder) => format!("externo, em {}", folder.display()),
    };
    text.push_str(&format!("origem:        {from}\n"));
    text.push_str(&format!("estado:        {}\n", view.state.words()));
    text.push_str(&format!("pontos:        {}\n", points(view)));
    let mut needs: Vec<String> = view.dependencies.clone();
    needs.extend(view.requires.iter().map(|r| format!("{} ({})", r.plugin, r.accepts)));
    text.push_str(&format!(
        "depende de:    {}\n",
        if needs.is_empty() { "nada".to_owned() } else { needs.join(", ") }
    ));
    text.push_str(&format!(
        "permissões:    declaradas {}; concedidas {}\n",
        permissions(&view.declared),
        permissions(&view.granted)
    ));
    text.push_str(&format!("configuração:  {}\n", view.settings));
    let memory = match view.memory {
        Memory::Shared => "dividida com o servidor (embutido)".to_owned(),
        Memory::NotRunning => "0 (o programa não está rodando agora)".to_owned(),
        Memory::Bytes(bytes) => format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)),
        Memory::Unknown => "o sistema não diz".to_owned(),
    };
    text.push_str(&format!("memória:       {memory}\n"));
    text.push_str(&format!("auditoria:     {audited} pedidos registrados\n"));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn the_options_come_before_the_command_and_its_words_after() {
        let invocation = Invocation::parse(&args(&[
            "--dados",
            "/d",
            "--plugins",
            "/p",
            "--format",
            "json",
            "plugins",
            "listar",
        ]))
        .expect("reads");
        assert_eq!(invocation.data, PathBuf::from("/d"));
        assert_eq!(invocation.plugin_dirs, [PathBuf::from("/p")]);
        assert!(invocation.json);
        let command =
            Invocation::parse(&args(&["--dados", "/d", "converter", "--saida", "x"])).expect("reads");
        assert_eq!(command.words, ["converter", "--saida", "x"], "a plugin's command keeps its own flags");
        assert!(Invocation::parse(&args(&["--inventado"])).is_err());
        assert!(Invocation::parse(&args(&["--dados"])).is_err());
        assert!(Invocation::parse(&[]).is_err());
    }
}
