//! The videos of the project's channel, recorded from the real apps.
//!
//! ```text
//! cargo run --release -p channel-video -- record OUT
//! FF_AI_MODELS=~/.raven-financas/modelos cargo run --release -p channel-video -- narrate OUT
//! cargo run --release -p channel-video -- export OUT
//! ```
//!
//! `record` writes the films, `narrate` records what their boxes say with the local
//! voice model, and `export` writes the wide and the vertical video, the subtitles,
//! the chapters for the description and a cover. `frames OUT 0:6000` draws
//! single moments (film:ms) as pictures, to look at before exporting.
#![allow(clippy::expect_used, clippy::print_stdout, clippy::print_stderr, reason = "a tool run by hand")]

use std::path::{Path, PathBuf};

use family_finance::db::Database;
use family_finance::ui::App;
use family_finance::update::Updater;
use family_finance::{seed, themes};
use ravenrs::core::Key;
use ravenrs::movie::{Movie, MovieLabels, Narration, OverlayKind, RecordError, Recorder};
use ravenrs::prelude::*;
use ravenrs_movie_video::{Video, VideoFormat, chapters, subtitles};

const SIZE: Size = Size::new(1280.0, 720.0);
const BRAND: &str = "RavenRS · github.com/alexkads/RavenRS";

/// The components, in the gallery.
fn gallery(take: &mut Recorder) -> Result<(), RecordError> {
    take.wait(0.6);
    take.intro(
        "RavenRS",
        "Um framework de interface em Rust. Tudo o que aparece aqui é desenhado pela GPU, com o mesmo código no Mac, no Linux, no Windows e no navegador.",
    );
    take.explain(
        "REVENUE",
        "Um painel só de componentes",
        "Cartões, gráficos e tabela saem prontos da biblioteca. Não há nenhuma imagem: cada traço é desenhado na hora.",
    )?;
    take.click("Data grid")?;
    take.wait(0.8);
    take.explain(
        "100,000 of 100,000 rows",
        "Cem mil linhas sem travar",
        "A grade de dados só desenha o que está na tela. Ela ordena por várias colunas, filtra, agrupa e exporta.",
    )?;
    take.type_into("Search all columns", "Aurora")?;
    take.wait(1.6);
    take.click("Charts")?;
    take.wait(0.8);
    take.explain(
        "Chart family",
        "Gráficos",
        "Área, barra, linha, pizza, radar e radial, todos com a dica que aparece ao passar o mouse.",
    )?;
    take.click("Bar")?;
    take.wait(1.4);
    take.click("Radial")?;
    take.wait(1.4);
    take.click("Ribbon")?;
    take.wait(0.8);
    take.explain(
        "Clipboard",
        "Faixa de comandos",
        "Menu Arquivo, barra de acesso rápido, abas e grupos de botões, como nos programas de escritório.",
    )?;
    take.click("Insert")?;
    take.wait(1.2);
    take.click("Motion")?;
    take.wait(0.8);
    take.explain(
        "Send to the other side",
        "Animação com molas",
        "Clique de novo no meio do caminho e ela volta sem tranco, porque a mola guarda a velocidade.",
    )?;
    take.click("Send to the other side")?;
    take.wait(0.35);
    take.click("Send to the other side")?;
    take.wait(1.4);
    take.explain("Theme", "Temas", "Um tema troca a aparência do aplicativo inteiro de uma vez.")?;
    take.click("Dark")?;
    take.wait(1.6);
    take.click("Glass")?;
    take.wait(1.6);
    take.click("Light")?;
    take.wait(1.0);
    Ok(())
}

/// A real app made with it.
fn finance(take: &mut Recorder) -> Result<(), RecordError> {
    take.wait(0.6);
    take.intro(
        "Um aplicativo de verdade",
        "O Raven Finanças é feito só com o RavenRS: banco de dados local, relatórios, agenda e um assistente que roda na própria máquina.",
    );
    // The voice of the intro is a little longer than its box.
    take.wait(0.8);
    take.explain(
        "SALDO DAS CONTAS",
        "O painel",
        "Saldo, receitas, despesas e o que vence nos próximos trinta dias, num relance.",
    )?;
    take.click("Novo")?;
    take.wait(1.6);
    take.press(Key::Escape);
    take.wait(0.4);
    take.explain(
        "Procurar comando",
        "Paleta de comandos",
        "Qualquer tela fica a um atalho de distância. É só digitar o nome.",
    )?;
    take.click("Procurar comando")?;
    take.wait(0.6);
    take.type_into("Paleta de comandos", "Orça")?;
    take.wait(1.6);
    take.press(Key::Escape);
    take.wait(0.5);
    take.intro(
        "Gravado pelo próprio RavenRS",
        "Este vídeo saiu de um roteiro, rodando o aplicativo de verdade. Dentro do app, o mesmo filme vira um manual interativo e ocupa poucos quilobytes.",
    );
    take.intro(
        "Código aberto",
        "O RavenRS está no GitHub. O endereço está na descrição. Até o próximo vídeo!",
    );
    // Room for the last words before the fade.
    take.wait(1.5);
    Ok(())
}

/// The opening card, fades and the halo around the pointer.
fn edit(film: Movie, title: &str, subtitle: &str) -> Movie {
    let mut project = film.project().clone();
    let total = project.duration_ms();
    project.add_overlay(OverlayKind::FadeOut, total.saturating_sub(800), 800);
    project.add_overlay(OverlayKind::PointerHighlight, 0, total);
    if !title.is_empty() {
        project.insert_title(0, title, subtitle, 2500);
    }
    project.add_overlay(OverlayKind::FadeIn, 0, 600);
    film.with_project(project)
}

fn record(out: &Path) {
    let mut take = Recorder::new(component_gallery::Gallery::new(), Theme::raven_light(), SIZE);
    gallery(&mut take).unwrap_or_else(|e| panic!("the gallery script no longer fits: {e}"));
    let first = edit(take.finish().expect("film"), "RavenRS", "Interfaces em Rust, desenhadas na GPU");
    let today = Date::new(2026, 11, 15).expect("valid date");
    let db = Database::in_memory().expect("database");
    seed::load_demo(&db, today).expect("seed");
    let mut take = Recorder::new(App::new(db, today, None, Updater::idle()), themes::light(), SIZE);
    finance(&mut take).unwrap_or_else(|e| panic!("the finance script no longer fits: {e}"));
    let second = edit(take.finish().expect("film"), "", "");
    for (name, film) in [("1-componentes", &first), ("2-financas", &second)] {
        std::fs::write(out.join(format!("{name}.ravenmovie")), film.to_bytes()).expect("film");
        println!("{name}: {:.1}s, {} boxes", film.duration_ms() as f32 / 1000.0, film.hotspots().len());
    }
}

fn films(out: &Path) -> Vec<Movie> {
    ["1-componentes", "2-financas"]
        .iter()
        .map(|name| {
            let bytes = std::fs::read(out.join(format!("{name}.ravenmovie"))).expect("run record first");
            Movie::from_bytes(&bytes).expect("a film")
        })
        .collect()
}

fn narrate(out: &Path) {
    use ravenrs_ai::{ModelFolder, SPEECH_RATE, SpeechEngine, SpeechSettings, model_by_id, sentences};
    let mut said: Vec<String> = films(out)
        .iter()
        .flat_map(|film| {
            film.hotspots().iter().map(|s| format!("{}. {}", s.title, s.text)).collect::<Vec<_>>()
        })
        .collect();
    said.dedup();
    let root = std::env::var("FF_AI_MODELS").expect("FF_AI_MODELS: the folder of the models");
    let model = model_by_id("lfm3-audio-2b-q4").expect("known");
    let mut engine = SpeechEngine::load(&ModelFolder::new(root), &model).expect("the voice model");
    let settings = SpeechSettings::default();
    let silence = vec![0.0_f32; (SPEECH_RATE / 4) as usize];
    let wavs = out.join("falas");
    std::fs::create_dir_all(&wavs).expect("folder");
    let mut recordings = Vec::new();
    for (index, text) in said.iter().enumerate() {
        let mut sound = Vec::new();
        for sentence in sentences(&family_finance::ui::spoken(text)) {
            sound.extend(engine.speak(&sentence, &settings, &mut |_| true).expect("sound"));
            sound.extend_from_slice(&silence);
        }
        let allowed = 0.8 + text.chars().count() as f32 * 0.075;
        println!("{:.1}s of {allowed:.1}s | {text}", sound.len() as f32 / SPEECH_RATE as f32);
        std::fs::write(wavs.join(format!("{index:02}.wav")), wav_of(&sound, SPEECH_RATE)).expect("wav");
        std::fs::write(wavs.join(format!("{index:02}.txt")), text).expect("text");
        recordings.push((text.clone(), sound));
    }
    std::fs::write(out.join("narracao.voz"), Narration::pack(&recordings, SPEECH_RATE)).expect("narration");
}

fn wav_of(sound: &[f32], rate: u32) -> Vec<u8> {
    let mut wav = Vec::with_capacity(44 + sound.len() * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + sound.len() as u32 * 2).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    for value in [1_u16, 1] {
        wav.extend_from_slice(&value.to_le_bytes());
    }
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes());
    for value in [2_u16, 16] {
        wav.extend_from_slice(&value.to_le_bytes());
    }
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(sound.len() as u32 * 2).to_le_bytes());
    for sample in sound {
        wav.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    wav
}

fn export(out: &Path, only: Option<&str>) {
    let films = films(out);
    let narration = std::fs::read(out.join("narracao.voz"))
        .ok()
        .map(|bytes| Narration::new(Box::leak(bytes.into_boxed_slice())));
    if narration.is_none() {
        eprintln!("no narracao.voz: the video goes without the voice");
    }
    let make = |format: VideoFormat, films: &[Movie]| {
        let mut video = Video::new(films[0].clone(), format).labels(MovieLabels::pt_br()).brand(BRAND);
        for film in &films[1..] {
            video = video.then(film.clone());
        }
        match &narration {
            Some(narration) => video.narration(narration.clone()),
            None => video,
        }
    };
    let started = std::time::Instant::now();
    let write = |name: &str, video: Video| {
        let report = video
            .write(out.join(name), |done, all| {
                if done % 300 == 0 || done == all {
                    eprintln!("{name}: {done}/{all}");
                }
            })
            .expect("the video");
        println!(
            "{name}: {:.1}s, {:.1} MB, {:.0}s to make{}",
            report.duration_ms as f32 / 1000.0,
            report.bytes as f32 / 1e6,
            started.elapsed().as_secs_f32(),
            if report.too_long { ", too long for the format" } else { "" }
        );
        for text in report.unrecorded {
            println!("  not said: {text}");
        }
    };
    if only != Some("vertical") {
        let wide = make(VideoFormat::WIDE, &films);
        std::fs::write(out.join("capa.png"), wide.thumbnail(1_200).expect("cover")).expect("cover");
        write("ravenrs-youtube.mov", wide);
        std::fs::write(out.join("ravenrs-youtube.srt"), subtitles(&films)).expect("srt");
        let description = format!(
            "RavenRS é um framework de interface em Rust: componentes desenhados pela GPU, o mesmo código no Mac, no Linux, no Windows e no navegador.\n\n\
             Neste vídeo: a galeria de componentes (painel, grade de dados com cem mil linhas, gráficos, faixa de comandos, animações e temas) e o Raven Finanças, um aplicativo completo feito só com o RavenRS.\n\n\
             O vídeo foi gravado pelo próprio RavenRS, a partir de um roteiro, com o crate ravenrs-movie-video.\n\n\
             Código: https://github.com/alexkads/RavenRS\n\n{}",
            chapters(&films)
        );
        std::fs::write(out.join("descricao.txt"), description).expect("description");
    }
    if only != Some("wide") {
        let followed: Vec<Movie> = films.to_vec();
        write("ravenrs-vertical.mov", make(VideoFormat::VERTICAL, &followed));
    }
}

/// Pictures of moments of the first film, wide and vertical, to look at.
fn frames(out: &Path, moments: &[String]) {
    let films = films(out);
    for (film, at) in moments.iter().filter_map(|m| m.split_once(':')) {
        let movie = &films[film.parse::<usize>().expect("film")];
        let at: u64 = at.parse().expect("ms");
        for (name, format, movie) in
            [("wide", VideoFormat::WIDE, movie.clone()), ("vertical", VideoFormat::VERTICAL, movie.clone())]
        {
            let video = Video::new(movie, format).labels(MovieLabels::pt_br()).brand(BRAND);
            std::fs::write(
                out.join(format!("q-{film}-{at}-{name}.png")),
                video.thumbnail(at).expect("picture"),
            )
            .expect("png");
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(args.get(2).expect("usage: channel-video record|narrate|export OUT"));
    std::fs::create_dir_all(&out).expect("folder");
    match args[1].as_str() {
        "record" => record(&out),
        "narrate" => narrate(&out),
        "export" => export(&out, args.get(3).map(String::as_str)),
        "frames" => frames(&out, &args[3..]),
        other => panic!("unknown step {other}"),
    }
}
