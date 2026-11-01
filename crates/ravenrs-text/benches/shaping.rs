//! Text shaping and line breaking, the main cost of building text-heavy views.
#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use ravenrs_text::{TextAttrs, TextSystem};

const PARAGRAPH: &str = "RavenRS draws every component on the GPU, in a native window and in the browser. \
    One declarative UI, the same layout engine and the same renderer on every platform.";

fn bench(c: &mut Criterion) {
    let mut text = TextSystem::new();
    let attrs = TextAttrs::default();
    c.bench_function("shape_label", |b| {
        b.iter(|| text.layout(black_box("Get Started"), &attrs).measure(None))
    });
    c.bench_function("shape_and_wrap_paragraph", |b| {
        b.iter(|| text.layout(black_box(PARAGRAPH), &attrs).measure(Some(320.0)))
    });
    let mut layout = text.layout(PARAGRAPH, &attrs);
    let mut width = 320.0;
    c.bench_function("rewrap_paragraph", |b| {
        b.iter(|| {
            width = if width == 320.0 { 321.0 } else { 320.0 };
            layout.measure(Some(black_box(width)))
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
