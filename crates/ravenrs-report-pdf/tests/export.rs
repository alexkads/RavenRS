//! The file is valid PDF: an independent reader finds the pages, fonts and text.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::collections::HashMap;

use miniz_oxide::inflate::decompress_to_vec_zlib;
use ravenrs_core::{Color, Rect};
use ravenrs_report::{Band, Control, Document, Item, Page, Report, ReportEngine, Style, samples};
use ravenrs_report_pdf::to_pdf;
use ravenrs_text::{TextAttrs, TextSystem};
use serde_json::json;

fn invoice() -> Document {
    let report = samples::invoice();
    let data = report.embedded_data().cloned().unwrap();
    ReportEngine::new().render(&report, &data)
}

/// The objects of a PDF file, read by following its syntax. It also checks the
/// cross-reference table, the part readers rely on to find objects.
struct Parsed {
    objects: HashMap<u32, (String, Vec<u8>)>,
}

fn read(document: &Document) -> Parsed {
    parse(&to_pdf(document).unwrap())
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack[from..].windows(needle.len()).position(|w| w == needle).map(|i| i + from)
}

fn parse(bytes: &[u8]) -> Parsed {
    assert!(bytes.starts_with(b"%PDF-1.7"));
    let xref = find(bytes, b"xref\n", 0).unwrap();
    let mut objects = HashMap::new();
    let mut at = 0;
    while let Some(mark) = find(&bytes[..xref], b" 0 obj\n", at) {
        let start = bytes[..mark].iter().rposition(|b| !b.is_ascii_digit()).map_or(0, |i| i + 1);
        let id: u32 = String::from_utf8_lossy(&bytes[start..mark]).parse().unwrap();
        let body = mark + 7;
        let end = find(bytes, b"endobj", body).unwrap();
        let stream = find(&bytes[..end], b"stream\n", body);
        let (dict, data, next) = match stream {
            Some(stream) => {
                let dict = String::from_utf8_lossy(&bytes[body..stream]).into_owned();
                let length: usize = number_after(&dict, "/Length ").unwrap();
                let data = bytes[stream + 7..stream + 7 + length].to_vec();
                let inflated =
                    if dict.contains("FlateDecode") { decompress_to_vec_zlib(&data).unwrap() } else { data };
                (dict, inflated, find(bytes, b"endobj", stream + 7 + length).unwrap() + 6)
            }
            None => (String::from_utf8_lossy(&bytes[body..end]).into_owned(), Vec::new(), end + 6),
        };
        // The cross-reference table must point at this object.
        let entry = find(bytes, format!("{start:010} 00000 n").as_bytes(), xref);
        assert!(entry.is_some(), "object {id} at {start} is not in the table");
        objects.insert(id, (dict, data));
        at = next;
    }
    Parsed { objects }
}

fn number_after<T: std::str::FromStr>(text: &str, key: &str) -> Option<T> {
    let rest = &text[text.find(key)? + key.len()..];
    rest.split_whitespace().next()?.trim_end_matches(['>', '/']).parse().ok()
}

impl Parsed {
    fn with(&self, marker: &str) -> Vec<&(String, Vec<u8>)> {
        self.objects.values().filter(|(dict, _)| dict.contains(marker)).collect()
    }

    fn first_page(&self) -> &String {
        let tree = &self.with("/Type /Pages")[0].0;
        let kid: u32 = number_after(tree, "/Kids [").unwrap();
        &self.objects[&kid].0
    }

    fn page_content(&self) -> String {
        let id: u32 = number_after(self.first_page(), "/Contents ").unwrap();
        String::from_utf8_lossy(&self.objects[&id].1).into_owned()
    }

    /// The Unicode text of each CID of a resource font (`F0` or `F1`).
    fn to_unicode(&self, font: &str) -> HashMap<u16, String> {
        let resources = &self.with(&format!("/{font} "))[0].0;
        let type0: u32 = number_after(resources, &format!("/{font} ")).unwrap();
        let cmap: u32 = number_after(&self.objects[&type0].0, "/ToUnicode ").unwrap();
        let cmap = String::from_utf8_lossy(&self.objects[&cmap].1).into_owned();
        let mut map = HashMap::new();
        for section in cmap.split("beginbfchar").skip(1) {
            let section = section.split("endbfchar").next().unwrap();
            let hex: Vec<&str> =
                section.split(['<', '>', ' ', '\n']).filter(|t| !t.trim().is_empty()).collect();
            for pair in hex.chunks(2) {
                let units: Vec<u16> = pair[1]
                    .as_bytes()
                    .chunks(4)
                    .map(|u| u16::from_str_radix(std::str::from_utf8(u).unwrap(), 16).unwrap())
                    .collect();
                map.insert(u16::from_str_radix(pair[0], 16).unwrap(), String::from_utf16(&units).unwrap());
            }
        }
        map
    }

    /// The first page's text, decoded the way a reader does: glyph codes through ToUnicode.
    fn page_text(&self) -> String {
        let fonts = [("F0", self.to_unicode("F0")), ("F1", self.to_unicode("F1"))];
        let id: u32 = number_after(self.first_page(), "/Contents ").unwrap();
        let content = &self.objects[&id].1;
        let mut current = 0;
        let mut out = String::new();
        let mut i = 0;
        while i < content.len() {
            match content[i] {
                b'/' if content[i..].starts_with(b"/F0 ") => current = 0,
                b'/' if content[i..].starts_with(b"/F1 ") => current = 1,
                b'[' => out.push('\n'),
                b'(' | b'<' => {
                    let (codes, next) = pdf_string(content, i);
                    for code in codes.chunks(2) {
                        let cid = u16::from_be_bytes([code[0], code[1]]);
                        out.push_str(fonts[current].1.get(&cid).map_or("", String::as_str));
                    }
                    i = next;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
        out
    }
}

/// Decodes the literal or hexadecimal string at `at`; returns it and where it ends.
fn pdf_string(content: &[u8], at: usize) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut i = at + 1;
    if content[at] == b'<' {
        let end = i + content[i..].iter().position(|b| *b == b'>').unwrap();
        let hex: Vec<u8> = content[i..end].iter().copied().filter(u8::is_ascii_hexdigit).collect();
        for pair in hex.chunks(2) {
            out.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
        }
        return (out, end + 1);
    }
    let mut depth = 1;
    loop {
        match content[i] {
            b'\\' => {
                i += 1;
                match content[i] {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    digit @ b'0'..=b'7' => {
                        let mut value = u32::from(digit - b'0');
                        for _ in 0..2 {
                            if matches!(content[i + 1], b'0'..=b'7') {
                                i += 1;
                                value = value * 8 + u32::from(content[i] - b'0');
                            }
                        }
                        out.push(value as u8);
                    }
                    other => out.push(other),
                }
            }
            b'(' => {
                depth += 1;
                out.push(b'(');
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return (out, i + 1);
                }
                out.push(b')');
            }
            other => out.push(other),
        }
        i += 1;
    }
}

#[test]
fn pages_and_sizes_match_the_document() {
    let report = samples::sales_by_region(500);
    let data = report.embedded_data().cloned().unwrap();
    let doc = ReportEngine::new().render(&report, &data);
    let pdf = read(&doc);
    assert_eq!(pdf.with("/Type /Page\n").len(), doc.pages.len());
    let (w, h) = (doc.pages[0].width, doc.pages[0].height);
    assert!(pdf.first_page().contains(&format!("/MediaBox [0 0 {w} {h}]")), "{}", pdf.first_page());
}

#[test]
fn text_is_extractable_with_accents() {
    let text = read(&invoice()).page_text();
    for needle in ["FATURA", "Kestrel Saúde S.A.", "OBSERVAÇÕES", "Instalação e configuração"] {
        assert!(text.contains(needle), "{needle} not in {text}");
    }
}

#[test]
fn fonts_are_embedded_subsets() {
    let bytes = to_pdf(&invoice()).unwrap();
    let pdf = parse(&bytes);
    let fonts = pdf.with("/Subtype /Type0");
    assert_eq!(fonts.len(), 2);
    for (font, _) in fonts {
        let name = font.split("/BaseFont /").nth(1).unwrap();
        assert_eq!(name.as_bytes()[6], b'+', "a subset's name starts with a tag: {name}");
    }
    assert_eq!(pdf.with("/FontFile2").len(), 2);
    // Two full copies of Inter would be 800 KB; subsets keep an invoice small.
    assert!(bytes.len() < 60 * 1024, "{} bytes", bytes.len());
}

#[test]
fn title_is_stored_in_the_document_info() {
    let doc = Document { title: "Relatório de Março".into(), ..invoice() };
    let bytes = to_pdf(&doc).unwrap();
    let key = b"/Title ";
    let (title, _) = pdf_string(&bytes, find(&bytes, key, 0).unwrap() + key.len());
    // Accented text is stored as UTF-16 with a byte order mark.
    let (mark, text) = title.split_at(2);
    assert_eq!(mark, [0xfe, 0xff]);
    let units: Vec<u16> = text.chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    assert_eq!(String::from_utf16(&units).unwrap(), "Relatório de Março");
}

#[test]
fn shaping_for_the_file_agrees_with_the_measured_width() {
    let doc = invoice();
    let mut text = TextSystem::new();
    for item in &doc.pages[0].items {
        let Item::Text(block) = item else { continue };
        let attrs = TextAttrs {
            size: 1000.0,
            line_height: 1000.0,
            weight: if block.bold { 600.0 } else { 400.0 },
            ..TextAttrs::default()
        };
        for line in &block.lines {
            let mut layout = text.layout(line.text.trim_end(), &attrs);
            layout.measure(None);
            let glyphs = layout.glyphs();
            let end = glyphs.last().map_or(0.0, |g| g.x + g.advance) * block.size / 1000.0;
            assert!((end - line.width).abs() < 0.1, "{:?}: {end} vs {}", line.text, line.width);
        }
    }
}

#[test]
fn translucent_colors_use_a_graphics_state() {
    let doc = Document {
        title: "T".into(),
        culture: "en-US".into(),
        pages: vec![Page {
            width: 200.0,
            height: 200.0,
            items: vec![
                Item::Rect {
                    rect: Rect::new(10.0, 10.0, 50.0, 50.0),
                    fill: Some(Color::rgba(1.0, 0.0, 0.0, 0.5)),
                    stroke: None,
                    radius: 6.0,
                },
                Item::Ellipse {
                    rect: Rect::new(70.0, 10.0, 50.0, 50.0),
                    fill: None,
                    stroke: Some((1.0, Color::rgba(0.0, 0.0, 1.0, 1.0))),
                },
            ],
            ..Page::default()
        }],
        errors: Vec::new(),
    };
    let pdf = read(&doc);
    let state = &pdf.with("/ca ")[0].0;
    assert!((number_after::<f32>(state, "/ca ").unwrap() - 0.5).abs() < 0.01, "{state}");
    let content = pdf.page_content();
    assert!(content.contains("/A128 gs"), "{content}");
    // A rounded corner and the ellipse are curves.
    assert!(content.matches(" c\n").count() >= 8, "{content}");
    // y is flipped: the box at the top of the page sits near the top of PDF space.
    assert!(content.contains("190"), "{content}");
}

#[test]
fn text_wider_than_its_box_is_clipped_and_fitting_text_is_not() {
    let render = |width: f32| {
        let report = Report::new("T").detail(
            Band::new(20.0).control(
                Control::label("Um texto bem comprido que não cabe")
                    .at(0.0, 0.0, width, 20.0)
                    .style(Style { word_wrap: Some(false), ..Style::new() }),
            ),
        );
        read(&ReportEngine::new().render(&report, &json!([{}]))).page_content()
    };
    assert!(render(40.0).contains("W\n"));
    assert!(!render(400.0).contains("W\n"));
}

#[test]
fn an_empty_report_still_gives_a_valid_file() {
    let doc = ReportEngine::new().render(&Report::new("Vazio"), &json!([]));
    assert_eq!(read(&doc).with("/Type /Page\n").len(), doc.pages.len());
}

fn encoded_image(format: image::ImageFormat, alpha: u8) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    if format == image::ImageFormat::Jpeg {
        image::RgbImage::from_pixel(32, 16, image::Rgb([20, 120, 200])).write_to(&mut bytes, format).unwrap();
    } else {
        image::RgbaImage::from_pixel(32, 16, image::Rgba([20, 120, 200, alpha]))
            .write_to(&mut bytes, format)
            .unwrap();
    }
    bytes.into_inner()
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = u32::from(chunk[0]) << 16
            | u32::from(chunk.get(1).copied().unwrap_or(0)) << 8
            | u32::from(chunk.get(2).copied().unwrap_or(0));
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

fn rows(band: Band, count: usize) -> Document {
    let report = Report::new("T").detail(band);
    let data: Vec<_> = (0..count).map(|i| json!({ "N": format!("Linha {i}") })).collect();
    ReportEngine::new().render(&report, &json!(data))
}

#[test]
fn an_image_on_many_rows_is_stored_once() {
    let png = encode_base64(&encoded_image(image::ImageFormat::Png, 255));
    let doc = rows(Band::new(40.0).control(Control::picture(png).at(0.0, 0.0, 32.0, 16.0)), 60);
    assert!(doc.pages.len() > 1);
    let pdf = read(&doc);
    assert_eq!(pdf.with("/Subtype /Image").len(), 1);
    assert!(pdf.page_content().contains(" Do"), "{}", pdf.page_content());
}

#[test]
fn a_jpeg_keeps_its_original_file_and_a_png_with_transparency_gets_a_mask() {
    let jpeg = encoded_image(image::ImageFormat::Jpeg, 255);
    let doc =
        rows(Band::new(40.0).control(Control::picture(encode_base64(&jpeg)).at(0.0, 0.0, 32.0, 16.0)), 1);
    let bytes = to_pdf(&doc).unwrap();
    assert!(find(&bytes, &jpeg, 0).is_some(), "the JPEG file is in the PDF byte for byte");
    assert!(parse(&bytes).with("/DCTDecode").len() == 1);

    let opaque = encode_base64(&encoded_image(image::ImageFormat::Png, 255));
    let doc = rows(Band::new(40.0).control(Control::picture(opaque).at(0.0, 0.0, 32.0, 16.0)), 1);
    assert!(read(&doc).with("/SMask").is_empty());

    let clear = encode_base64(&encoded_image(image::ImageFormat::Png, 128));
    let doc = rows(Band::new(40.0).control(Control::picture(clear).at(0.0, 0.0, 32.0, 16.0)), 1);
    let pdf = read(&doc);
    assert_eq!(pdf.with("/SMask").len(), 1);
    assert_eq!(pdf.with("/Subtype /Image").len(), 2, "the image and its mask");
}

#[test]
fn links_become_annotations_where_the_text_is() {
    let doc = rows(
        Band::new(40.0).control(Control::link("site", "https://example.com/a").at(10.0, 0.0, 100.0, 20.0)),
        1,
    );
    let bytes = to_pdf(&doc).unwrap();
    let pdf = parse(&bytes);
    let annotation = &pdf.with("/Subtype /Link")[0].0;
    assert!(annotation.contains("/URI (https://example.com/a)"), "{annotation}");
    assert!(annotation.contains("/Border [0 0 0]"), "{annotation}");
    assert!(pdf.first_page().contains("/Annots"), "{}", pdf.first_page());
    let (w, h) = (doc.pages[0].width, doc.pages[0].height);
    assert!(w > 0.0 && h > 0.0);
}

#[test]
fn no_bookmarks_means_no_outline() {
    let pdf = read(&rows(Band::new(20.0).control(Control::field("[N]").at(0.0, 0.0, 100.0, 20.0)), 1));
    assert!(pdf.with("/Outlines").is_empty());
    assert!(pdf.with("/PageMode").is_empty());
}

#[test]
fn bookmarks_build_a_tree_that_opens_on_the_right_page() {
    let band = Band::new(100.0)
        .control(Control::bookmark("[N]", 0))
        .control(Control::bookmark("'Sub ' + [N]", 1))
        .control(Control::field("[N]").at(0.0, 0.0, 100.0, 20.0));
    let doc = rows(band, 12);
    assert!(doc.pages.len() >= 2);
    let bytes = to_pdf(&doc).unwrap();
    let pdf = parse(&bytes);
    let catalog = &pdf.with("/Type /Catalog")[0].0;
    assert!(catalog.contains("/Outlines") && catalog.contains("/PageMode /UseOutlines"), "{catalog}");
    let root_id: u32 = number_after(catalog, "/Outlines ").unwrap();
    let root = &pdf.objects[&root_id].0;
    assert_eq!(number_after::<i32>(root, "/Count "), Some(24));
    // Walk the top level by its links, and each entry's single child.
    let mut current: u32 = number_after(root, "/First ").unwrap();
    let mut pages = Vec::new();
    let mut seen = 0;
    loop {
        let item = &pdf.objects[&current].0;
        assert_eq!(number_after::<u32>(item, "/Parent "), Some(root_id));
        assert_eq!(number_after::<i32>(item, "/Count "), Some(1));
        let child: u32 = number_after(item, "/First ").unwrap();
        assert_eq!(number_after::<u32>(item, "/Last "), Some(child));
        assert_eq!(number_after::<u32>(&pdf.objects[&child].0, "/Parent "), Some(current));
        pages.push(number_after::<u32>(item, "/Dest [").unwrap());
        seen += 1;
        match number_after::<u32>(item, "/Next ") {
            Some(next) => current = next,
            None => break,
        }
    }
    assert_eq!(seen, 12);
    assert_eq!(number_after::<u32>(root, "/Last "), Some(current));
    assert!(pages.first() != pages.last(), "entries lead to the pages they are on");
}

#[test]
fn a_level_that_skips_one_still_gives_a_valid_tree() {
    let band = Band::new(30.0).control(Control::bookmark("[N]", 3));
    let pdf = read(&rows(band, 2));
    let items = pdf.with("/Parent ");
    let root: u32 = number_after(&pdf.with("/Type /Catalog")[0].0, "/Outlines ").unwrap();
    let entries: Vec<_> = items.iter().filter(|(item, _)| item.contains("/Title ")).collect();
    assert_eq!(entries.len(), 2);
    for (item, _) in entries {
        assert_eq!(number_after::<u32>(item, "/Parent "), Some(root), "both hang from the top: {item}");
    }
}

#[test]
fn accented_outline_titles_are_utf16() {
    let band = Band::new(30.0).control(Control::bookmark("'Região'", 0));
    let bytes = to_pdf(&rows(band, 1)).unwrap();
    let parsed = parse(&bytes);
    assert!(
        parsed.with("/Title ").iter().any(|(d, _)| d.contains('\u{fe}') || d.contains("FEFF")),
        "title not utf-16"
    );
}

#[test]
fn barcodes_are_vector_rectangles() {
    use ravenrs_report::barcode::Symbology;
    let band = Band::new(60.0).control(Control::barcode(Symbology::Code128, "[N]").at(0.0, 0.0, 200.0, 60.0));
    let pdf = read(&rows(band, 1));
    assert!(pdf.with("/Subtype /Image").is_empty(), "no raster");
    let content = pdf.page_content();
    assert!(content.matches("h\n").count() > 10, "{content}");
}

#[test]
fn charts_are_vector_with_searchable_text() {
    let report = samples::sales_dashboard();
    let data = report.embedded_data().cloned().unwrap();
    let doc = ReportEngine::new().render(&report, &data);
    let pdf = read(&doc);
    assert!(pdf.with("/Subtype /Image").is_empty(), "no raster");
    assert!(pdf.page_text().contains("Por região"));
    assert!(pdf.page_content().contains(" c\n"), "curves for the slices");
}
