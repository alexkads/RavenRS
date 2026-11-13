# ravenrs-pdf

Headless PDF engine for [RavenRS](https://github.com/alexkads/RavenRS), in pure Rust: reading
(damaged and encrypted files too), writing with incremental and full saves, merging and splitting,
page order and rotation, attachments and metadata, font metrics and shaping, and pages rendered to
pixels with their text extracted. Nothing here draws to a window, so it runs on servers and in
WebAssembly too.

```rust
use std::sync::Arc;
use ravenrs_pdf::cos::Document;
use ravenrs_pdf::render::{PageRenderer, RenderConfig, RenderRequest};

// A one-page PDF drawing a red square; the reader rebuilds the missing cross-reference table.
let bytes = Arc::new(
    b"%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n\
      2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n\
      3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >> endobj\n\
      4 0 obj << /Length 27 >> stream\n1 0 0 rg 25 25 50 50 re f\nendstream endobj\n\
      trailer << /Root 1 0 R >>\n%%EOF\n"
        .to_vec(),
);

// Pages to pixels: premultiplied RGBA, one point per pixel at scale 1.
let mut renderer = PageRenderer::new(bytes.clone(), RenderConfig::default());
assert_eq!(renderer.page_count(), 1);
let page = renderer.render(RenderRequest { page: 0, scale: 1.0, ..Default::default() });
assert_eq!((page.width, page.height), (100, 100));
let centre = ((50 * 100 + 50) * 4) as usize;
assert_eq!(&page.rgba[centre..centre + 3], &[255, 0, 0]);

// The same file twice, as one document of two pages.
let doc = Document::open(bytes)?;
let both = ravenrs_pdf::organize::combine(&[("a.pdf", &doc), ("b.pdf", &doc)])?;
assert_eq!(ravenrs_pdf::organize::page_count(&both)?, 2);
```

The engine comes from [PdfCraft](https://github.com/storytold/pdfcraft); see `NOTICE`.

Licensed under MIT or Apache-2.0; the bundled fonts and data keep their own licences (`LICENSES/`).
