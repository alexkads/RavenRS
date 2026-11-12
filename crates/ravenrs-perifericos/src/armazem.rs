//! Where the installed drivers are kept.

use std::fmt;
use std::sync::Mutex;

/// Why the installed drivers could not be read or saved.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroArmazem {
    /// The file or the medium failed.
    Acesso(String),
    /// What is saved is not valid.
    Conteudo(String),
}

impl fmt::Display for ErroArmazem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acesso(motivo) => write!(f, "não foi possível acessar os drivers instalados: {motivo}"),
            Self::Conteudo(motivo) => write!(f, "os drivers instalados estão inválidos: {motivo}"),
        }
    }
}

impl std::error::Error for ErroArmazem {}

/// Somewhere to keep the registry.
pub trait Armazem {
    /// What is saved, or `None` if nothing was yet.
    fn ler(&self) -> Result<Option<Vec<u8>>, ErroArmazem>;

    /// Replaces what is saved. If this fails, what was there stays whole.
    fn gravar(&self, bytes: &[u8]) -> Result<(), ErroArmazem>;
}

/// Keeps it in memory, for tests and the browser.
#[derive(Debug, Default)]
pub struct ArmazemMemoria(Mutex<Option<Vec<u8>>>);

impl Armazem for ArmazemMemoria {
    fn ler(&self) -> Result<Option<Vec<u8>>, ErroArmazem> {
        let guardado = self.0.lock().map_err(|_| ErroArmazem::Acesso("trava perdida".to_owned()))?;
        Ok(guardado.clone())
    }

    fn gravar(&self, bytes: &[u8]) -> Result<(), ErroArmazem> {
        let mut guardado = self.0.lock().map_err(|_| ErroArmazem::Acesso("trava perdida".to_owned()))?;
        *guardado = Some(bytes.to_vec());
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use arquivo::ArmazemArquivo;

#[cfg(not(target_arch = "wasm32"))]
mod arquivo {
    use std::fs::{self, File};
    use std::io::{ErrorKind, Write as _};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{Armazem, ErroArmazem};

    static PROXIMO: AtomicU64 = AtomicU64::new(0);

    /// Keeps it in a file. A write goes to a new file beside it, reaches the disk, and only then
    /// takes its place, so a power cut leaves the old file or the new one, never half of one.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ArmazemArquivo {
        caminho: PathBuf,
    }

    impl ArmazemArquivo {
        /// A file at this path. Its folder is created when needed.
        pub fn novo(caminho: impl Into<PathBuf>) -> Self {
            Self { caminho: caminho.into() }
        }

        fn falha(&self, erro: &std::io::Error) -> ErroArmazem {
            ErroArmazem::Acesso(format!("{}: {erro}", self.caminho.display()))
        }

        fn gravar_em(&self, temporario: &Path, bytes: &[u8]) -> std::io::Result<()> {
            let mut arquivo = File::create_new(temporario)?;
            arquivo.write_all(bytes)?;
            arquivo.sync_all()?;
            fs::rename(temporario, &self.caminho)
        }
    }

    impl Armazem for ArmazemArquivo {
        fn ler(&self) -> Result<Option<Vec<u8>>, ErroArmazem> {
            match fs::read(&self.caminho) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(erro) if erro.kind() == ErrorKind::NotFound => Ok(None),
                Err(erro) => Err(self.falha(&erro)),
            }
        }

        fn gravar(&self, bytes: &[u8]) -> Result<(), ErroArmazem> {
            let pasta = self.caminho.parent().filter(|pasta| !pasta.as_os_str().is_empty());
            if let Some(pasta) = pasta {
                fs::create_dir_all(pasta).map_err(|erro| self.falha(&erro))?;
            }
            let nome = self.caminho.file_name().map_or("drivers".into(), |nome| nome.to_string_lossy());
            let numero = PROXIMO.fetch_add(1, Ordering::Relaxed);
            let temporario =
                self.caminho.with_file_name(format!(".{nome}.{}.{numero}.tmp", std::process::id()));
            let gravado = self.gravar_em(&temporario, bytes);
            if let Err(erro) = gravado {
                let _ = fs::remove_file(&temporario);
                return Err(self.falha(&erro));
            }
            // The rename reaches the disk with its folder. Not every system can do this, and the
            // file is whole either way.
            if let Some(pasta) = pasta
                && let Ok(aberta) = File::open(pasta)
            {
                let _ = aberta.sync_all();
            }
            Ok(())
        }
    }
}
