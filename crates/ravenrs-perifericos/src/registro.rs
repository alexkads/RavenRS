//! The drivers installed on this machine, which one of each is active, and how it got there.

use std::collections::BTreeMap;
use std::fmt;

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::armazem::{Armazem, ErroArmazem};
use crate::catalogo::{Atualizacao, Catalogo, ChavePublica, ErroCatalogo};
use crate::embutidos::manifestos_embutidos;
use crate::manifesto::{ErroManifesto, Manifesto};
use crate::transporte::TransporteCatalogo;

const FORMATO: u32 = 1;
/// How many events of a driver are kept.
const MAXIMO_HISTORICO: usize = 200;

/// What happened to a driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcaoHistorico {
    /// A version arrived.
    Instalado,
    /// A version became the one in use.
    Ativado,
    /// The one in use went back to the previous.
    Voltou,
}

/// One line of a driver's history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evento {
    /// What happened.
    pub acao: AcaoHistorico,
    /// To which version.
    pub versao: Version,
}

/// A device an application keeps: this is all it needs to save.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dispositivo {
    /// The name people call it.
    pub apelido: String,
    /// Its driver.
    pub driver_id: String,
    /// A version to stay on. `None` follows the active one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versao_fixada: Option<Version>,
    /// Where it is, as the application's output understands it.
    pub conexao: String,
}

/// Why the registry refused.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroRegistro {
    /// The manifest is not valid.
    ManifestoInvalido(ErroManifesto),
    /// The driver asks for a newer program.
    ProgramaAntigo {
        /// The driver.
        id: String,
        /// Its version.
        versao: String,
        /// What it asks for.
        exige: String,
        /// What is running.
        programa: String,
    },
    /// This version is installed with different contents. A version never changes.
    VersaoConflitante {
        /// The driver.
        id: String,
        /// Its version.
        versao: Version,
    },
    /// No driver with this name.
    DriverDesconhecido(String),
    /// This driver does not have this version here.
    VersaoNaoInstalada {
        /// The driver.
        id: String,
        /// The version.
        versao: Version,
    },
    /// There is no previous version to go back to.
    SemAnterior(String),
    /// The catalog does not have what was accepted.
    ForaDoCatalogo {
        /// The driver.
        id: String,
        /// The version.
        versao: Version,
    },
}

impl fmt::Display for ErroRegistro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestoInvalido(erro) => write!(f, "driver inválido: {erro}"),
            Self::ProgramaAntigo { id, versao, exige, programa } => write!(
                f,
                "o driver {id} {versao} precisa do programa {exige} ou mais novo; este é o {programa}. \
                 Atualize o programa antes."
            ),
            Self::VersaoConflitante { id, versao } => {
                write!(f, "o driver {id} {versao} já está instalado com outro conteúdo")
            }
            Self::DriverDesconhecido(id) => write!(f, "o driver {id} não está instalado"),
            Self::VersaoNaoInstalada { id, versao } => {
                write!(f, "a versão {versao} do driver {id} não está instalada")
            }
            Self::SemAnterior(id) => write!(f, "o driver {id} não tem versão anterior para voltar"),
            Self::ForaDoCatalogo { id, versao } => {
                write!(f, "o catálogo não traz o driver {id} {versao}")
            }
        }
    }
}

impl std::error::Error for ErroRegistro {}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct DriverInstalado {
    versoes: Vec<Manifesto>,
    /// The versions in the order they became active; the last one is in use.
    ativacoes: Vec<Version>,
    historico: Vec<Evento>,
}

impl DriverInstalado {
    fn registrar(&mut self, acao: AcaoHistorico, versao: &Version) {
        self.historico.push(Evento { acao, versao: versao.clone() });
        if self.historico.len() > MAXIMO_HISTORICO {
            self.historico.remove(0);
        }
    }

    fn versao(&self, versao: &Version) -> Option<&Manifesto> {
        self.versoes.iter().find(|manifesto| manifesto.versao == *versao)
    }
}

#[derive(Serialize, Deserialize)]
struct EstadoGravado {
    formato: u32,
    ultima_sequencia: u64,
    drivers: BTreeMap<String, DriverInstalado>,
}

/// The drivers on this machine.
#[derive(Clone, Debug, PartialEq)]
pub struct Registro {
    programa: Version,
    ultima_sequencia: u64,
    drivers: BTreeMap<String, DriverInstalado>,
}

impl Registro {
    /// A registry with nothing installed, for this version of the program.
    pub fn vazio(programa: Version) -> Self {
        Self { programa, ultima_sequencia: 0, drivers: BTreeMap::new() }
    }

    /// A registry with the drivers that come with the program.
    pub fn novo(programa: Version) -> Self {
        let mut registro = Self::vazio(programa);
        registro.trazer_embutidos();
        registro
    }

    /// Brings in what came with the program. Each new driver starts active, and a newer version of
    /// one already here stays only installed, so a program update does not change the one in use.
    fn trazer_embutidos(&mut self) {
        for manifesto in manifestos_embutidos() {
            // A saved copy of the same version stays, and one that needs a newer program waits.
            let _ = self.instalar(manifesto);
        }
    }

    /// The registry saved in `armazem`, plus what came with the program. With nothing saved, the same
    /// as [`Self::novo`]. Every saved manifest is checked again.
    pub fn carregar(programa: Version, armazem: &dyn Armazem) -> Result<Self, ErroArmazem> {
        let mut registro = Self::vazio(programa);
        if let Some(bytes) = armazem.ler()? {
            let estado: EstadoGravado =
                serde_json::from_slice(&bytes).map_err(|erro| ErroArmazem::Conteudo(erro.to_string()))?;
            if estado.formato != FORMATO {
                return Err(ErroArmazem::Conteudo(format!("formato {} desconhecido", estado.formato)));
            }
            for (id, driver) in &estado.drivers {
                for manifesto in &driver.versoes {
                    manifesto.validar().map_err(|erro| ErroArmazem::Conteudo(erro.to_string()))?;
                    if manifesto.id != *id {
                        return Err(ErroArmazem::Conteudo(format!("{} guardado como {id}", manifesto.id)));
                    }
                }
                let conhecidas = driver.ativacoes.iter().all(|versao| driver.versao(versao).is_some());
                if !conhecidas || driver.ativacoes.is_empty() {
                    return Err(ErroArmazem::Conteudo(format!("ativações de {id} sem versão instalada")));
                }
            }
            registro.ultima_sequencia = estado.ultima_sequencia;
            registro.drivers = estado.drivers;
        }
        registro.trazer_embutidos();
        Ok(registro)
    }

    /// Saves everything in `armazem`.
    pub fn salvar(&self, armazem: &dyn Armazem) -> Result<(), ErroArmazem> {
        let estado = EstadoGravado {
            formato: FORMATO,
            ultima_sequencia: self.ultima_sequencia,
            drivers: self.drivers.clone(),
        };
        let bytes =
            serde_json::to_vec_pretty(&estado).map_err(|erro| ErroArmazem::Conteudo(erro.to_string()))?;
        armazem.gravar(&bytes)
    }

    /// The program's version it works for.
    pub fn programa(&self) -> &Version {
        &self.programa
    }

    /// The highest catalog sequence seen, so an older catalog is not accepted again.
    pub fn ultima_sequencia(&self) -> u64 {
        self.ultima_sequencia
    }

    fn exigir_compativel(&self, manifesto: &Manifesto) -> Result<(), ErroRegistro> {
        match &manifesto.min_versao_programa {
            Some(exige) if !manifesto.compativel_com(&self.programa) => Err(ErroRegistro::ProgramaAntigo {
                id: manifesto.id.clone(),
                versao: manifesto.versao.to_string(),
                exige: exige.to_string(),
                programa: self.programa.to_string(),
            }),
            _ => Ok(()),
        }
    }

    /// Adds a version. The first version of a driver becomes active; later ones wait for
    /// [`Self::ativar`]. The same version again changes nothing.
    pub fn instalar(&mut self, manifesto: Manifesto) -> Result<(), ErroRegistro> {
        manifesto.validar().map_err(ErroRegistro::ManifestoInvalido)?;
        self.exigir_compativel(&manifesto)?;
        let driver = self.drivers.entry(manifesto.id.clone()).or_default();
        if let Some(existente) = driver.versao(&manifesto.versao) {
            if *existente == manifesto {
                return Ok(());
            }
            return Err(ErroRegistro::VersaoConflitante { id: manifesto.id, versao: manifesto.versao });
        }
        driver.registrar(AcaoHistorico::Instalado, &manifesto.versao);
        if driver.ativacoes.is_empty() {
            driver.ativacoes.push(manifesto.versao.clone());
            driver.registrar(AcaoHistorico::Ativado, &manifesto.versao);
        }
        driver.versoes.push(manifesto);
        Ok(())
    }

    /// Puts an installed version in use.
    pub fn ativar(&mut self, id: &str, versao: &Version) -> Result<(), ErroRegistro> {
        let driver = self.drivers.get(id).ok_or_else(|| ErroRegistro::DriverDesconhecido(id.to_owned()))?;
        let manifesto = driver
            .versao(versao)
            .ok_or_else(|| ErroRegistro::VersaoNaoInstalada { id: id.to_owned(), versao: versao.clone() })?;
        self.exigir_compativel(manifesto)?;
        let driver =
            self.drivers.get_mut(id).ok_or_else(|| ErroRegistro::DriverDesconhecido(id.to_owned()))?;
        if driver.ativacoes.last() != Some(versao) {
            driver.ativacoes.push(versao.clone());
            driver.registrar(AcaoHistorico::Ativado, versao);
        }
        Ok(())
    }

    /// Goes back to the version in use before this one, and returns it.
    pub fn voltar(&mut self, id: &str) -> Result<Version, ErroRegistro> {
        let driver = self.drivers.get(id).ok_or_else(|| ErroRegistro::DriverDesconhecido(id.to_owned()))?;
        let [.., anterior, _] = driver.ativacoes.as_slice() else {
            return Err(ErroRegistro::SemAnterior(id.to_owned()));
        };
        let anterior = anterior.clone();
        let manifesto = driver.versao(&anterior).ok_or_else(|| ErroRegistro::VersaoNaoInstalada {
            id: id.to_owned(),
            versao: anterior.clone(),
        })?;
        self.exigir_compativel(manifesto)?;
        let driver =
            self.drivers.get_mut(id).ok_or_else(|| ErroRegistro::DriverDesconhecido(id.to_owned()))?;
        driver.ativacoes.pop();
        driver.registrar(AcaoHistorico::Voltou, &anterior);
        Ok(anterior)
    }

    /// The version in use.
    pub fn ativo(&self, id: &str) -> Option<&Manifesto> {
        let driver = self.drivers.get(id)?;
        driver.versao(driver.ativacoes.last()?)
    }

    /// The installed versions, in the order they arrived.
    pub fn versoes(&self, id: &str) -> &[Manifesto] {
        self.drivers.get(id).map_or(&[], |driver| driver.versoes.as_slice())
    }

    /// What happened to a driver, oldest first.
    pub fn historico(&self, id: &str) -> &[Evento] {
        self.drivers.get(id).map_or(&[], |driver| driver.historico.as_slice())
    }

    /// The version in use of every driver.
    pub fn ativos(&self) -> impl Iterator<Item = &Manifesto> {
        self.drivers.keys().filter_map(|id| self.ativo(id))
    }

    /// The driver a device runs on: its pinned version, or the active one.
    pub fn driver_de(&self, dispositivo: &Dispositivo) -> Result<&Manifesto, ErroRegistro> {
        let id = &dispositivo.driver_id;
        let manifesto = match &dispositivo.versao_fixada {
            Some(versao) => {
                self.versoes(id).iter().find(|manifesto| manifesto.versao == *versao).ok_or_else(|| {
                    ErroRegistro::VersaoNaoInstalada { id: id.clone(), versao: versao.clone() }
                })?
            }
            None => self.ativo(id).ok_or_else(|| ErroRegistro::DriverDesconhecido(id.clone()))?,
        };
        self.exigir_compativel(manifesto)?;
        Ok(manifesto)
    }

    /// Checks a signed catalog against the last one seen, and remembers its sequence.
    pub fn abrir_catalogo(
        &mut self,
        bytes: &[u8],
        chave: &ChavePublica,
        hoje: &str,
    ) -> Result<Catalogo, ErroCatalogo> {
        let catalogo = Catalogo::abrir(bytes, chave, hoje, self.ultima_sequencia)?;
        self.ultima_sequencia = catalogo.sequencia();
        Ok(catalogo)
    }

    /// Fetches a catalog and does the same as [`Self::abrir_catalogo`].
    pub fn buscar_catalogo(
        &mut self,
        transporte: &dyn TransporteCatalogo,
        chave: &ChavePublica,
        hoje: &str,
    ) -> Result<Catalogo, ErroCatalogo> {
        let bytes = transporte.baixar().map_err(ErroCatalogo::Transporte)?;
        self.abrir_catalogo(&bytes, chave, hoje)
    }

    /// Installs what a person accepted and puts it in use. [`Self::voltar`] undoes it.
    pub fn aceitar(&mut self, catalogo: &Catalogo, atualizacao: &Atualizacao) -> Result<(), ErroRegistro> {
        let (id, versao) = (&atualizacao.id, &atualizacao.para);
        let item =
            catalogo.itens().iter().find(|item| item.manifesto.id == *id && item.manifesto.versao == *versao);
        let item =
            item.ok_or_else(|| ErroRegistro::ForaDoCatalogo { id: id.clone(), versao: versao.clone() })?;
        self.instalar(item.manifesto.clone())?;
        self.ativar(id, versao)
    }
}
