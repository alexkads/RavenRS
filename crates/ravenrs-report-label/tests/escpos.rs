//! Receipts in ESC/POS, read back by a parser written apart from the encoder. The codes are then read
//! by `rxing` from what the parser got, which is what the printer would put on paper.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use ravenrs_core::{Color, Rect};
use ravenrs_report::barcode::{Symbology, encode, shape};
use ravenrs_report::labels::{LabelStock, ShelfLabelFields, shelf_label};
use ravenrs_report::{
    Band, Control, Document, Item, Page, PlacedLine, PrinterAction, PrinterCommand, ReportEngine, TextAlign,
    TextBlock,
};
use ravenrs_report_label::{Bitmap, LabelJob, Language, compile, encode_bitmap, rasterize_page};
use ravenrs_text::TextSystem;
use rxing::BarcodeFormat;
use rxing::helpers::detect_in_luma;
use serde_json::json;

const DPI: u32 = 203;
/// 80 mm paper prints 576 dots.
const WIDTH_POINTS: f32 = 576.0 * 72.0 / 203.0;

struct Receipt {
    bitmap: Bitmap,
    /// The rows of each `GS v 0`.
    blocks: Vec<usize>,
}

fn word(bytes: &[u8]) -> usize {
    usize::from(bytes[0]) + usize::from(bytes[1]) * 256
}

fn parse(bytes: &[u8]) -> Vec<Receipt> {
    let mut receipts = Vec::new();
    let mut at = bytes;
    while !at.is_empty() {
        assert_eq!(at[..5], [0x1B, b'@', 0x1B, b'a', 1], "initialized and centered");
        at = &at[5..];
        let (mut rows, mut blocks, mut per_row) = (Vec::new(), Vec::new(), 0);
        while at.starts_with(&[0x1D, b'v', b'0', 0]) {
            per_row = word(&at[4..6]);
            let height = word(&at[6..8]);
            assert!((1..=255).contains(&per_row), "{per_row}");
            assert!((1..=4607).contains(&height), "Epson's limit: {height}");
            let (data, rest) = at[8..].split_at(per_row * height);
            rows.extend_from_slice(data);
            blocks.push(height);
            at = rest;
        }
        assert_eq!(at[..3], [0x1D, b'V', 66], "cut with feed");
        at = &at[4..];
        let height: usize = blocks.iter().sum();
        let mut bitmap = Bitmap::new(per_row as u32 * 8, height as u32);
        for (y, row) in rows.chunks(per_row).enumerate() {
            for x in 0..per_row * 8 {
                bitmap.set(x as u32, y as u32, row[x / 8] & (0x80 >> (x % 8)) != 0);
            }
        }
        receipts.push(Receipt { bitmap, blocks });
    }
    receipts
}

/// The rows from `top` as gray pixels, for the decoder.
fn luma(bitmap: &Bitmap, top: u32, rows: u32) -> Vec<u8> {
    let mut pixels = Vec::new();
    for y in top..top + rows {
        for x in 0..bitmap.width() {
            pixels.push(if bitmap.get(x, y) { 0 } else { 255 });
        }
    }
    pixels
}

fn read(bitmap: &Bitmap, top: u32, rows: u32, format: BarcodeFormat) -> String {
    let result = detect_in_luma(luma(bitmap, top, rows), bitmap.width(), rows, Some(format));
    result.unwrap_or_else(|e| panic!("{format:?}: {e}")).getText().to_owned()
}

fn roll_report(symbology: Symbology, content: &str) -> Document {
    // 576 dots are 72.07 mm.
    let stock = LabelStock::roll("Bobina 80 mm", 576.0 * 25.4 / 203.0, 40.0);
    let fields = ShelfLabelFields {
        description: "[Nome]".to_owned(),
        price: "[Preco]".to_owned(),
        price_format: "C2".to_owned(),
        barcode: "[Codigo]".to_owned(),
        symbology,
    };
    let data = json!([
        { "Nome": "Açúcar União", "Preco": 12.9, "Codigo": content },
        { "Nome": "Pão de Forma", "Preco": 8.5, "Codigo": content },
    ]);
    ReportEngine::new().render(&shelf_label(&stock, "Cupom", &fields), &data)
}

#[test]
fn each_page_is_a_receipt_that_reads_back_as_the_picture() {
    let document = roll_report(Symbology::Code128, "ABC-1234");
    assert_eq!(document.pages.len(), 2);
    let job = LabelJob { copies: 2, ..LabelJob::new(Language::EscPos, DPI) };
    let bytes = compile(&document, &job).unwrap();
    let receipts = parse(&bytes);
    assert_eq!(receipts.len(), 4, "2 pages, 2 copies");
    let mut text = TextSystem::new();
    let first = rasterize_page(&document.pages[0], DPI, &mut text).unwrap();
    let second = rasterize_page(&document.pages[1], DPI, &mut text).unwrap();
    assert_eq!(first.width(), 576);
    assert_ne!(first, second);
    // The copies of a receipt come together, as with the copies of a label.
    for (receipt, original) in receipts.iter().zip([&first, &first, &second, &second]) {
        assert_eq!(receipt.bitmap, *original);
    }
}

#[test]
fn the_codes_read_from_what_was_sent() {
    for (symbology, format, content, expected) in [
        (Symbology::Ean13, BarcodeFormat::EAN_13, "789123456789", "7891234567895"),
        (Symbology::Qr, BarcodeFormat::QR_CODE, "NFC-E 3525", "NFC-E 3525"),
    ] {
        let document = roll_report(symbology, content);
        let bytes = compile(&document, &LabelJob::new(Language::EscPos, DPI)).unwrap();
        let receipt = &parse(&bytes)[0];
        let height = receipt.bitmap.height();
        assert_eq!(read(&receipt.bitmap, 0, height, format), expected);
    }
}

fn text_line(y: f32, line: String) -> Item {
    Item::Text(TextBlock {
        clip: Rect::new(0.0, y, WIDTH_POINTS, 14.0),
        lines: vec![PlacedLine { text: line, x: 4.0, top: y, baseline: y + 10.0, width: 150.0 }],
        size: 9.0,
        line_height: 12.0,
        bold: false,
        color: Color::BLACK,
        align: TextAlign::Left,
        value: None,
        format: None,
    })
}

/// A receipt of hundreds of items, with an EAN-13 at the start and a QR Code at the end.
fn long_receipt() -> Page {
    let mut items = Vec::new();
    let code = |rect: Rect, symbology, content: &str, items: &mut Vec<Item>| {
        items.push(Item::BarcodeMark { rect, symbology, content: content.to_owned() });
        let pattern = encode(symbology, content).unwrap();
        items.push(Item::Vector { rect, shapes: vec![shape(&pattern, rect, Color::BLACK)] });
    };
    code(Rect::new(0.0, 5.0, WIDTH_POINTS, 40.0), Symbology::Ean13, "789123456789", &mut items);
    let mut y = 50.0;
    for index in 0..400 {
        items.push(text_line(y, format!("{index:03} Café com açúcar 2 x R$ 3,50")));
        y += 12.0;
    }
    code(Rect::new(40.0, y, 120.0, 120.0), Symbology::Qr, "https://nfce.exemplo/3525", &mut items);
    Page { width: WIDTH_POINTS, height: y + 125.0, items, ..Page::default() }
}

#[test]
fn a_long_receipt_is_split_in_blocks_and_reads_back_whole() {
    let page = long_receipt();
    let started = std::time::Instant::now();
    let bitmap = rasterize_page(&page, DPI, &mut TextSystem::new()).unwrap();
    println!("{} pontos de altura em {:?}", bitmap.height(), started.elapsed());
    assert!(bitmap.height() > 13_000, "{}", bitmap.height());

    let document = Document { pages: vec![page], ..Document::default() };
    let bytes = compile(&document, &LabelJob::new(Language::EscPos, DPI)).unwrap();
    let receipts = parse(&bytes);
    assert_eq!(receipts.len(), 1);
    let receipt = &receipts[0];
    assert_eq!(receipt.bitmap, bitmap);
    assert!(receipt.blocks.len() > 10, "{:?}", receipt.blocks);
    let (last, full) = receipt.blocks.split_last().unwrap();
    assert!(full.iter().all(|rows| *rows == 1024), "{:?}", receipt.blocks);
    assert!((1..=1024).contains(last));
    assert_eq!(bytes[bytes.len() - 4..bytes.len() - 1], [0x1D, b'V', 66], "the cut comes last");

    // The ends of the receipt, on either side of many blocks.
    assert_eq!(read(&bitmap, 0, 200, BarcodeFormat::EAN_13), "7891234567895");
    let height = bitmap.height();
    assert_eq!(read(&bitmap, height - 500, 500, BarcodeFormat::QR_CODE), "https://nfce.exemplo/3525");
}

/// What the bytes do, in order.
#[derive(Debug, PartialEq, Eq)]
enum Event {
    /// Rows of one or more blocks, joined.
    Rows(usize),
    Command(Vec<u8>),
}

fn events(bytes: &[u8]) -> Vec<Event> {
    assert_eq!(bytes[..5], [0x1B, b'@', 0x1B, b'a', 1]);
    let mut at = &bytes[5..];
    let mut found = Vec::new();
    while !at.is_empty() {
        let length = match at {
            [0x1D, b'v', b'0', 0, ..] => {
                let (per_row, rows) = (word(&at[4..6]), word(&at[6..8]));
                at = &at[8 + per_row * rows..];
                match found.last_mut() {
                    Some(Event::Rows(joined)) => *joined += rows,
                    _ => found.push(Event::Rows(rows)),
                }
                continue;
            }
            [0x1D, b'V', 65 | 66, _, ..] => 4,
            [0x1B, b'p', ..] => 5,
            [0x1B, b'd', _, ..] => 3,
            [0x1B, b'(', b'A', low, high, ..] => 5 + word(&[*low, *high]),
            other => panic!("unknown bytes: {:?}", &other[..other.len().min(8)]),
        };
        found.push(Event::Command(at[..length].to_vec()));
        at = &at[length..];
    }
    found
}

const END_CUT: [u8; 4] = [0x1D, b'V', 66, 24];
const ROWS: usize = 400;

/// A page of 400 rows with ink at the top and the bottom, and these commands at row 100.
fn page_with(commands: Vec<Item>) -> Document {
    let height = ROWS as f32 * 72.0 / 203.0;
    let ink = |y: f32| Item::Rect {
        rect: Rect::new(0.0, y, WIDTH_POINTS, 2.0),
        fill: Some(Color::BLACK),
        stroke: None,
        radius: 0.0,
    };
    let mut items = vec![ink(0.0), ink(height - 3.0)];
    items.extend(commands);
    Document {
        pages: vec![Page { width: WIDTH_POINTS, height, items, ..Page::default() }],
        ..Document::default()
    }
}

fn command_at(row: usize, action: PrinterAction, lines: u8) -> Item {
    Item::PrinterCommand { y: row as f32 * 72.0 / 203.0, command: PrinterCommand { action, lines } }
}

fn compiled_events(document: &Document) -> Vec<Event> {
    events(&compile(document, &LabelJob::new(Language::EscPos, DPI)).unwrap())
}

#[test]
fn each_command_comes_between_its_rows() {
    for (action, expected) in [
        (PrinterAction::Cut, vec![0x1D, b'V', 65, 24]),
        (PrinterAction::PartialCut, vec![0x1D, b'V', 66, 24]),
        (PrinterAction::OpenDrawer, vec![0x1B, b'p', 0, 25, 250]),
        (PrinterAction::Beep, vec![0x1B, b'(', b'A', 3, 0, 97, 1, 1]),
        (PrinterAction::Feed, vec![0x1B, b'd', 5]),
    ] {
        let document = page_with(vec![command_at(100, action, 5)]);
        assert_eq!(
            compiled_events(&document),
            [
                Event::Rows(100),
                Event::Command(expected),
                Event::Rows(ROWS - 100),
                Event::Command(END_CUT.to_vec()),
            ],
            "{action:?}"
        );
    }
}

#[test]
fn without_a_cut_on_the_layout_the_receipt_is_cut_at_the_end() {
    let document = page_with(Vec::new());
    assert_eq!(compiled_events(&document), [Event::Rows(ROWS), Event::Command(END_CUT.to_vec())]);
}

#[test]
fn a_cut_at_the_end_of_the_layout_is_the_only_one() {
    for row in [ROWS, ROWS + 50] {
        let document = page_with(vec![command_at(row, PrinterAction::Cut, 0)]);
        assert_eq!(
            compiled_events(&document),
            [Event::Rows(ROWS), Event::Command(vec![0x1D, b'V', 65, 24])],
            "at row {row}"
        );
    }
}

#[test]
fn blank_paper_after_the_last_cut_does_not_go_and_later_commands_still_do() {
    let mut document = page_with(Vec::new());
    document.pages[0].items.truncate(1);
    document.pages[0].items.push(command_at(50, PrinterAction::PartialCut, 0));
    document.pages[0].items.push(command_at(60, PrinterAction::OpenDrawer, 0));
    assert_eq!(
        compiled_events(&document),
        [
            Event::Rows(50),
            Event::Command(vec![0x1D, b'V', 66, 24]),
            Event::Rows(10),
            Event::Command(vec![0x1B, b'p', 0, 25, 250]),
        ]
    );
}

#[test]
fn a_cut_in_the_middle_with_print_after_it_still_ends_with_a_cut() {
    let document =
        page_with(vec![command_at(300, PrinterAction::Beep, 0), command_at(100, PrinterAction::Cut, 0)]);
    assert_eq!(
        compiled_events(&document),
        [
            Event::Rows(100),
            Event::Command(vec![0x1D, b'V', 65, 24]),
            Event::Rows(200),
            Event::Command(vec![0x1B, b'(', b'A', 3, 0, 97, 1, 1]),
            Event::Rows(100),
            Event::Command(END_CUT.to_vec()),
        ],
        "in the order they come down the paper"
    );
}

#[test]
fn commands_split_the_rows_and_the_picture_stays_whole() {
    let document = page_with(vec![command_at(100, PrinterAction::Feed, 2)]);
    let plain = page_with(Vec::new());
    let job = LabelJob::new(Language::EscPos, DPI);
    let mut text = TextSystem::new();
    let original = rasterize_page(&plain.pages[0], DPI, &mut text).unwrap();
    let bytes = compile(&document, &job).unwrap();
    // Without the command, its bytes and the block header around it, the rows are the same.
    let per_row = original.bytes_per_row();
    let first_rows = 13 + 100 * per_row;
    let tail = &bytes[first_rows + 3 + 8..bytes.len() - 4];
    assert_eq!(&bytes[13..first_rows], &original.bits()[..100 * per_row]);
    assert_eq!(tail, &original.bits()[100 * per_row..]);
}

#[test]
fn the_other_languages_ignore_the_commands() {
    let with =
        page_with(vec![command_at(100, PrinterAction::Cut, 0), command_at(120, PrinterAction::Beep, 0)]);
    let without = page_with(Vec::new());
    for language in [Language::Zpl, Language::Epl, Language::Ppla, Language::Tspl] {
        let job = LabelJob::new(language, DPI);
        assert_eq!(compile(&with, &job).unwrap(), compile(&without, &job).unwrap(), "{language:?}");
    }
}

#[test]
fn a_command_control_on_a_report_reaches_the_bytes() {
    let stock = LabelStock::roll("Bobina 80 mm", 576.0 * 25.4 / 203.0, 40.0);
    let mut report = stock.report("Cupom");
    report.detail = Band::new(stock.height)
        .control(Control::label("Total").at(0.0, 0.0, 100.0, 20.0))
        .control(Control::printer_command(PrinterAction::OpenDrawer).at(0.0, 30.0, 100.0, 5.0));
    let document = ReportEngine::new().render(&report, &json!([{}]));
    let found = compiled_events(&document);
    let row = (30.0_f32 * 203.0 / 72.0).round() as usize;
    assert_eq!(found[0], Event::Rows(row));
    assert_eq!(found[1], Event::Command(vec![0x1B, b'p', 0, 25, 250]));
    assert_eq!(found.last(), Some(&Event::Command(END_CUT.to_vec())));
}

#[test]
fn a_picture_on_whole_bytes_is_not_shifted_and_any_other_is_centered() {
    let mut exact = Bitmap::new(24, 3);
    exact.fill(0, 1, 1, 2, true);
    let job = LabelJob::new(Language::EscPos, DPI);
    assert_eq!(parse(&encode_bitmap(&exact, &job).unwrap())[0].bitmap, exact);

    let mut odd = Bitmap::new(575, 2);
    odd.fill(0, 0, 575, 1, true);
    let back = &parse(&encode_bitmap(&odd, &job).unwrap())[0].bitmap;
    assert_eq!(back.width(), 576, "72 mm round to 575 dots and still print");
    assert!(back.get(0, 0) && back.get(574, 0) && !back.get(575, 0));
}
