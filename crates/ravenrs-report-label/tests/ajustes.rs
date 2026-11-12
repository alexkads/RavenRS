//! The adjustments of a driver reaching the bytes that go to the printer.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use ravenrs_core::Rect;
use ravenrs_perifericos::{Ajustes, Comando, Registro, Version};
use ravenrs_report::{Document, Item, Page, PrinterAction, PrinterCommand};
use ravenrs_report_label::{Bitmap, LabelJob, Language, compile, encode_bitmap};

const ROWS: f32 = 400.0;

fn pontos(dots: f32) -> f32 {
    dots * 72.0 / 203.0
}

fn receipt(actions: &[PrinterAction]) -> Document {
    let mut items = vec![Item::Rect {
        rect: Rect::new(0.0, 0.0, pontos(576.0), pontos(ROWS)),
        fill: Some(ravenrs_core::Color::BLACK),
        stroke: None,
        radius: 0.0,
    }];
    for (index, action) in actions.iter().enumerate() {
        items.push(Item::PrinterCommand {
            y: pontos(100.0 + index as f32),
            command: PrinterCommand { action: *action, lines: 2 },
        });
    }
    let page = Page { width: pontos(576.0), height: pontos(ROWS), items, ..Page::default() };
    Document { pages: vec![page], ..Document::default() }
}

fn contains(bytes: &[u8], part: &[u8]) -> bool {
    bytes.windows(part.len()).any(|window| window == part)
}

fn bematech() -> LabelJob {
    let registro = Registro::novo(Version::new(27, 1, 13));
    LabelJob::do_driver(registro.ativo("bematech.mp-4200-th.cupom").unwrap()).unwrap()
}

#[test]
fn the_mp4200_driver_changes_the_beep_the_cut_and_the_mode() {
    let job = bematech();
    assert_eq!((job.language, job.dpi), (Language::EscPos, 203));
    let document = receipt(&[PrinterAction::Beep, PrinterAction::Cut]);
    let bytes = compile(&document, &job).unwrap();
    assert!(bytes.starts_with(&[0x1D, 0xF9, 0x20, 0x01, 0x1B, b'@', 0x1B, b'a', 1]), "the mode comes first");
    assert!(contains(&bytes, &[0x1B, b'(', b'A', 5, 0, 0x61, 0x64, 1, 2, 1]), "its own beep");
    assert!(!contains(&bytes, &[0x1B, b'(', b'A', 3, 0, 97]), "not Epson's");
    assert!(contains(&bytes, &[0x1D, b'V', 66, 24]), "its only cut with feed");
    assert!(!contains(&bytes, &[0x1D, b'V', 65, 24]), "a full cut it does not list");
}

#[test]
fn without_adjustments_the_usual_commands_remain() {
    let job = LabelJob::new(Language::EscPos, 203);
    let bytes = compile(&receipt(&[PrinterAction::Beep, PrinterAction::Cut]), &job).unwrap();
    assert!(bytes.starts_with(&[0x1B, b'@']));
    assert!(contains(&bytes, &[0x1B, b'(', b'A', 3, 0, 97, 1, 1]));
    assert!(contains(&bytes, &[0x1D, b'V', 65, 24]));
}

#[test]
fn each_adjustment_goes_where_it_belongs() {
    let comando = |bytes: &[u8]| Comando::novo(bytes);
    let ajustes = Ajustes {
        corte_total: comando(&[0x1D, b'V', 48]),
        corte_parcial: comando(&[0x1D, b'V', 49]),
        gaveta: comando(&[0x1B, b'v', 100]),
        ..Ajustes::default()
    };
    let job = LabelJob { ajustes, ..LabelJob::new(Language::EscPos, 203) };
    let bytes = compile(&receipt(&[PrinterAction::Cut, PrinterAction::OpenDrawer]), &job).unwrap();
    assert!(contains(&bytes, &[0x1D, b'V', 48]));
    assert!(contains(&bytes, &[0x1B, b'v', 100]));
    assert!(bytes.ends_with(&[0x1D, b'V', 49]), "the receipt's last cut is the partial one");
}

#[test]
fn the_block_size_comes_from_the_driver() {
    let ajustes = Ajustes { tamanho_bloco: Some(50), ..Ajustes::default() };
    let job = LabelJob { ajustes, ..LabelJob::new(Language::EscPos, 203) };
    let bitmap = Bitmap::new(8, 120);
    let bytes = encode_bitmap(&bitmap, &job).unwrap();
    // Header of 8 bytes and 50 rows of 1 byte, twice, then the rest.
    assert_eq!(bytes[5..13], [0x1D, b'v', b'0', 0, 1, 0, 50, 0]);
    assert_eq!(bytes[63..71], [0x1D, b'v', b'0', 0, 1, 0, 50, 0]);
    assert_eq!(bytes[121..129], [0x1D, b'v', b'0', 0, 1, 0, 20, 0]);
}

#[test]
fn inverted_polarity_flips_the_raster_in_every_language() {
    let mut bitmap = Bitmap::new(8, 1);
    bitmap.fill(0, 0, 4, 1, true);
    let invertida = Ajustes { polaridade_invertida: true, ..Ajustes::default() };
    let encode = |language, ajustes| {
        let job = LabelJob { ajustes, ..LabelJob::new(language, 203) };
        encode_bitmap(&bitmap, &job).unwrap()
    };
    // Black is 1 in ZPL, PPLA and ESC/POS, and 0 in EPL and TSPL.
    let zpl = encode(Language::Zpl, invertida);
    assert!(contains(&zpl, b",0F^FS"), "{}", String::from_utf8_lossy(&zpl));
    assert!(contains(&encode(Language::Ppla, invertida), b"80010F\r"));
    assert!(contains(&encode(Language::EscPos, invertida), &[0x1D, b'v', b'0', 0, 1, 0, 1, 0, 0x0F]));
    assert!(contains(&encode(Language::Epl, invertida), &[b',', 0xF0, b'\n']));
    assert!(contains(&encode(Language::Tspl, invertida), &[b',', 0xF0, b'\r']));
    // And without it, the language's own polarity.
    assert!(contains(&encode(Language::Epl, Ajustes::default()), &[b',', 0x0F, b'\n']));
}

#[test]
fn a_driver_of_what_does_not_print_is_not_a_job() {
    let registro = Registro::novo(Version::new(27, 1, 13));
    let pinpad = registro.ativo("gertec.ppc930.pinpad").unwrap();
    assert!(LabelJob::do_driver(pinpad).is_err());
    let leitor = registro.ativo("generico.hid-teclado.leitor").unwrap();
    assert!(LabelJob::do_driver(leitor).is_err());
}

#[test]
fn every_printer_driver_makes_a_job() {
    let registro = Registro::novo(Version::new(27, 1, 13));
    let mut feitos = 0;
    for driver in registro.ativos() {
        if let Ok(job) = LabelJob::do_driver(driver) {
            compile(&receipt(&[]), &job).unwrap_or_else(|erro| panic!("{}: {erro}", driver.id));
            feitos += 1;
        }
    }
    assert_eq!(feitos, 9);
}
