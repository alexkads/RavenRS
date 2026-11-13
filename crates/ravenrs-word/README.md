# ravenrs-word

Headless word-processing engine for [RavenRS](https://github.com/alexkads/RavenRS): a document
model with paragraphs, tables, styles, lists, sections, fields, notes, comments and tracked
changes, and reading and writing of `.docx` (password-protected files too), `.doc` (read-only),
RTF, OpenDocument Text, HTML, Markdown, LaTeX and plain text. Nothing here lays out pages or draws,
so it runs on servers and in WebAssembly too.

```rust
// Markdown in, DOCX out, and back again.
let document = ravenrs_word::formats::import("md", b"# Letter\n\nDear *reader*,").unwrap().unwrap();
let docx = ravenrs_word::docx::write(&document).unwrap();
let again = ravenrs_word::docx::read(&docx).unwrap();
let markdown = ravenrs_word::formats::export("md", &again).unwrap().unwrap();
assert!(String::from_utf8(markdown).unwrap().contains("Letter"));
```

The engine comes from [WordCraft](https://github.com/storytold/wordcraft); see `NOTICE`.

Licensed under MIT or Apache-2.0.
