//! A real serial port. Never exercised against a real pinpad.

use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

use super::driver::PortaPinpad;

/// The speed the specification fixes when the medium has one.
const VELOCIDADE: u32 = 19_200;
/// How long one read waits. The session counts the real time limits.
const ESPERA_LEITURA: Duration = Duration::from_millis(50);

/// A pinpad on a serial port: 19200 bps, 8 bits, no parity, 1 stop bit.
#[derive(Debug)]
pub struct PortaSerial {
    porta: Box<dyn serialport::SerialPort>,
    inicio: Instant,
}

impl PortaSerial {
    /// Opens a port, such as `COM3`, `/dev/ttyACM0` or `/dev/cu.usbmodem1`.
    pub fn abrir(caminho: &str) -> io::Result<Self> {
        let porta = serialport::new(caminho, VELOCIDADE)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(ESPERA_LEITURA)
            .open()
            .map_err(io::Error::other)?;
        Ok(Self { porta, inicio: Instant::now() })
    }

    /// The names of the ports this system lists.
    pub fn listar() -> io::Result<Vec<String>> {
        let portas = serialport::available_ports().map_err(io::Error::other)?;
        Ok(portas.into_iter().map(|porta| porta.port_name).collect())
    }
}

impl PortaPinpad for PortaSerial {
    fn escrever(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.porta.write_all(bytes)?;
        self.porta.flush()
    }

    fn ler(&mut self, destino: &mut [u8]) -> io::Result<usize> {
        match self.porta.read(destino) {
            Ok(lidos) => Ok(lidos),
            Err(erro) if erro.kind() == io::ErrorKind::TimedOut => Ok(0),
            Err(erro) => Err(erro),
        }
    }

    fn agora_ms(&self) -> u64 {
        u64::try_from(self.inicio.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porta_inexistente_vira_erro_e_nao_panico() {
        assert!(PortaSerial::abrir("/dev/ravenrs-pagamento-nao-existe").is_err());
        assert!(PortaSerial::listar().is_ok());
    }
}
