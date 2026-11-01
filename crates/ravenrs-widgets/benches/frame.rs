//! A full frame (build, layout, paint) for typical screens. Resizing forces the
//! work each iteration, like a window being dragged. Budget: under 8 ms, half a
//! 60 Hz frame, so the GPU and the platform have the other half.
#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use ravenrs_core::Size;
use ravenrs_style::{Semantic, TextStyle, Theme};
use ravenrs_widgets::grid::{GridColumn, GridData, Summary, Value};
use ravenrs_widgets::*;

/// A settings-style form: labels, inputs, toggles, cards and buttons.
fn form() -> impl View {
    let rows: Vec<AnyView> = (0..40)
        .map(|i| {
            let card: AnyView = Box::new(Card::new(HStack::new((
                VStack::new((
                    Text::new(format!("Setting {i}")).style(TextStyle::Headline),
                    Text::new("A short description of what this option changes.")
                        .foreground(Semantic::InkMuted),
                ))
                .grow(1.0),
                Badge::new("Beta"),
                Button::new("Configure").icon(IconName::Settings),
            ))));
            card
        })
        .collect();
    ScrollView::new(rows).spacing(12.0).padding(24.0)
}

/// The form with a spinner, whose frames repeat the paint phase alone.
fn animated_form() -> impl View {
    VStack::new((Spinner::new(), form())).grow(1.0)
}

fn grid() -> impl View {
    let rows = (0..100_000)
        .map(|i| vec![Value::from(format!("Customer {i:06}")), Value::from("Brazil"), Value::from(i % 500)])
        .collect();
    DataGrid::new(
        GridData::new(rows),
        [
            GridColumn::text("Customer"),
            GridColumn::text("Country"),
            GridColumn::number("Seats").summary(Summary::Sum),
        ],
    )
    .height(720.0)
}

fn frame(c: &mut Criterion, name: &str, view: impl View + 'static) {
    let mut ui = Ui::new(view, Theme::raven_dark());
    let sizes = [Size::new(1280.0, 800.0), Size::new(1281.0, 800.0)];
    let mut flip = 0;
    ui.set_viewport(sizes[0], 2.0);
    ui.update();
    c.bench_function(name, |b| {
        b.iter(|| {
            flip ^= 1;
            ui.set_viewport(sizes[flip], 2.0);
            ui.update().primitives().len()
        })
    });
}

/// A frame where only an animation is due: the paint phase, with no build or layout.
fn motion_frame(c: &mut Criterion) {
    let mut ui = Ui::new(animated_form(), Theme::raven_dark());
    ui.set_viewport(Size::new(1280.0, 800.0), 2.0);
    let mut time = 0.0;
    ui.set_time(time);
    ui.update();
    c.bench_function("frame_form_40_cards_motion_only", |b| {
        b.iter(|| {
            time += 1.0 / 60.0;
            ui.set_time(time);
            ui.update().primitives().len()
        })
    });
}

fn bench(c: &mut Criterion) {
    frame(c, "frame_form_40_cards", form());
    frame(c, "frame_datagrid_100k", grid());
    motion_frame(c);
}

criterion_group!(benches, bench);
criterion_main!(benches);
