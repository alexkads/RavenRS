//! Where the bytes go. Printers are reached the way the operating system offers; in the browser there
//! is none of them, so only the trait exists there.

use crate::error::LabelError;

/// Somewhere a compiled label can go.
pub trait LabelOutput {
    /// Hands over the bytes as they are.
    fn send(&self, bytes: &[u8]) -> Result<(), LabelError>;
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{CupsOutput, FileOutput, TcpOutput};
#[cfg(windows)]
pub use windows::WindowsShareOutput;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::io::Write as _;
    use std::net::{Shutdown, TcpStream, ToSocketAddrs as _};
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    use super::{LabelError, LabelOutput};

    fn failed(what: &str, error: &std::io::Error) -> LabelError {
        LabelError::Output(format!("{what}: {error}"))
    }

    /// A printer on the network, on the raw port.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct TcpOutput {
        /// Its name or address.
        pub host: String,
        /// The port, 9100 on almost every label printer.
        pub port: u16,
        /// How long to wait to connect and to write.
        pub timeout: Duration,
    }

    impl TcpOutput {
        /// The usual port, waiting five seconds.
        pub fn new(host: impl Into<String>) -> Self {
            Self { host: host.into(), port: 9100, timeout: Duration::from_secs(5) }
        }
    }

    impl LabelOutput for TcpOutput {
        fn send(&self, bytes: &[u8]) -> Result<(), LabelError> {
            let place = format!("{}:{}", self.host, self.port);
            let addresses = (self.host.as_str(), self.port)
                .to_socket_addrs()
                .map_err(|error| failed(&format!("endereço {place}"), &error))?;
            let mut last = None;
            for address in addresses {
                match TcpStream::connect_timeout(&address, self.timeout) {
                    Ok(mut stream) => {
                        let write = |stream: &mut TcpStream| {
                            stream.set_write_timeout(Some(self.timeout))?;
                            stream.write_all(bytes)?;
                            stream.flush()
                        };
                        write(&mut stream).map_err(|error| failed(&format!("envio a {place}"), &error))?;
                        // The printer may have gone by now, and it has the bytes.
                        let _ = stream.shutdown(Shutdown::Both);
                        return Ok(());
                    }
                    Err(error) => last = Some(error),
                }
            }
            Err(match last {
                Some(error) => failed(&format!("conexão com {place}"), &error),
                None => LabelError::Output(format!("{place} não tem endereço")),
            })
        }
    }

    /// A printer known to the system's print service (CUPS), without any filter on the way.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CupsOutput {
        name: String,
        program: String,
    }

    impl CupsOutput {
        /// A destination by name. Only letters, digits, `-`, `_` and `.` are taken, starting with a
        /// letter or digit: it goes to a command's arguments.
        pub fn new(name: impl Into<String>) -> Result<Self, LabelError> {
            let name = name.into();
            let valid = name.chars().next().is_some_and(|first| first.is_ascii_alphanumeric())
                && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
            if !valid {
                return Err(LabelError::InvalidDestination(format!(
                    "o nome de impressora \"{name}\" só pode ter letras, números, - _ e ."
                )));
            }
            Ok(Self { name, program: "lp".to_owned() })
        }

        /// The destination's name.
        pub fn name(&self) -> &str {
            &self.name
        }
    }

    impl LabelOutput for CupsOutput {
        fn send(&self, bytes: &[u8]) -> Result<(), LabelError> {
            let mut child = Command::new(&self.program)
                .args(["-d", &self.name, "-o", "raw"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| failed("não foi possível executar lp", &error))?;
            let written = match child.stdin.take() {
                Some(mut stdin) => stdin.write_all(bytes),
                None => Ok(()),
            };
            let result = child.wait_with_output().map_err(|error| failed("lp", &error))?;
            if !result.status.success() {
                let said = String::from_utf8_lossy(&result.stderr);
                return Err(LabelError::Output(format!("lp recusou: {}", said.trim())));
            }
            written.map_err(|error| failed("envio ao lp", &error))
        }
    }

    /// A `.prn` file, to send later by hand or to look at.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct FileOutput {
        /// Where to write. An existing file is replaced.
        pub path: PathBuf,
    }

    impl FileOutput {
        /// A file at `path`.
        pub fn new(path: impl Into<PathBuf>) -> Self {
            Self { path: path.into() }
        }
    }

    impl LabelOutput for FileOutput {
        fn send(&self, bytes: &[u8]) -> Result<(), LabelError> {
            std::fs::write(&self.path, bytes)
                .map_err(|error| failed(&format!("arquivo {}", self.path.display()), &error))
        }
    }

    #[cfg(test)]
    mod tests {
        use std::io::Read as _;
        use std::net::TcpListener;

        use super::*;

        fn scratch(name: &str) -> PathBuf {
            std::env::temp_dir().join(format!("ravenrs-report-label-{}-{name}", std::process::id()))
        }

        #[test]
        fn tcp_delivers_the_bytes() {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("{e}"));
            let port = listener.local_addr().unwrap_or_else(|e| panic!("{e}")).port();
            let receiver = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap_or_else(|e| panic!("{e}"));
                let mut received = Vec::new();
                stream.read_to_end(&mut received).unwrap_or_else(|e| panic!("{e}"));
                received
            });
            let output = TcpOutput { port, ..TcpOutput::new("127.0.0.1") };
            assert_eq!(output.send(b"^XA^XZ"), Ok(()));
            assert_eq!(receiver.join().unwrap_or_else(|_| panic!("the receiver failed")), b"^XA^XZ");
        }

        #[test]
        fn tcp_says_when_nobody_answers() {
            // A port just freed: nothing listens there.
            let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("{e}"));
            let port = listener.local_addr().unwrap_or_else(|e| panic!("{e}")).port();
            drop(listener);
            let output = TcpOutput { port, ..TcpOutput::new("127.0.0.1") };
            assert!(matches!(output.send(b"x"), Err(LabelError::Output(_))));
        }

        #[test]
        fn cups_names_cannot_become_arguments_or_shell() {
            for name in ["", "-o", "a b", "a;rm", "a/b", "$(x)", "a\nb"] {
                assert!(matches!(CupsOutput::new(name), Err(LabelError::InvalidDestination(_))), "{name:?}");
            }
            assert!(CupsOutput::new("Elgin_L42-Pro.1").is_ok());
        }

        #[cfg(unix)]
        #[test]
        fn cups_pipes_the_bytes_to_the_command_with_raw() {
            use std::os::unix::fs::PermissionsExt as _;
            let (script, received) = (scratch("lp.sh"), scratch("lp.out"));
            let body = format!("#!/bin/sh\nprintf '%s ' \"$@\" > '{0}'\ncat >> '{0}'\n", received.display());
            std::fs::write(&script, body).unwrap_or_else(|e| panic!("{e}"));
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|e| panic!("{e}"));
            let output = CupsOutput {
                program: script.display().to_string(),
                ..CupsOutput::new("Etiquetas").unwrap_or_else(|e| panic!("{e}"))
            };
            let sent = output.send(b"^XA^XZ");
            let got = std::fs::read_to_string(&received).unwrap_or_default();
            let _ = std::fs::remove_file(&script);
            let _ = std::fs::remove_file(&received);
            assert_eq!(sent, Ok(()));
            assert_eq!(got, "-d Etiquetas -o raw ^XA^XZ");
        }

        #[test]
        fn cups_says_when_the_command_is_missing() {
            let output = CupsOutput {
                program: "/nao/existe/lp".to_owned(),
                ..CupsOutput::new("Etiquetas").unwrap_or_else(|e| panic!("{e}"))
            };
            assert!(matches!(output.send(b"x"), Err(LabelError::Output(_))));
        }

        #[test]
        fn a_file_gets_the_bytes() {
            let path = scratch("etiqueta.prn");
            let sent = FileOutput::new(&path).send(b"^XA^XZ");
            let got = std::fs::read(&path).unwrap_or_default();
            let _ = std::fs::remove_file(&path);
            assert_eq!(sent, Ok(()));
            assert_eq!(got, b"^XA^XZ");
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::io::Write as _;

    use super::{LabelError, LabelOutput};

    /// A printer shared on the network, as `\\host\share`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct WindowsShareOutput {
        path: String,
    }

    impl WindowsShareOutput {
        /// A share by its host and name. Slashes and other path characters are refused.
        pub fn new(host: &str, share: &str) -> Result<Self, LabelError> {
            let valid = |part: &str| {
                !part.is_empty()
                    && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' '))
            };
            if !valid(host) || !valid(share) {
                return Err(LabelError::InvalidDestination(format!(
                    "compartilhamento \\\\{host}\\{share} com caracteres não aceitos"
                )));
            }
            Ok(Self { path: format!(r"\\{host}\{share}") })
        }
    }

    impl LabelOutput for WindowsShareOutput {
        fn send(&self, bytes: &[u8]) -> Result<(), LabelError> {
            let failed = |error: std::io::Error| LabelError::Output(format!("{}: {error}", self.path));
            let mut share = std::fs::OpenOptions::new().write(true).open(&self.path).map_err(failed)?;
            share.write_all(bytes).map_err(failed)?;
            share.flush().map_err(failed)
        }
    }
}
