//! `Timeline`, every feature: the cut of a video on named tracks under a ruler with markers and
//! a playhead; blocks that are selected, moved along their track and trimmed by their ends, some
//! fixed; a thumbnail before a label; and a ruler that zooms. The app holds the cut and applies
//! what the timeline reports, written on a line.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "TimelineDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Linha do tempo";

/// The tracks, top to bottom.
pub const TRACKS: [&str; 3] = ["Vídeo", "Áudio", "Legendas"];

/// A block of the cut: its id, its track, where it starts, how long it is and what it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    /// Given back by the timeline.
    pub id: u64,
    /// Its track, in [`TRACKS`].
    pub track: usize,
    /// Where it starts.
    pub start_ms: u64,
    /// How long it is.
    pub duration_ms: u64,
    /// What it is.
    pub label: &'static str,
}

/// The cut the page starts with.
pub fn cut() -> Vec<Clip> {
    let clip = |id, track, start_ms, duration_ms, label| Clip { id, track, start_ms, duration_ms, label };
    vec![
        clip(1, 0, 0, 8_000, "Abertura"),
        clip(2, 0, 8_000, 12_000, "Entrevista"),
        clip(3, 0, 20_000, 6_000, "Encerramento"),
        clip(4, 1, 0, 26_000, "Trilha sonora"),
        clip(5, 1, 9_000, 3_000, "Aplausos"),
        clip(6, 2, 2_000, 4_000, "Título"),
        clip(7, 2, 10_000, 5_000, "Nome da entrevistada"),
    ]
}

fn seconds(ms: u64) -> String {
    format!("{}:{:02}", ms / 60_000, ms / 1_000 % 60)
}

/// The page.
#[derive(Debug, Default)]
pub struct TimelinePage;

impl View for TimelinePage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let clips = cx.use_state(cut);
        let position = cx.use_state(|| 4_000_u64);
        let selected = cx.use_state(|| None::<u64>);
        let pixels = cx.use_state(|| 28.0_f32);
        let said = cx.use_state(|| "Escolha um bloco, arraste-o ou mova a agulha pela régua.".to_string());

        let all = clips.get();
        let chosen = selected.get();
        let tracks: Vec<TimelineTrack> = TRACKS
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let items = all
                    .iter()
                    .filter(|clip| clip.track == index)
                    .map(|clip| {
                        let mut item =
                            TimelineItem::new(clip.id, clip.start_ms, clip.duration_ms, clip.label)
                                .tone(match index {
                                    0 => Tone::Accent,
                                    1 => Tone::Success,
                                    _ => Tone::Warning,
                                })
                                .selected(chosen == Some(clip.id));
                        // The soundtrack is fixed under the cut; everything else moves and trims.
                        if clip.id != 4 {
                            item = item.movable().trimmable();
                        }
                        if index == 0 {
                            item = item.leading(Icon::new(IconName::Video).size(14.0));
                        }
                        item
                    })
                    .collect();
                TimelineTrack::new(*name, if index == 0 { 56.0 } else { 36.0 }, items)
            })
            .collect();
        let label_of = {
            let all = all.clone();
            move |id: u64| all.iter().find(|c| c.id == id).map_or("", |c| c.label)
        };
        let (seek, seek_line) = (position.clone(), said.clone());
        let (pick, pick_line, pick_label) = (selected.clone(), said.clone(), label_of.clone());
        let (edits, edit_line, edit_label) = (clips.clone(), said.clone(), label_of.clone());
        let timeline = Timeline::new(tracks)
            .markers(vec![
                TimelineMarker { at_ms: 8_000, title: "Corte 1".into() },
                TimelineMarker { at_ms: 20_000, title: "Corte 2".into() },
            ])
            .position_ms(position.get())
            .pixels_per_second(pixels.get())
            .heads_width(if narrow { 80.0 } else { 112.0 })
            .on_seek(move |ms| {
                seek.set(ms);
                seek_line.set(format!("Agulha em {}", seconds(ms)));
            })
            .on_select(move |id| {
                pick.set(Some(id));
                pick_line.set(format!("Escolheu {}", pick_label(id)));
            })
            .on_edit(move |edit| {
                let text = match edit {
                    TimelineEdit::Began { id } => format!("Começou a mexer em {}", edit_label(id)),
                    TimelineEdit::Moved { id, start_ms } => {
                        edits.update(|all| {
                            if let Some(clip) = all.iter_mut().find(|c| c.id == id) {
                                clip.start_ms = start_ms;
                            }
                        });
                        format!("{} agora começa em {}", edit_label(id), seconds(start_ms))
                    }
                    TimelineEdit::TrimmedStart { id, by_ms } => {
                        edits.update(|all| {
                            if let Some(clip) = all.iter_mut().find(|c| c.id == id) {
                                clip.start_ms = clip.start_ms.saturating_add_signed(by_ms);
                                clip.duration_ms = clip.duration_ms.saturating_add_signed(-by_ms).max(500);
                            }
                        });
                        format!("{} aparado no começo em {by_ms} ms", edit_label(id))
                    }
                    TimelineEdit::TrimmedEnd { id, by_ms } => {
                        edits.update(|all| {
                            if let Some(clip) = all.iter_mut().find(|c| c.id == id) {
                                clip.duration_ms = clip.duration_ms.saturating_add_signed(by_ms).max(500);
                            }
                        });
                        format!("{} aparado no fim em {by_ms} ms", edit_label(id))
                    }
                    _ => "Outra edição".to_string(),
                };
                edit_line.set(text);
            });
        let height = timeline.total_height();
        let (ahead, back, nearer, farther, reset) =
            (position.clone(), position.clone(), pixels.clone(), pixels.clone(), clips.clone());
        let (back_line, ahead_line) = (said.clone(), said.clone());
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                HStack::new((
                    Button::new("Voltar ao início").style(ButtonStyle::Secondary).on_click(move || {
                        back.set(0);
                        back_line.set("Agulha em 0:00".into());
                    }),
                    Button::new("Avançar 5 s").on_click(move || {
                        let to = ahead.get() + 5_000;
                        ahead.set(to);
                        ahead_line.set(format!("Agulha em {}", seconds(to)));
                    }),
                    Button::new("Aproximar a régua")
                        .style(ButtonStyle::Secondary)
                        .on_click(move || nearer.update(|p| *p = (*p * 1.5).min(120.0))),
                    Button::new("Afastar a régua")
                        .style(ButtonStyle::Secondary)
                        .on_click(move || farther.update(|p| *p = (*p / 1.5).max(8.0))),
                    Button::new("Refazer o corte")
                        .style(ButtonStyle::Ghost)
                        .on_click(move || reset.set(cut())),
                ))
                .spacing(8.0)
                .wrap(),
                timeline,
                Text::new(format!(
                    "Agulha em {} · {:.0} px por segundo · {height:.0} px de altura",
                    seconds(position.get()),
                    pixels.get()
                )),
                Text::new(chosen.and_then(|id| all.iter().find(|c| c.id == id)).map_or_else(
                    || "Escolhido: nenhum".to_string(),
                    |clip| {
                        format!(
                            "Escolhido: {}, de {} a {}",
                            clip.label,
                            seconds(clip.start_ms),
                            seconds(clip.start_ms + clip.duration_ms)
                        )
                    },
                )),
                Text::new("A trilha sonora fica fixa; os outros blocos se movem e se aparam pelas pontas.")
                    .style(TextStyle::Caption)
                    .foreground(Semantic::InkMuted),
                Text::new("Eventos").style(TextStyle::Headline),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
