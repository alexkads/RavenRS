# ravenrs-report-label

RavenRS Reporting on label and receipt printers. A document becomes a 1-bit bitmap at the printer's resolution, and the bitmap goes inside ZPL II, EPL2/PPLB, PPLA, TSPL or ESC/POS. The layout is the one made in the designer, so it comes out the same on every brand, accents included.

> **Aviso:** testado por decodificação, ainda não em impressora física. Os bytes de cada linguagem foram lidos de volta por um parser de teste e os códigos de barras por um decodificador independente (`rxing`). Nenhuma etiqueta foi impressa. `FONTES.md` diz o que cada manual confirmou e o que ficou de memória; a primeira impressão em cada linguagem ainda precisa ser conferida com papel e leitor.

```rust
use ravenrs_report_label::{CupsOutput, LabelJob, LabelOutput, Language, compile};

let job = LabelJob { copies: 2, ..LabelJob::new(Language::Zpl, 203) };
let bytes = compile(&document, &job)?;
CupsOutput::new("Etiquetas")?.send(&bytes)?;
```

- Each page of the document is one label, or one receipt with a cut after it. A long receipt goes in several blocks. For an 80 mm roll make the page 204.3 pt wide (576 dots); for 58 mm, 136.2 pt (384 dots).
- Bars are laid on whole dots, centered, with no smoothing. A code that cannot keep its minimum module gives a `LabelError` that says which one and how much room it needs.
- `presets()` lists printers by name; a preset only picks the language.
- Outputs: `TcpOutput` (port 9100), `CupsOutput` (`lp -d name -o raw`, no shell), `FileOutput` (`.prn`) and, on Windows, `WindowsShareOutput` (`\\host\share`). In wasm32 only the core compiles.
- `bitmap_to_png` shows what goes to the printer: `cargo run -p ravenrs-report-label --example preview -- etiqueta.png`.

Labels on an A4 sheet do not come here: they go through the normal print path of `ravenrs-report-viewer`.

Licensed under MIT or Apache-2.0.
