//! The report of a suite as one HTML page (T5 of #627): the summary, a table per scenario
//! with each block, and the failure with its meaning, fix and the app's screen. The page needs
//! nothing from the network, reads in light and dark, and fits a phone.

use crate::report::Report;

/// `reports` as one self-contained page.
pub fn page(reports: &[Report]) -> String {
    let passed = reports.iter().filter(|r| r.passed()).count();
    let blocks: usize = reports.iter().map(|r| r.total).sum();
    let ms: u64 = reports.iter().map(|r| r.duration_ms).sum();
    let verdict = if passed == reports.len() { "passed" } else { "failed" };
    let mut body = format!(
        "<header><h1>Relatório e2e</h1><p class=\"verdict {verdict}\">{}</p>\
         <dl><div><dt>Cenários</dt><dd>{passed} de {} passaram</dd></div>\
         <div><dt>Passos</dt><dd>{blocks}</dd></div><div><dt>Tempo</dt><dd>{}</dd></div></dl></header>",
        if verdict == "passed" { "Passou" } else { "Falhou" },
        reports.len(),
        seconds(ms)
    );
    for report in reports {
        body.push_str(&scenario(report));
    }
    if let Some(report) = reports.first() {
        body.push_str("<footer><h2>Não verificado</h2><ul>");
        for item in report.not_verified() {
            body.push_str(&format!("<li>{}</li>", esc(item)));
        }
        body.push_str("</ul></footer>");
    }
    format!(
        "<!doctype html>\n<html lang=\"pt-BR\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Relatório e2e</title><style>{STYLE}</style></head><body><main>{body}</main></body></html>\n"
    )
}

fn scenario(report: &Report) -> String {
    let state = if report.passed() { "passed" } else { "failed" };
    let mut out = format!(
        "<section class=\"{state}\"><h2>{}</h2><p class=\"summary\">{} · {}{}</p>\
         <div class=\"scroll\"><table><thead><tr><th>#</th><th>Bloco</th><th>Alvo</th><th>Resultado</th><th>Tempo</th></tr></thead><tbody>",
        esc(&report.scenario),
        esc(&crate::session::summary(&report.outcomes, report.total)),
        seconds(report.duration_ms),
        if report.cached_at.is_some() { " · do cache" } else { "" }
    );
    for (i, o) in report.outcomes.iter().enumerate() {
        let (class, result) = match (&o.error, &o.code) {
            (None, _) => ("ok", "passou".to_owned()),
            (Some(_), Some(code)) => ("bad", format!("falhou · <code>{}</code>", esc(code))),
            (Some(_), None) => ("bad", "falhou".to_owned()),
        };
        out.push_str(&format!(
            "<tr class=\"{class}\"><td>{}</td><td>{}<small>{}</small></td><td><code>{}</code></td><td>{result}</td><td>{}</td></tr>",
            i + 1,
            esc(&o.title),
            esc(&o.kind),
            esc(&o.line),
            seconds(o.ms)
        ));
    }
    for (i, title) in report.not_run.iter().enumerate() {
        out.push_str(&format!(
            "<tr class=\"skip\"><td>{}</td><td>{}</td><td></td><td>não rodou</td><td></td></tr>",
            report.outcomes.len() + i + 1,
            esc(title)
        ));
    }
    out.push_str("</tbody></table></div>");
    if let Some(f) = report.failure() {
        out.push_str("<div class=\"failure\"><h3>Por que falhou</h3>");
        out.push_str(&format!("<p>{}</p>", esc(f.error.as_deref().unwrap_or_default())));
        if let Some(fix) = &f.fix {
            out.push_str(&format!("<p><strong>Como corrigir:</strong> {}</p>", esc(fix)));
        }
        if !f.close.is_empty() {
            out.push_str("<p><strong>Parecidos na tela:</strong></p><ul>");
            for close in &f.close {
                out.push_str(&format!("<li><code>{}</code></li>", esc(close)));
            }
            out.push_str("</ul>");
        }
        if let Some(screen) = &f.screen {
            out.push_str(&format!(
                "<details open><summary>Tela do app testado</summary><pre>{}</pre></details>",
                esc(screen)
            ));
        }
        out.push_str("</div>");
    }
    if !report.films.is_empty() || report.film_note.is_some() {
        out.push_str("<div class=\"films\"><h3>Filme</h3><ul>");
        for film in &report.films {
            // The film sits beside the page, in the same folder, when the run was kept.
            let name = std::path::Path::new(film)
                .file_name()
                .map_or_else(|| film.clone(), |n| n.to_string_lossy().into_owned());
            out.push_str(&format!(
                "<li><a href=\"{}\">{}</a> <small>abre no MoviePlayer</small></li>",
                esc(&name),
                esc(&name)
            ));
        }
        if let Some(note) = &report.film_note {
            out.push_str(&format!("<li>{}</li>", esc(note)));
        }
        out.push_str("</ul></div>");
    }
    out.push_str("</section>");
    out
}

/// Text that goes inside an element or an attribute.
pub(crate) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn seconds(ms: u64) -> String {
    format!("{},{:03} s", ms / 1000, ms % 1000)
}

/// The page's look, with the RavenRS theme's colors for light and dark.
const STYLE: &str = ":root{--bg:#f7f8fa;--surface:#fff;--ink:#16181d;--muted:#5b6170;--line:#e2e5eb;\
--ok:#1d7f4e;--bad:#c2362b;--skip:#8a8f9c;--code:#eef0f4}\
@media (prefers-color-scheme:dark){:root{--bg:#121418;--surface:#1b1e24;--ink:#e8eaee;--muted:#9aa1ae;\
--line:#2c3038;--ok:#4cc38a;--bad:#ff6b5e;--skip:#7d8390;--code:#252932}}\
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);\
font:15px/1.5 system-ui,-apple-system,'Segoe UI',sans-serif}main{max-width:1040px;margin:0 auto;padding:24px 16px}\
h1{font-size:24px;margin:0 0 4px}h2{font-size:18px;margin:0}h3{font-size:15px;margin:0 0 8px}\
.verdict{display:inline-block;font-weight:600;margin:0 0 16px;padding:2px 10px;border-radius:999px;color:#fff}\
.verdict.passed{background:var(--ok)}.verdict.failed{background:var(--bad)}\
dl{display:flex;flex-wrap:wrap;gap:12px;margin:0 0 24px}dl div{background:var(--surface);border:1px solid var(--line);\
border-radius:8px;padding:8px 14px;min-width:120px}dt{color:var(--muted);font-size:13px}dd{margin:0;font-weight:600}\
section{background:var(--surface);border:1px solid var(--line);border-left:4px solid var(--ok);border-radius:8px;\
padding:16px;margin:0 0 16px}section.failed{border-left-color:var(--bad)}.summary{color:var(--muted);margin:4px 0 12px}\
.scroll{overflow-x:auto}table{border-collapse:collapse;width:100%;min-width:560px}th,td{text-align:left;padding:6px 8px;\
border-bottom:1px solid var(--line);vertical-align:top}th{color:var(--muted);font-weight:500;font-size:13px}\
td small{display:block;color:var(--muted);font-size:12px}tr.ok td:nth-child(4){color:var(--ok)}\
tr.bad td{color:var(--bad);font-weight:600}tr.skip td{color:var(--skip)}\
code,pre{font:13px/1.45 ui-monospace,SFMono-Regular,Menlo,monospace;background:var(--code);border-radius:4px}\
code{padding:1px 4px}pre{padding:12px;overflow-x:auto;white-space:pre}\
.films{margin-top:12px}.films a{color:inherit}.failure{margin-top:12px;padding-top:12px;border-top:1px dashed var(--line)}footer{color:var(--muted);font-size:13px}\
footer h2{font-size:14px}";
