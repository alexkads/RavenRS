//! Visual regression tests: components are rendered on the GPU and compared
//! with reference images in `tests/golden/`.
//!
//! * A missing reference is written on first run (and the test passes).
//! * `RAVENRS_UPDATE_GOLDEN=1 cargo test -p ravenrs --test visual` rewrites them
//!   after an intentional visual change; review the PNG diff before committing.
//! * Machines without a GPU adapter skip these tests.
//!
//! Comparison allows small per-pixel differences, since GPUs and drivers
//! rasterize anti-aliased edges slightly differently.

#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::collections::BTreeSet;
use std::path::PathBuf;

use ravenrs::prelude::*;
use ravenrs::render::{GpuContext, Renderer, wgpu};
use ravenrs::widgets::grid::Value;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Largest per-channel difference that still counts as the same pixel.
const CHANNEL_TOLERANCE: u8 = 24;
/// Share of pixels allowed to differ beyond the tolerance.
const MAX_DIFF_RATIO: f64 = 0.004;

fn gpu() -> Option<GpuContext> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    match pollster::block_on(GpuContext::new(&instance, None)) {
        Ok(gpu) => Some(gpu),
        Err(e) => {
            eprintln!("skipping visual test: {e}");
            None
        }
    }
}

fn render(gpu: &GpuContext, view: impl View + 'static, theme: Theme, size: Size) -> image::RgbaImage {
    let mut ui = Ui::new(view, theme);
    ui.set_viewport(size, 1.0);
    ui.update();
    ui.update();
    let (w, h) = (size.width as u32, size.height as u32);
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("visual"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, FORMAT);
    renderer.render(ui.scene(), &view, (w, h), 1.0);
    let padded = (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    gpu.queue.submit(Some(encoder.finish()));
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback"));
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let data = slice.get_mapped_range().expect("mapped range");
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded) as usize;
        pixels.extend_from_slice(&data[start..start + (w * 4) as usize]);
    }
    image::RgbaImage::from_raw(w, h, pixels).expect("image")
}

fn check(name: &str, actual: &image::RgbaImage) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let path = dir.join(format!("{name}.png"));
    let update = std::env::var_os("RAVENRS_UPDATE_GOLDEN").is_some();
    if update || !path.exists() {
        actual.save(&path).expect("write golden");
        eprintln!("wrote reference {}", path.display());
        return;
    }
    let expected = image::open(&path).expect("read golden").into_rgba8();
    assert_eq!(expected.dimensions(), actual.dimensions(), "{name}: size changed");
    let differing = expected
        .pixels()
        .zip(actual.pixels())
        .filter(|(a, b)| a.0.iter().zip(b.0.iter()).any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE))
        .count();
    let ratio = differing as f64 / f64::from(expected.width() * expected.height());
    if ratio > MAX_DIFF_RATIO {
        let out = dir.join(format!("{name}.actual.png"));
        actual.save(&out).expect("write actual");
        panic!(
            "{name}: {:.2}% of pixels differ from {} (limit {:.2}%); see {}",
            ratio * 100.0,
            path.display(),
            MAX_DIFF_RATIO * 100.0,
            out.display()
        );
    }
}

fn buttons() -> impl View {
    VStack::new((
        HStack::new((
            Button::new("Primary"),
            Button::new("Secondary").style(ButtonStyle::Secondary),
            Button::new("Ghost").style(ButtonStyle::Ghost),
            Button::new("Delete").style(ButtonStyle::Danger).icon(IconName::Trash2),
        ))
        .spacing(10.0),
        HStack::new((
            Button::new("Small").size(ButtonSize::Small),
            Button::new("Large").size(ButtonSize::Large).trailing_icon(IconName::ArrowRight),
            Button::new("Disabled").disabled(true),
            IconButton::new(IconName::Settings).style(ButtonStyle::Secondary),
        ))
        .spacing(10.0)
        .align(Align::Center),
    ))
    .spacing(14.0)
    .padding(20.0)
}

fn inputs() -> impl View {
    VStack::new((
        TextField::new(State::new("Ana Lima".to_string()).binding()).icon(IconName::User),
        TextField::new(State::new(String::new()).binding()).placeholder("Search…").icon(IconName::Search),
        HStack::new((
            Toggle::new(State::new(true).binding()).label("On"),
            Toggle::new(State::new(false).binding()).label("Off"),
            Checkbox::new("Checked", State::new(true).binding()),
            Checkbox::new("Unchecked", State::new(false).binding()),
        ))
        .spacing(16.0),
        Slider::new(State::new(0.35_f32).binding()),
        SegmentedControl::new(State::new(1).binding(), ["Day", "Week", "Month"]),
        RadioGroup::new(State::new(0).binding(), ["Starter", "Team"]),
    ))
    .spacing(14.0)
    .padding(20.0)
    .align(Align::Stretch)
}

fn cards_and_feedback() -> impl View {
    VStack::new((
        Card::new(Stat::new("Revenue", "R$ 128.450").delta("+12.4%", true)),
        Alert::new("Storage almost full", "You are using 92% of your quota.").tone(Tone::Warning),
        HStack::new((
            Badge::new("New").tone(Tone::Accent),
            Badge::new("Paid").tone(Tone::Success).icon(IconName::Check),
            Avatar::new("Ana Lima"),
            Kbd::new("⌘K"),
        ))
        .spacing(8.0)
        .align(Align::Center),
        ProgressBar::new(0.62),
    ))
    .spacing(14.0)
    .padding(20.0)
    .align(Align::Stretch)
}

fn calendar() -> impl View {
    let d = Date::new(2026, 10, 31).expect("date");
    VStack::new((
        Calendar::new(State::new(Some(Date::new(2026, 10, 14).expect("date"))).binding(), d).today(d),
    ))
    .padding(16.0)
}

fn data_grid() -> impl View {
    let rows = (0..12)
        .map(|i| {
            vec![
                Value::from(format!("Customer {i:02}")),
                Value::from(["Brazil", "Chile", "Peru"][i % 3]),
                Value::from((i * 37 % 100) as f64),
            ]
        })
        .collect();
    VStack::new((DataGrid::new(
        GridData::new(rows),
        [
            GridColumn::text("Customer"),
            GridColumn::text("Country"),
            GridColumn::number("Seats").summary(Summary::Sum),
        ],
    )
    .selection_mode(SelectionMode::Multiple)
    .selection(State::new(BTreeSet::from([1, 2])).binding())
    .sort_by(2, SortDirection::Descending)
    .height(380.0),))
    .padding(12.0)
}

fn ribbon() -> impl View {
    VStack::new((Ribbon::new()
        .app_menu("File", vec![MenuItem::new("Open")])
        .quick_access(RibbonItem::button("Save", IconName::Save))
        .page(
            RibbonPage::new("Home")
                .group(
                    RibbonGroup::new("Clipboard")
                        .item(RibbonItem::button("Paste", IconName::ClipboardPaste))
                        .item(RibbonItem::button("Cut", IconName::Scissors).small())
                        .item(RibbonItem::button("Copy", IconName::Copy).small()),
                )
                .group(RibbonGroup::new("Font").item(
                    RibbonItem::toggle("Bold", IconName::Bold, State::new(true).binding()).icon_only(),
                )),
        )
        .page(RibbonPage::new("Insert"))
        .page(RibbonPage::new("Layout").contextual("Table tools", Tone::Warning, true)),))
}

macro_rules! visual {
    ($name:ident, $view:expr, $w:expr, $h:expr) => {
        #[test]
        fn $name() {
            let Some(gpu) = gpu() else { return };
            for (theme, suffix) in [(Theme::raven_dark(), "dark"), (Theme::raven_light(), "light")] {
                let img = render(&gpu, $view, theme, Size::new($w, $h));
                check(&format!("{}_{suffix}", stringify!($name)), &img);
            }
        }
    };
}

visual!(buttons_all_styles, buttons(), 520.0, 140.0);
visual!(inputs_and_choices, inputs(), 480.0, 420.0);
visual!(cards_alerts_badges, cards_and_feedback(), 480.0, 330.0);
visual!(calendar_month, calendar(), 300.0, 340.0);
visual!(data_grid_sorted_selected, data_grid(), 520.0, 404.0);
visual!(ribbon_home_page, ribbon(), 700.0, 160.0);
