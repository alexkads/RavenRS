//! The link layer: packets, byte substitution and CRC. It reads and writes no device.

use std::fmt;

/// End of transmission: the pinpad's answer to [`CAN`].
pub const EOT: u8 = 0x04;
/// The pinpad got a valid packet.
pub const ACK: u8 = 0x06;
/// Substitution byte.
pub const DC3: u8 = 0x13;
/// The packet was invalid: send it again.
pub const NAK: u8 = 0x15;
/// Start of a packet.
pub const SYN: u8 = 0x16;
/// End of a packet.
pub const ETB: u8 = 0x17;
/// Cancels the command in progress.
pub const CAN: u8 = 0x18;
/// The longest packet content, before substitution.
pub const TAMANHO_MAXIMO_PACOTE: usize = 2049;

/// CRC-16-CCITT, generator polynomial 1021h, starting at zero, most significant bit first.
#[must_use]
pub fn crc16(dados: &[u8]) -> u16 {
    dados.iter().fold(0, |crc, byte| crc_mais(crc, *byte))
}

fn crc_mais(mut crc: u16, byte: u8) -> u16 {
    let mut valor = u16::from(byte) << 8;
    for _ in 0..8 {
        crc = if (crc ^ valor) & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        valor <<= 1;
    }
    crc
}

fn crc_do_pacote(dados: &[u8]) -> u16 {
    crc_mais(crc16(dados), ETB)
}

/// Why a packet could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PacoteGrande(pub usize);

impl fmt::Display for PacoteGrande {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pacote de {} bytes passa do limite de {TAMANHO_MAXIMO_PACOTE}", self.0)
    }
}

impl std::error::Error for PacoteGrande {}

/// Builds the bytes of a packet. The CRC covers the original data and the `ETB`, and travels as is.
pub fn codificar_pacote(dados: &[u8]) -> Result<Vec<u8>, PacoteGrande> {
    if dados.len() > TAMANHO_MAXIMO_PACOTE {
        return Err(PacoteGrande(dados.len()));
    }
    let mut saida = Vec::with_capacity(dados.len() + 4);
    saida.push(SYN);
    for byte in dados {
        match *byte {
            DC3 => saida.extend_from_slice(&[DC3, 0x33]),
            SYN => saida.extend_from_slice(&[DC3, 0x36]),
            ETB => saida.extend_from_slice(&[DC3, 0x37]),
            outro => saida.push(outro),
        }
    }
    saida.push(ETB);
    saida.extend_from_slice(&crc_do_pacote(dados).to_be_bytes());
    Ok(saida)
}

/// What came out of the byte stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventoQuadro {
    /// A lone `ACK`.
    Ack,
    /// A lone `NAK`.
    Nak,
    /// A lone `EOT`.
    Eot,
    /// A lone `CAN`.
    Can,
    /// A whole packet with the right CRC.
    Pacote(Vec<u8>),
    /// A packet with a bad CRC, a bad substitution or too long.
    PacoteInvalido,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fase {
    Fora,
    Dentro,
    Substituicao,
    Crc1,
    Crc2(u8),
}

/// Reads the stream one byte at a time and keeps its place between calls.
#[derive(Clone, Debug)]
pub struct Decodificador {
    fase: Fase,
    dados: Vec<u8>,
    invalido: bool,
}

impl Default for Decodificador {
    fn default() -> Self {
        Self { fase: Fase::Fora, dados: Vec::new(), invalido: false }
    }
}

impl Decodificador {
    /// Reads one byte. Bytes that mean nothing outside a packet are dropped.
    pub fn alimentar(&mut self, byte: u8) -> Option<EventoQuadro> {
        match self.fase {
            Fase::Fora => match byte {
                ACK => Some(EventoQuadro::Ack),
                NAK => Some(EventoQuadro::Nak),
                EOT => Some(EventoQuadro::Eot),
                CAN => Some(EventoQuadro::Can),
                SYN => {
                    self.dados.clear();
                    self.invalido = false;
                    self.fase = Fase::Dentro;
                    None
                }
                _ => None,
            },
            Fase::Dentro => {
                match byte {
                    ETB => self.fase = Fase::Crc1,
                    DC3 => self.fase = Fase::Substituicao,
                    SYN => {
                        // A start inside a packet means the end of the last one was lost.
                        self.dados.clear();
                        self.invalido = false;
                        return Some(EventoQuadro::PacoteInvalido);
                    }
                    outro => self.guardar(outro),
                }
                None
            }
            Fase::Substituicao => {
                match byte {
                    0x33 => self.guardar(DC3),
                    0x36 => self.guardar(SYN),
                    0x37 => self.guardar(ETB),
                    _ => self.invalido = true,
                }
                self.fase = Fase::Dentro;
                None
            }
            Fase::Crc1 => {
                self.fase = Fase::Crc2(byte);
                None
            }
            Fase::Crc2(primeiro) => {
                self.fase = Fase::Fora;
                let recebido = u16::from_be_bytes([primeiro, byte]);
                let dados = std::mem::take(&mut self.dados);
                if self.invalido || crc_do_pacote(&dados) != recebido {
                    Some(EventoQuadro::PacoteInvalido)
                } else {
                    Some(EventoQuadro::Pacote(dados))
                }
            }
        }
    }

    /// Forgets a packet in the middle.
    pub fn reiniciar(&mut self) {
        *self = Self::default();
    }

    fn guardar(&mut self, byte: u8) {
        if self.dados.len() < TAMANHO_MAXIMO_PACOTE {
            self.dados.push(byte);
        } else {
            self.invalido = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decodificar(bytes: &[u8]) -> Vec<EventoQuadro> {
        let mut decodificador = Decodificador::default();
        bytes.iter().filter_map(|b| decodificador.alimentar(*b)).collect()
    }

    #[test]
    fn vetor_oficial_do_opn() {
        assert_eq!(codificar_pacote(b"OPN").expect("cabe"), [0x16, 0x4F, 0x50, 0x4E, 0x17, 0xA8, 0xA9]);
    }

    #[test]
    fn vetor_oficial_do_dsp() {
        // The bytes of the specification's example.
        let mensagem = b"DSP032    OPERA\xC7\xC3O       FINALIZADA   ";
        let pacote = codificar_pacote(mensagem).expect("cabe");
        assert_eq!(pacote[pacote.len() - 3..], [0x17, 0x52, 0x13]);
    }

    #[test]
    fn substituicao_cobre_os_tres_bytes_e_o_crc_viaja_cru() {
        let dados = [0x41, DC3, SYN, ETB, 0x42];
        let pacote = codificar_pacote(&dados).expect("cabe");
        assert_eq!(pacote[..9], [SYN, 0x41, DC3, 0x33, DC3, 0x36, DC3, 0x37, 0x42]);
        assert_eq!(pacote[9], ETB);
        assert_eq!(decodificar(&pacote), vec![EventoQuadro::Pacote(dados.to_vec())]);
    }

    #[test]
    fn crc_com_byte_de_controle_nao_atrapalha() {
        // Whatever the CRC holds, the reader takes two bytes after ETB without looking at them.
        for primeiro in 0..=255u8 {
            let dados = [primeiro, 0x10, 0x20];
            let pacote = codificar_pacote(&dados).expect("cabe");
            assert_eq!(decodificar(&pacote), vec![EventoQuadro::Pacote(dados.to_vec())]);
        }
    }

    #[test]
    fn crc_errado_e_substituicao_errada_sao_invalidos() {
        assert_eq!(decodificar(&[SYN, 0x4F, 0x50, 0x4E, ETB, 0, 0]), vec![EventoQuadro::PacoteInvalido]);
        let mut errado = vec![SYN, DC3, 0x99];
        errado.push(ETB);
        errado.extend_from_slice(&crc_do_pacote(&[]).to_be_bytes());
        assert_eq!(decodificar(&errado), vec![EventoQuadro::PacoteInvalido]);
    }

    #[test]
    fn controles_fora_do_pacote() {
        assert_eq!(
            decodificar(&[ACK, 0x55, NAK, EOT, CAN]),
            vec![EventoQuadro::Ack, EventoQuadro::Nak, EventoQuadro::Eot, EventoQuadro::Can]
        );
    }

    #[test]
    fn pacote_grande_demais_e_recusado_dos_dois_lados() {
        assert!(codificar_pacote(&vec![0x41; TAMANHO_MAXIMO_PACOTE]).is_ok());
        assert_eq!(codificar_pacote(&vec![0x41; 2050]), Err(PacoteGrande(2050)));
        let mut enorme = vec![SYN];
        enorme.extend(vec![0x41; 2050]);
        enorme.push(ETB);
        enorme.extend_from_slice(&[0, 0]);
        assert_eq!(decodificar(&enorme), vec![EventoQuadro::PacoteInvalido]);
    }

    #[test]
    fn syn_no_meio_recomeca() {
        let bom = codificar_pacote(b"OPN").expect("cabe");
        let mut bytes = vec![SYN, 0x41];
        bytes.extend_from_slice(&bom);
        assert_eq!(
            decodificar(&bytes),
            vec![EventoQuadro::PacoteInvalido, EventoQuadro::Pacote(b"OPN".to_vec())]
        );
    }
}
