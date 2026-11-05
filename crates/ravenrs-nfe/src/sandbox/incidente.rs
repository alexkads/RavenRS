//! Reproduces a field problem offline, from the XML and the answer the SEFAZ really gave.
//!
//! It compares what the server said with what the catalog predicts, and says when it does not
//! reproduce. Everything this exports is masked. It sends nothing and signs nothing, so reading a
//! production document here does not give the POC any way of issuing one.

use std::fmt::Write as _;

use super::analise::{Analise, Diagnostico, Opcoes, analisar};
use super::autopsia::Evidencia;
use super::catalogo::{Alcance, Regra, regra, regras_do_c_stat};
use super::mascara::mascarar_xml;
use super::tempo::Instante;
use crate::retorno::descricao_c_stat;
use crate::xml;

/// The answer the SEFAZ really gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetornoReal {
    /// The status that decides.
    pub c_stat: u32,
    /// The text.
    pub x_motivo: String,
    /// The application that answered.
    pub ver_aplic: Option<String>,
    /// The protocol, if there is one.
    pub n_prot: Option<String>,
    /// When the server received it.
    pub dh_recbto: Option<String>,
}

/// What the comparison concluded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Veredito {
    /// The catalog predicts exactly what the SEFAZ answered.
    Reproduziu {
        /// The code.
        c_stat: u32,
        /// The rule that explains it.
        regra: &'static str,
    },
    /// The server accepted, and the catalog agrees.
    AutorizadaSemAchados {
        /// The code.
        c_stat: u32,
    },
    /// The server accepted, but the catalog would refuse. A rule of ours may be wrong, or optional
    /// in that state.
    AceitaMasOCatalogoRecusaria {
        /// The code from the server.
        c_stat: u32,
        /// The code the catalog predicted.
        previsto: u32,
    },
    /// The code depends on something the message does not carry, so this cannot prove it.
    ForaDoAlcance {
        /// The code from the server.
        c_stat: u32,
        /// The rule that would explain it.
        regra: &'static str,
        /// What is missing.
        falta: String,
    },
    /// The catalog has a rule for the code, and the rule does not fire on this message.
    NaoReproduziu {
        /// The code from the server.
        c_stat: u32,
        /// The first code the catalog predicted, if any.
        previsto: Option<u32>,
    },
    /// The catalog has no rule for this code. The cause is not known here.
    RegraFaltando {
        /// The code from the server.
        c_stat: u32,
        /// The first code the catalog predicted, if any.
        previsto: Option<u32>,
    },
    /// No answer was given: this only says what the catalog predicts.
    SemRetorno {
        /// The first code the catalog predicted, if any.
        previsto: Option<u32>,
    },
}

/// A field problem under examination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Incidente {
    /// What the catalog finds in the message.
    pub analise: Analise,
    /// The answer of the SEFAZ.
    pub retorno: Option<RetornoReal>,
    /// The conclusion.
    pub veredito: Veredito,
    /// The XML masked, ready to attach.
    pub xml_mascarado: String,
}

fn ler_retorno(texto: &str) -> Option<RetornoReal> {
    let raiz = xml::parse(texto).ok()?;
    let grupo = raiz
        .find("infProt")
        .or_else(|| raiz.find("retEvento").and_then(|r| r.child("infEvento")))
        .or_else(|| raiz.find("retEnviNFe"))
        .or_else(|| raiz.find("retEnvEvento"))
        .or_else(|| raiz.find("retConsSitNFe"))
        .or_else(|| raiz.find("retConsStatServ"))
        .or_else(|| raiz.find("retDistDFeInt"))?;
    Some(RetornoReal {
        c_stat: grupo.child_text("cStat")?.trim().parse().ok()?,
        x_motivo: grupo.child_text("xMotivo").unwrap_or_default(),
        ver_aplic: grupo.child_text("verAplic"),
        n_prot: grupo.child_text("nProt"),
        dh_recbto: grupo.child_text("dhRecbto"),
    })
}

/// Whether the code means the server took the message.
fn aceito(c_stat: u32) -> bool {
    matches!(c_stat, 100 | 101 | 104 | 107 | 128 | 135 | 136 | 137 | 138 | 150 | 155)
}

fn julgar(analise: &Analise, retorno: Option<&RetornoReal>) -> Veredito {
    let previsto = analise.primeira_rejeicao().map(|d| d.c_stat);
    let Some(retorno) = retorno else {
        return Veredito::SemRetorno { previsto };
    };
    let real = retorno.c_stat;
    if aceito(real) {
        return match previsto {
            None => Veredito::AutorizadaSemAchados { c_stat: real },
            Some(previsto) => Veredito::AceitaMasOCatalogoRecusaria { c_stat: real, previsto },
        };
    }
    if let Some(achado) = analise.diagnosticos.iter().find(|d| d.c_stat == real) {
        return Veredito::Reproduziu { c_stat: real, regra: achado.regra };
    }
    let regras: Vec<&Regra> = regras_do_c_stat(real).collect();
    if let Some(regra) = regras.iter().find(|r| r.alcance != Alcance::Mensagem) {
        let falta = match regra.alcance {
            Alcance::EstadoDaSefaz => "o que a SEFAZ já guarda desta chave ou número",
            Alcance::Cadastro => "o cadastro de contribuintes e a lista de confiança da SEFAZ",
            Alcance::Relogio => "a hora em que a SEFAZ recebeu a mensagem",
            Alcance::Servico => "o estado do serviço naquele momento",
            Alcance::Mensagem => "",
        };
        return Veredito::ForaDoAlcance { c_stat: real, regra: regra.id, falta: falta.to_string() };
    }
    if regras.is_empty() {
        return Veredito::RegraFaltando { c_stat: real, previsto };
    }
    Veredito::NaoReproduziu { c_stat: real, previsto }
}

impl Incidente {
    /// Examines a document (`NFe`, `nfeProc` or SOAP) and the answer of the SEFAZ, with no network.
    ///
    /// With no `retorno` given, the protocol inside an `nfeProc` is used when there is one.
    #[must_use]
    pub fn de_producao(xml_texto: &str, retorno: Option<&str>) -> Self {
        let retorno = retorno.and_then(ler_retorno).or_else(|| ler_retorno(xml_texto));
        let agora = retorno.as_ref().and_then(|r| r.dh_recbto.as_deref()).and_then(Instante::de_xml);
        // The document says which environment it was for: the point here is not to judge that.
        let opcoes = Opcoes { agora, ..Opcoes::default() };
        let analise = analisar(xml_texto, &opcoes);
        let veredito = julgar(&analise, retorno.as_ref());
        Self { analise, retorno, veredito, xml_mascarado: mascarar_xml(xml_texto) }
    }

    /// The conclusion, in one sentence, in pt-BR.
    #[must_use]
    pub fn conclusao(&self) -> String {
        match &self.veredito {
            Veredito::Reproduziu { c_stat, regra } => {
                format!("Reproduziu: a análise offline chega ao mesmo cStat {c_stat}, pela regra {regra}.")
            }
            Veredito::AutorizadaSemAchados { c_stat } => {
                format!("A SEFAZ aceitou (cStat {c_stat}) e o catálogo não acha nada contra.")
            }
            Veredito::AceitaMasOCatalogoRecusaria { c_stat, previsto } => format!(
                "A SEFAZ aceitou (cStat {c_stat}), mas o catálogo previa {previsto}: a regra pode ser opcional naquela UF, ou estar errada aqui."
            ),
            Veredito::ForaDoAlcance { c_stat, regra, falta } => format!(
                "Não dá para reproduzir o cStat {c_stat} só com a mensagem: a regra {regra} depende de {falta}."
            ),
            Veredito::NaoReproduziu { c_stat, previsto } => format!(
                "Não reproduziu: há regra para o cStat {c_stat}, mas ela não dispara nesta mensagem{}. Pode ser diferença entre o XML guardado e o enviado.",
                previsto.map_or(String::new(), |p| format!("; o catálogo previa {p}"))
            ),
            Veredito::RegraFaltando { c_stat, previsto } => format!(
                "Não reproduziu: o catálogo não tem regra para o cStat {c_stat}{}. A causa não é conhecida aqui, e não será inventada.",
                previsto.map_or(String::new(), |p| format!("; ele previa {p}, que é outra coisa"))
            ),
            Veredito::SemRetorno { previsto: Some(previsto) } => {
                format!("Sem retorno da SEFAZ; o catálogo prevê o cStat {previsto}.")
            }
            Veredito::SemRetorno { previsto: None } => {
                "Sem retorno da SEFAZ; o catálogo não acha nada contra esta mensagem.".to_string()
            }
        }
    }

    /// The report of the incident, in Markdown, masked.
    #[must_use]
    pub fn relatorio_markdown(&self) -> String {
        let mut texto = String::from("# Relatório de incidente fiscal\n\n");
        texto.push_str("Gerado pelo ravenrs-nfe, prova de conceito beta. Não é parecer fiscal.\n\n");

        texto.push_str("## O que aconteceu\n\n");
        if let Some(chave) = &self.analise.chave {
            let _ = writeln!(texto, "- Chave: `{chave}`");
        }
        let _ = writeln!(texto, "- Documento lido como: `{}`", self.analise.tipo);
        match &self.retorno {
            Some(retorno) => {
                let _ = writeln!(
                    texto,
                    "- Retorno da SEFAZ: cStat {} ({})",
                    retorno.c_stat,
                    mascarar_motivo(&retorno.x_motivo)
                );
                if let Some(n_prot) = &retorno.n_prot {
                    let _ = writeln!(texto, "- Protocolo: `{n_prot}`");
                }
            }
            None => texto.push_str("- Nenhum retorno da SEFAZ foi informado.\n"),
        }
        let _ = writeln!(texto, "\n**{}**\n", self.conclusao());

        texto.push_str("## Causa\n\n");
        let causa = self.causa_principal();
        match causa {
            Some(achado) => {
                let _ = writeln!(texto, "**cStat {} — {}**\n", achado.c_stat, achado.titulo);
                let _ = writeln!(texto, "{}\n", achado.explicacao);
                let _ = writeln!(texto, "- Campo: `{}`", achado.caminho);
                if let Some(encontrado) = &achado.encontrado {
                    let _ = writeln!(texto, "- Encontrado: `{encontrado}`");
                }
                if let Some(esperado) = &achado.esperado {
                    let _ = writeln!(texto, "- Esperado: `{esperado}`");
                }
            }
            None => texto.push_str("Nenhuma causa foi apontada a partir da mensagem.\n"),
        }
        if let Some(autopsia) = &self.analise.autopsia
            && !autopsia.causas.is_empty()
        {
            texto.push_str("\n### Autópsia da assinatura\n\n");
            let _ = writeln!(
                texto,
                "- Digest informado: `{}`",
                autopsia.digest_informado.as_deref().unwrap_or("-")
            );
            let _ = writeln!(
                texto,
                "- Digest calculado: `{}`",
                autopsia.digest_calculado.as_deref().unwrap_or("-")
            );
            if let Some(divergencia) = &autopsia.divergencia {
                let _ = writeln!(texto, "- Primeiro byte divergente: {}", divergencia.byte);
            }
            for causa in &autopsia.causas {
                let forca = if causa.evidencia == Evidencia::Comprovada { "comprovada" } else { "indício" };
                let _ = writeln!(texto, "- {forca}: {}", causa.explicacao);
            }
        }

        texto.push_str("\n## Correção\n\n");
        match causa {
            Some(achado) => {
                let _ = writeln!(texto, "{}", achado.como_corrigir);
            }
            None => texto.push_str("Nada a sugerir sem uma causa.\n"),
        }

        texto.push_str("\n## Fonte\n\n");
        match causa {
            Some(achado) => {
                let _ = writeln!(texto, "{} ({}).", achado.fonte, achado.confianca);
            }
            None => texto.push_str("Sem regra aplicável.\n"),
        }
        if self.analise.diagnosticos.len() > 1 {
            texto.push_str(
                "\n### Todos os achados\n\n| cStat | Regra | Campo | Confiança |\n|---|---|---|---|\n",
            );
            for achado in &self.analise.diagnosticos {
                let _ = writeln!(
                    texto,
                    "| {} | {} | `{}` | {} |",
                    achado.c_stat, achado.regra, achado.caminho, achado.confianca
                );
            }
        }

        texto.push_str("\n## O que não foi possível concluir\n\n");
        let mut limites = self.analise.nao_avaliado.clone();
        limites.dedup();
        if let Veredito::RegraFaltando { c_stat, .. } = &self.veredito {
            let nome = descricao_c_stat(*c_stat).unwrap_or("código fora da tabela lida");
            limites.insert(
                0,
                format!("O cStat {c_stat} ({nome}) não tem regra no catálogo: a causa fica em aberto."),
            );
        }
        if limites.is_empty() {
            texto.push_str("Nada ficou de fora desta análise.\n");
        }
        for limite in limites {
            let _ = writeln!(texto, "- {limite}");
        }
        texto
    }

    /// The finding that matches the answer, or the first refusal.
    fn causa_principal(&self) -> Option<&Diagnostico> {
        let real = self.retorno.as_ref().map(|r| r.c_stat);
        self.analise.diagnosticos.iter().find(|d| Some(d.c_stat) == real).or_else(|| {
            if matches!(
                self.veredito,
                Veredito::RegraFaltando { .. }
                    | Veredito::NaoReproduziu { .. }
                    | Veredito::ForaDoAlcance { .. }
            ) {
                None
            } else {
                self.analise.primeira_rejeicao()
            }
        })
    }
}

/// The text of a return can quote a document number, so it goes through the same scrubbing.
fn mascarar_motivo(texto: &str) -> String {
    mascarar_xml(&format!("cru:{texto}")).trim_start_matches("cru:").to_string()
}

/// Says where a rule of the catalog can be looked up, for a report written elsewhere.
#[must_use]
pub fn fonte_da_regra(id: &str) -> Option<String> {
    regra(id).map(|r| format!("{} ({})", r.fonte, r.confianca))
}
