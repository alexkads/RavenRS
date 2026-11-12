//! The words of a film as text files: subtitles and a list of chapters.

use ravenrs_movie::Movie;

fn clock(ms: u64, separator: char) -> String {
    let (h, m, s, rest) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    format!("{h:02}:{m:02}:{s:02}{separator}{rest:03}")
}

/// What the boxes of `films`, one after the other, say and when, as SubRip
/// subtitles (`.srt`), which video sites take beside the video.
pub fn subtitles(films: &[Movie]) -> String {
    let mut out = String::new();
    let (mut number, mut offset) = (1, 0);
    for film in films {
        let spots = film.hotspots();
        for (i, spot) in spots.iter().enumerate() {
            // A box shows until the next one takes its place.
            let next = spots.get(i + 1).map_or(u64::MAX, |n| n.from_ms);
            let until = spot.until_ms.min(next).min(film.duration_ms());
            if until <= spot.from_ms {
                continue;
            }
            out.push_str(&format!(
                "{number}\n{} --> {}\n{}\n{}\n\n",
                clock(offset + spot.from_ms, ','),
                clock(offset + until, ','),
                spot.title,
                spot.text
            ));
            number += 1;
        }
        offset += film.duration_ms();
    }
    out
}

/// The chapters of `films`, one after the other, as lines "0:00 Title" that video
/// sites turn into chapters when pasted in the description. The first one starts
/// at 0:00, which those sites require, even when the film opens on a title card.
pub fn chapters(films: &[Movie]) -> String {
    let mut out = String::new();
    let mut offset = 0;
    for film in films {
        for chapter in film.chapters() {
            let at = if out.is_empty() { 0 } else { (offset + chapter.at_ms) / 1000 };
            let stamp = if at >= 3600 {
                format!("{}:{:02}:{:02}", at / 3600, at / 60 % 60, at % 60)
            } else {
                format!("{}:{:02}", at / 60, at % 60)
            };
            out.push_str(&format!("{stamp} {}\n", chapter.title));
        }
        offset += film.duration_ms();
    }
    out
}
