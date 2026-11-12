//! Makes a short video from a recorded film and reads it back, box by box and
//! picture by picture.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests")]

use ravenrs_core::Size;
use ravenrs_movie::{Movie, Narration, Recorder, Soundtrack};
use ravenrs_movie_video::{Layout, Video, VideoError, VideoFormat, chapters, subtitles};
use ravenrs_style::Theme;
use ravenrs_widgets::{Button, Text, VStack};

fn film() -> Movie {
    let app =
        VStack::new((Text::new("Raven"), Button::new("Save").on_click(|| {}))).spacing(12.0).padding(40.0);
    let mut take = Recorder::new(app, Theme::raven_light(), Size::new(640.0, 360.0));
    take.intro("Saving", "How to save.");
    take.explain("Save", "The button", "Click it to save.").expect("on screen");
    take.click("Save").expect("on screen");
    take.wait(0.5);
    take.finish().expect("a film")
}

/// The boxes of a QuickTime file, by their path, such as "moov/trak/mdia".
fn boxes(bytes: &[u8], path: &str, out: &mut Vec<(String, std::ops::Range<usize>)>) {
    let mut at = 0;
    while at + 8 <= bytes.len() {
        let mut size = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let kind = String::from_utf8_lossy(&bytes[at + 4..at + 8]).to_string();
        let mut header = 8;
        if size == 1 {
            size = u64::from_be_bytes(bytes[at + 8..at + 16].try_into().unwrap()) as usize;
            header = 16;
        }
        assert!(size >= header && at + size <= bytes.len(), "box {kind} fits");
        let name = if path.is_empty() { kind.clone() } else { format!("{path}/{kind}") };
        out.push((name.clone(), at + header..at + size));
        if ["moov", "trak", "mdia", "minf", "stbl", "dinf"].contains(&kind.as_str()) {
            let mut inner = Vec::new();
            boxes(&bytes[at + header..at + size], &name, &mut inner);
            out.extend(inner.into_iter().map(|(n, r)| (n, r.start + at + header..r.end + at + header)));
        }
        at += size;
    }
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
}

#[test]
fn a_film_becomes_a_video_that_decodes_to_what_was_drawn() {
    let movie = film();
    let format = VideoFormat::new(640, 360, Layout::Full);
    let video = Video::new(movie.clone(), format);
    let path = std::env::temp_dir().join(format!("ravenrs-video-{}.mov", std::process::id()));
    let report = match video.write(&path, |_, _| {}) {
        Err(VideoError::NoGpu) => return,
        other => other.expect("a video"),
    };
    let bytes = std::fs::read(&path).expect("the file");
    std::fs::remove_file(&path).ok();
    assert_eq!(report.bytes, bytes.len() as u64);
    assert_eq!(report.frames, (movie.duration_ms() * 30).div_ceil(1000));
    let mut found = Vec::new();
    boxes(&bytes, "", &mut found);
    let get =
        |name: &str| found.iter().filter(|(n, _)| n == name).map(|(_, r)| r.clone()).collect::<Vec<_>>();
    assert_eq!(&bytes[8..12], b"qt  ");
    assert_eq!(get("moov/trak").len(), 2, "pictures and sound");
    // The pictures: their sizes, where they are, and the parameter sets.
    let stsz = get("moov/trak/mdia/minf/stbl/stsz")[0].clone();
    let count = u32_at(&bytes, stsz.start + 8) as usize;
    assert_eq!(count as u64, report.frames);
    let sizes: Vec<usize> = (0..count).map(|i| u32_at(&bytes, stsz.start + 12 + i * 4) as usize).collect();
    let stco = get("moov/trak/mdia/minf/stbl/stco")[0].clone();
    let offsets: Vec<usize> = (0..count).map(|i| u32_at(&bytes, stco.start + 8 + i * 4) as usize).collect();
    let stsd = get("moov/trak/mdia/minf/stbl/stsd")[0].clone();
    let avcc =
        bytes[stsd.start..stsd.end].windows(4).position(|w| w == b"avcC").expect("avcC") + stsd.start + 4;
    let sps_len = u16::from_be_bytes([bytes[avcc + 6], bytes[avcc + 7]]) as usize;
    let sps = &bytes[avcc + 8..avcc + 8 + sps_len];
    let pps_len = u16::from_be_bytes([bytes[avcc + 9 + sps_len], bytes[avcc + 10 + sps_len]]) as usize;
    let pps = &bytes[avcc + 11 + sps_len..avcc + 11 + sps_len + pps_len];
    // Decoded the way a player would, the last picture matches the last moment drawn.
    let mut decoder = openh264::decoder::Decoder::new().expect("decoder");
    let mut stream = Vec::new();
    for nal in [sps, pps] {
        stream.extend_from_slice(&[0, 0, 0, 1]);
        stream.extend_from_slice(nal);
    }
    let mut last = None;
    for (offset, size) in offsets.iter().zip(&sizes) {
        let mut at = *offset;
        while at < offset + size {
            let len = u32_at(&bytes, at) as usize;
            stream.extend_from_slice(&[0, 0, 0, 1]);
            stream.extend_from_slice(&bytes[at + 4..at + 4 + len]);
            at += 4 + len;
        }
        if let Ok(Some(yuv)) = decoder.decode(&stream) {
            use openh264::formats::YUVSource;
            let mut rgb = vec![0; yuv.rgb8_len()];
            yuv.write_rgb8(&mut rgb);
            last = Some(rgb);
        }
        stream.clear();
    }
    let decoded = last.expect("pictures decode");
    let last_ms = (report.frames - 1) * 1000 / 30;
    let png = video.thumbnail(last_ms).expect("a picture");
    let drawn = image::load_from_memory(&png).expect("png").into_rgb8();
    let difference: u64 =
        drawn.as_raw().iter().zip(&decoded).map(|(a, b)| u64::from(a.abs_diff(*b))).sum::<u64>()
            / decoded.len() as u64;
    assert!(difference < 6, "the decoded picture is what was drawn, off by {difference} on average");
    // The sound: as many samples as the pictures last.
    let sound_stsz = get("moov/trak/mdia/minf/stbl/stsz")[1].clone();
    let samples = u64::from(u32_at(&bytes, sound_stsz.start + 8));
    assert_eq!(samples, report.frames * 48_000 / 30);
}

#[test]
fn the_soundtrack_says_each_box_when_it_shows() {
    let movie = film();
    let spot = &movie.hotspots()[1];
    let said = format!("{}. {}", spot.title, spot.text);
    let voice: Vec<f32> = (0..8_000).map(|i| (i as f32 / 10.0).sin() * 0.5).collect();
    let bytes: &'static [u8] = Box::leak(Narration::pack(&[(said, voice)], 16_000).into_boxed_slice());
    let sound = Soundtrack::of(&movie, Some(&Narration::new(bytes)), 48_000);
    assert_eq!(sound.unrecorded().len(), 1, "the intro had no recording");
    let start = (spot.from_ms as usize) * 48;
    let loud = |from: usize, to: usize| sound.samples()[from..to].iter().any(|s| s.abs() > 0.1);
    assert!(!loud(start.saturating_sub(4_800), start.saturating_sub(100)), "silent before");
    assert!(loud(start, start + 4_800), "the voice starts with the box");
}

#[test]
fn subtitles_and_chapters_follow_the_film() {
    let movie = film();
    let srt = subtitles(&[movie.clone(), movie.clone()]);
    assert!(srt.starts_with("1\n00:00:00,"), "{srt}");
    assert!(srt.contains("The button\nClick it to save."), "{srt}");
    assert_eq!(srt.matches(" --> ").count(), movie.hotspots().len() * 2);
    let list = chapters(&[movie.clone(), movie.clone()]);
    let second = movie.duration_ms() / 1000;
    assert_eq!(list, format!("0:00 Saving\n{}:{:02} Saving\n", second / 60, second % 60));
}

#[test]
fn following_zooms_into_each_spot_it_explains() {
    let movie = film();
    let followed = movie.following(2.0);
    assert!(followed.project().overlays.len() > movie.project().overlays.len());
}
