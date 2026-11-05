//! Compiles real labels to each language and reads the bytes back with a parser written apart from
//! the encoder: what the printer would put on paper must be the picture we started from.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use ravenrs_report::labels::{LabelStock, ShelfLabelFields, shelf_label};
use ravenrs_report::{Document, ReportEngine, barcode::Symbology};
use ravenrs_report_label::{Bitmap, LabelJob, Language, compile, encode_bitmap, rasterize_page};
use ravenrs_text::TextSystem;
use serde_json::json;

const LANGUAGES: [Language; 4] = [Language::Zpl, Language::Epl, Language::Ppla, Language::Tspl];

fn take_until<'a>(bytes: &mut &'a [u8], end: &[u8]) -> &'a [u8] {
    let at = bytes
        .windows(end.len())
        .position(|w| w == end)
        .unwrap_or_else(|| panic!("{:?} not found in {:?}", String::from_utf8_lossy(end), head(bytes)));
    let (before, rest) = bytes.split_at(at);
    *bytes = &rest[end.len()..];
    before
}

fn head(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(60)]).into_owned()
}

fn expect(bytes: &mut &[u8], start: &[u8]) {
    assert!(bytes.starts_with(start), "expected {:?} at {:?}", String::from_utf8_lossy(start), head(bytes));
    *bytes = &bytes[start.len()..];
}

fn number(text: &[u8]) -> usize {
    std::str::from_utf8(text).unwrap().parse().unwrap()
}

fn from_hex(text: &[u8]) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.chunks(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect()
}

/// Paints raster rows onto a picture, `black_bit` being the value that burns a dot.
fn paint(bitmap: &mut Bitmap, top: u32, per_row: usize, data: &[u8], black_bit: bool) {
    for (row, line) in data.chunks(per_row).enumerate() {
        for x in 0..bitmap.width() {
            let bit = line[x as usize / 8] & (0x80 >> (x % 8)) != 0;
            bitmap.set(x, top + row as u32, bit == black_bit);
        }
    }
}

struct Label {
    bitmap: Bitmap,
    copies: usize,
}

fn parse_zpl(bytes: &mut &[u8]) -> Label {
    expect(bytes, b"^XA^PW");
    let width = number(take_until(bytes, b"^LL"));
    let height = number(take_until(bytes, b"^LH0,0"));
    let mut bitmap = Bitmap::new(width as u32, height as u32);
    while bytes.starts_with(b"^FO0,") {
        expect(bytes, b"^FO0,");
        let top = number(take_until(bytes, b"^GFA,"));
        let binary = number(take_until(bytes, b","));
        let total = number(take_until(bytes, b","));
        let per_row = number(take_until(bytes, b","));
        assert_eq!(binary, total, "in ASCII the byte count matches the field count");
        assert!(total <= 99_999, "the manual's limit");
        let data = from_hex(take_until(bytes, b"^FS"));
        assert_eq!(data.len(), total);
        paint(&mut bitmap, top as u32, per_row, &data, true);
    }
    expect(bytes, b"^PQ");
    let copies = number(take_until(bytes, b"^XZ\n"));
    Label { bitmap, copies }
}

fn parse_epl(bytes: &mut &[u8]) -> Label {
    expect(bytes, b"\nq");
    let width = number(take_until(bytes, b"\n"));
    expect(bytes, b"R0,0\nN\nGW0,0,");
    let per_row = number(take_until(bytes, b","));
    let height = number(take_until(bytes, b","));
    let (data, rest) = bytes.split_at(per_row * height);
    *bytes = rest;
    let mut bitmap = Bitmap::new(width as u32, height as u32);
    paint(&mut bitmap, 0, per_row, data, false);
    expect(bytes, b"\nP");
    Label { bitmap, copies: number(take_until(bytes, b"\n")) }
}

fn parse_tspl(bytes: &mut &[u8], width: u32, dpi: u32) -> Label {
    expect(bytes, b"SIZE ");
    let size = String::from_utf8(take_until(bytes, b"\r\n").to_vec()).unwrap();
    let declared: f32 = size.split(" mm").next().unwrap().parse().unwrap();
    let expected = width as f32 * 25.4 / dpi as f32;
    assert!((declared - expected).abs() < 0.06, "{size} for {width} dots");
    expect(bytes, b"REFERENCE 0,0\r\nCLS\r\nBITMAP 0,0,");
    let per_row = number(take_until(bytes, b","));
    let height = number(take_until(bytes, b","));
    expect(bytes, b"0,");
    let (data, rest) = bytes.split_at(per_row * height);
    *bytes = rest;
    let mut bitmap = Bitmap::new(width, height as u32);
    paint(&mut bitmap, 0, per_row, data, false);
    expect(bytes, b"\r\nPRINT ");
    let copies = number(take_until(bytes, b",1\r\n"));
    Label { bitmap, copies }
}

fn parse_ppla(bytes: &mut &[u8], width: u32) -> Label {
    expect(bytes, b"\x02IAF");
    let name = take_until(bytes, b"\r").to_vec();
    assert!(name.len() <= 16, "the manual's limit");
    let mut rows = Vec::new();
    loop {
        let record = take_until(bytes, b"\r");
        if record == b"FFFF" {
            break;
        }
        assert_eq!(&record[..2], b"80");
        let count = from_hex(&record[2..4])[0] as usize;
        let data = from_hex(&record[4..]);
        assert_eq!(data.len(), count);
        rows.push(data);
    }
    expect(bytes, b"\x02L\rD11\r1Y11000");
    let place = take_until(bytes, b"\r");
    assert_eq!(&place[..8], b"00000000", "row and column zero");
    assert_eq!(&place[8..], name);
    expect(bytes, b"Q");
    let copies = number(take_until(bytes, b"\r"));
    expect(bytes, b"E\r\x02xAG");
    assert_eq!(take_until(bytes, b"\r"), name, "the stored image is released");
    let mut bitmap = Bitmap::new(width, rows.len() as u32);
    // The first record is the bottom row.
    let top_down: Vec<u8> = rows.iter().rev().flatten().copied().collect();
    paint(&mut bitmap, 0, width.div_ceil(8) as usize, &top_down, true);
    Label { bitmap, copies }
}

fn parse_all(language: Language, mut bytes: &[u8], width: u32, dpi: u32) -> Vec<Label> {
    let mut labels = Vec::new();
    while !bytes.is_empty() {
        labels.push(match language {
            Language::Zpl => parse_zpl(&mut bytes),
            Language::Epl => parse_epl(&mut bytes),
            Language::Ppla => parse_ppla(&mut bytes, width),
            Language::Tspl => parse_tspl(&mut bytes, width, dpi),
            other => panic!("{other:?}"),
        });
    }
    labels
}

fn shelf_labels(products: serde_json::Value) -> Document {
    let stock = LabelStock::roll("Rolo 60 x 40 mm", 60.0, 40.0);
    let fields = ShelfLabelFields {
        description: "[Nome]".to_owned(),
        price: "[Preco]".to_owned(),
        price_format: "C2".to_owned(),
        barcode: "[Codigo]".to_owned(),
        symbology: Symbology::Code128,
    };
    let document = ReportEngine::new().render(&shelf_label(&stock, "Gôndola", &fields), &products);
    assert!(document.errors.is_empty(), "{:?}", document.errors);
    document
}

#[test]
fn every_language_at_every_resolution_gives_back_the_picture() {
    let document = shelf_labels(json!([
        { "Nome": "Açúcar União Orgânico", "Preco": 12.9, "Codigo": "78912345" },
        { "Nome": "Pão de Forma Integral", "Preco": 8.5, "Codigo": "ABC-987" },
    ]));
    assert_eq!(document.pages.len(), 2);
    for dpi in [203, 300] {
        let mut text = TextSystem::new();
        let originals: Vec<Bitmap> =
            document.pages.iter().map(|page| rasterize_page(page, dpi, &mut text).unwrap()).collect();
        assert!(originals.iter().all(|bitmap| bitmap.bits().iter().any(|byte| *byte != 0)));
        assert_ne!(originals[0], originals[1]);
        for language in LANGUAGES {
            let job = LabelJob { copies: 3, ..LabelJob::new(language, dpi) };
            let bytes = compile(&document, &job).unwrap();
            let labels = parse_all(language, &bytes, originals[0].width(), dpi);
            assert_eq!(labels.len(), 2, "{language:?} at {dpi}");
            for (label, original) in labels.iter().zip(&originals) {
                assert_eq!(label.bitmap, *original, "{language:?} at {dpi}");
                assert_eq!(label.copies, 3);
            }
        }
    }
}

#[test]
fn a_width_that_is_not_a_whole_number_of_bytes_keeps_its_edge() {
    let mut bitmap = Bitmap::new(13, 5);
    bitmap.fill(0, 0, 13, 1, true);
    bitmap.fill(12, 0, 13, 5, true);
    bitmap.fill(0, 4, 1, 5, true);
    for language in LANGUAGES {
        let bytes = encode_bitmap(&bitmap, &LabelJob::new(language, 203)).unwrap();
        let labels = parse_all(language, &bytes, 13, 203);
        assert_eq!(labels[0].bitmap, bitmap, "{language:?}");
    }
}

#[test]
fn a_tall_label_is_split_where_zpl_requires_and_reads_back_whole() {
    let mut bitmap = Bitmap::new(800, 1800);
    bitmap.fill(10, 0, 20, 1800, true);
    bitmap.fill(0, 1790, 800, 1800, true);
    let bytes = encode_bitmap(&bitmap, &LabelJob::new(Language::Zpl, 203)).unwrap();
    let fields = bytes.windows(4).filter(|w| *w == b"^GFA").count();
    assert_eq!(fields, 2, "100 bytes x 1800 rows is above 99999");
    assert_eq!(parse_all(Language::Zpl, &bytes, 800, 203)[0].bitmap, bitmap);
}
