//! `ModelManager`: offers what fits, downloads, and removes.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use ravenrs_ai::{LocalModel, Machine, ModelFamily, ModelFile, ModelFolder, ModelManager, ModelState};
use ravenrs_core::{AccessAction, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;
use sha2::{Digest, Sha256};

const GB: u64 = 1_000_000_000;
const ROOMY: Machine = Machine { memory: 16 * GB, free_disk: 100 * GB };

fn serve(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("port");
    let port = listener.local_addr().expect("address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let head =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    format!("http://127.0.0.1:{port}/model")
}

fn model(name: &'static str, url: String, bytes: u64, hash: String) -> LocalModel {
    let file = ModelFile {
        name: "model.gguf",
        url: Box::leak(url.into_boxed_str()),
        bytes,
        sha256: Box::leak(hash.into_boxed_str()),
    };
    LocalModel {
        id: name,
        name,
        license: "MIT",
        family: ModelFamily::Qwen3,
        memory: bytes + GB,
        understood: None,
        files: Box::leak(Box::new([file])),
    }
}

fn served() -> LocalModel {
    let body = vec![7_u8; 200_000];
    let hash = Sha256::digest(&body).iter().map(|byte| format!("{byte:02x}")).collect();
    model("Served", serve(body), 200_000, hash)
}

fn ui_of(manager: ModelManager) -> Ui {
    let mut ui = Ui::new(manager, Theme::raven_light());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

fn says(ui: &mut Ui, wanted: &str) -> bool {
    let starts = |text: &Option<String>| text.as_deref().is_some_and(|text| text.starts_with(wanted));
    ui.access_tree().nodes.iter().any(|n| starts(&n.name) || starts(&n.value))
}

fn open_the_list(ui: &mut Ui) {
    let tree = ui.access_tree();
    let list = tree.nodes.iter().find(|n| n.role == Role::ComboBox).expect("the list").id;
    ui.perform_action(list, AccessAction::Click);
    ui.update();
}

fn click(ui: &mut Ui, name: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no button {name}"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
}

fn click_option(ui: &mut Ui, name: &str) {
    let tree = ui.access_tree();
    let id = tree
        .nodes
        .iter()
        .find(|n| n.role != Role::ComboBox && n.name.as_deref().is_some_and(|text| text.starts_with(name)))
        .unwrap_or_else(|| panic!("no {name}"))
        .id;
    ui.perform_action(id, AccessAction::Click);
    ui.update();
}

/// Lets the download thread run, updating as frames would, until `wanted` shows.
fn wait_for(ui: &mut Ui, wanted: &str) {
    let started = Instant::now();
    while !says(ui, wanted) {
        assert!(started.elapsed() < Duration::from_secs(10), "{wanted} never showed");
        std::thread::sleep(Duration::from_millis(5));
        ui.update();
    }
}

#[test]
fn it_offers_only_what_fits_this_machine() {
    let dir = tempfile::tempdir().expect("folder");
    let catalog = [
        model("Small", "http://127.0.0.1:9/".into(), GB, "0".repeat(64)),
        model("Huge", "http://127.0.0.1:9/".into(), 40 * GB, "0".repeat(64)),
    ];
    let manager = ModelManager::for_machine(&catalog, ModelFolder::new(dir.path()), &ROOMY);
    let mut ui = ui_of(manager);
    assert!(says(&mut ui, "Small"));
    open_the_list(&mut ui);
    assert!(!says(&mut ui, "Huge"), "it would not fit in memory");
}

#[test]
fn with_nothing_that_fits_it_says_so() {
    let dir = tempfile::tempdir().expect("folder");
    let catalog = [model("Huge", "http://127.0.0.1:9/".into(), 40 * GB, "0".repeat(64))];
    let mut ui = ui_of(ModelManager::for_machine(&catalog, ModelFolder::new(dir.path()), &ROOMY));
    assert!(says(&mut ui, "No model fits"));
}

#[test]
fn it_downloads_then_offers_to_remove() {
    let dir = tempfile::tempdir().expect("folder");
    let test = served();
    let folder = ModelFolder::new(dir.path());
    let mut ui = ui_of(ModelManager::for_machine(&[test], folder.clone(), &ROOMY));

    click(&mut ui, "Download");
    wait_for(&mut ui, "Remove");
    assert_eq!(folder.state(&test), ModelState::Installed);

    click(&mut ui, "Remove");
    assert!(says(&mut ui, "Download"), "it can come back");
    assert_eq!(folder.state(&test), ModelState::Absent);
}

#[test]
fn an_installed_model_is_recognized_on_opening() {
    let dir = tempfile::tempdir().expect("folder");
    let test = served();
    let folder = ModelFolder::new(dir.path());
    let never = std::sync::atomic::AtomicBool::new(false);
    folder.download(&test, &|_, _| {}, &never).expect("download");
    let mut ui = ui_of(ModelManager::for_machine(&[test], folder, &ROOMY));
    assert!(says(&mut ui, "Remove"));
}

#[test]
fn a_failed_download_says_so_and_can_be_tried_again() {
    let dir = tempfile::tempdir().expect("folder");
    let body = vec![7_u8; 1000];
    let wrong = model("Wrong", serve(body), 1000, "0".repeat(64));
    let mut ui = ui_of(ModelManager::for_machine(&[wrong], ModelFolder::new(dir.path()), &ROOMY));
    click(&mut ui, "Download");
    wait_for(&mut ui, "What arrived is not");
    assert!(says(&mut ui, "Download"));
}

#[test]
fn choosing_another_model_shows_its_size_and_it_opens_on_the_one_already_here() {
    let dir = tempfile::tempdir().expect("folder");
    let folder = ModelFolder::new(dir.path());
    let here = served();
    let never = std::sync::atomic::AtomicBool::new(false);
    folder.download(&here, &|_, _| {}, &never).expect("download");
    let other = model("Other", "http://127.0.0.1:9/".into(), GB, "0".repeat(64));
    let mut ui = ui_of(ModelManager::for_machine(&[other, here], folder, &ROOMY));
    assert!(says(&mut ui, "Remove"), "it opened on the one that is here");
    open_the_list(&mut ui);
    click_option(&mut ui, "Other");
    assert!(says(&mut ui, "Download"), "the other can be downloaded");
    assert!(says(&mut ui, "1.0 GB"));
}

#[test]
fn on_a_machine_with_16_gb_the_largest_is_not_offered() {
    let dir = tempfile::tempdir().expect("folder");
    let manager = ModelManager::for_machine(ravenrs_ai::CATALOG, ModelFolder::new(dir.path()), &ROOMY);
    let mut ui = ui_of(manager);
    open_the_list(&mut ui);
    let families = ["Qwen3 0.6B", "Qwen 2.5 0.5B", "Llama 3.2 1B", "Phi-3 Mini", "Qwen3 4B"];
    for name in families {
        assert!(says(&mut ui, name), "{name}");
    }
    assert!(!says(&mut ui, "Qwen3 8B"), "it would not fit");
    assert!(!says(&mut ui, "Mistral 7B"), "it would not fit");
}

#[test]
fn the_catalog_is_pinned_and_consistent() {
    assert!(ravenrs_ai::CATALOG.windows(2).all(|pair| pair[0].bytes() <= pair[1].bytes()), "smallest first");
    for model in ravenrs_ai::CATALOG {
        assert!(!model.license.is_empty());
        for file in model.files {
            assert!(file.url.starts_with("https://"), "{}", file.url);
            assert_eq!(file.sha256.len(), 64, "{}", file.name);
            assert!(file.bytes > 0);
            assert!(!file.url.contains("/main/"), "pinned to a commit: {}", file.url);
            assert!(file.url.contains("/resolve/"), "{}", file.url);
        }
    }
}

#[test]
fn a_model_in_use_is_remembered_and_removing_it_stops_using_it() {
    let dir = tempfile::tempdir().expect("folder");
    let test = served();
    let folder = ModelFolder::new(dir.path());
    let never = std::sync::atomic::AtomicBool::new(false);
    folder.download(&test, &|_, _| {}, &never).expect("download");

    let manager = ModelManager::for_machine(&[test], folder.clone(), &ROOMY);
    assert!(manager.assistant().is_none(), "nothing is in use until someone says so");
    let mut ui = ui_of(manager.clone());
    click(&mut ui, "Use");
    assert!(says(&mut ui, "Served · In use"));
    assert!(manager.assistant().is_some());

    let again = ModelManager::for_machine(&[test], folder.clone(), &ROOMY);
    assert!(again.assistant().is_some(), "and it is still in use next time");

    click(&mut ui, "Remove");
    assert!(manager.assistant().is_none());
    assert!(ModelManager::for_machine(&[test], folder, &ROOMY).assistant().is_none());
}

#[test]
fn a_model_that_does_not_run_says_so() {
    let dir = tempfile::tempdir().expect("folder");
    let test = served();
    let folder = ModelFolder::new(dir.path());
    let never = std::sync::atomic::AtomicBool::new(false);
    folder.download(&test, &|_, _| {}, &never).expect("download");
    let mut ui = ui_of(ModelManager::for_machine(&[test], folder, &ROOMY));
    click(&mut ui, "Test speed");
    wait_for(&mut ui, "This model did not run");
    assert!(says(&mut ui, "Remove"), "and it can go");
}

/// Runs only where the real model is, as it is more than a gigabyte:
/// `RAVENRS_AI_MODELS=folder cargo test --release -p ravenrs-ai --test manager real -- --ignored`.
#[test]
#[ignore = "needs the real model"]
fn the_real_model_tests_and_answers() {
    let root = std::env::var("RAVENRS_AI_MODELS").expect("RAVENRS_AI_MODELS");
    // `RAVENRS_AI_MODEL` names which one, by its id.
    let wanted = std::env::var("RAVENRS_AI_MODEL").unwrap_or_else(|_| "qwen3-1.7b-q4".to_owned());
    let mut ui =
        ui_of(ModelManager::for_machine(ravenrs_ai::CATALOG, ModelFolder::new(root.clone()), &ROOMY));
    click(&mut ui, "Test speed");
    let started = Instant::now();
    while !says(&mut ui, "Answered the test in") {
        assert!(started.elapsed() < Duration::from_secs(120), "no result");
        std::thread::sleep(Duration::from_millis(50));
        ui.update();
    }
    let names: Vec<_> = ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect();
    eprintln!("{names:?}");

    let assistant = ravenrs_ai::LocalAssistant::new(
        ModelFolder::new(root),
        ravenrs_ai::model_by_id(&wanted).expect("known"),
    );
    let text = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    assistant.write(
        "Responda em português, em uma frase.".into(),
        "Para que serve um backup?".into(),
        text.clone(),
        ui.ui_waker(),
        "FALHOU".into(),
    );
    let started = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(300));
        let written = text.lock().expect("text").clone();
        if written.ends_with('.') || started.elapsed() > Duration::from_secs(60) {
            eprintln!("{written}");
            assert!(!written.is_empty() && written != "FALHOU", "{written}");
            break;
        }
    }
}

/// Runs only where the real voice model is:
/// `RAVENRS_AI_MODELS=folder cargo test --release -p ravenrs-ai --test manager real_speech -- --ignored --nocapture`.
#[test]
#[ignore = "needs the real model"]
fn real_speech_is_written_to_a_file() {
    use ravenrs_ai::{ModelFolder, SpeechEngine, SpeechSettings, SpeechVoice};
    let root = std::env::var("RAVENRS_AI_MODELS").expect("RAVENRS_AI_MODELS");
    let model = ravenrs_ai::model_by_id("lfm3-audio-2b-q4").expect("known");
    let started = Instant::now();
    let mut engine = SpeechEngine::load(&ModelFolder::new(root.clone()), &model).expect("the model");
    eprintln!("loaded in {:.1}s", started.elapsed().as_secs_f32());
    let cases = [
        ("Você tem cinquenta reais de saldo nas contas.", SpeechVoice::Female),
        ("Para pagar uma conta, clique em Pagar e confira o valor.", SpeechVoice::Male),
        (
            "Você tem mil duzentos e trinta e quatro reais e cinquenta e seis centavos nas contas. A conta vence em quinze de novembro.",
            SpeechVoice::Female,
        ),
    ];
    for (index, (text, voice)) in cases.iter().enumerate() {
        let settings = SpeechSettings { voice: *voice, ..SpeechSettings::default() };
        let started = Instant::now();
        let sound = engine.speak(text, &settings, &mut |_| true).expect("sound");
        let seconds = sound.len() as f32 / ravenrs_ai::SPEECH_RATE as f32;
        let took = started.elapsed().as_secs_f32();
        eprintln!("{text:?}: {seconds:.1}s of sound in {took:.1}s, {:.2}x real time", seconds / took);
        let peak = sound.iter().fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        eprintln!("peak {peak:.3}");
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + sound.len() as u32 * 2).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&ravenrs_ai::SPEECH_RATE.to_le_bytes());
        wav.extend_from_slice(&(ravenrs_ai::SPEECH_RATE * 2).to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(sound.len() as u32 * 2).to_le_bytes());
        for sample in &sound {
            wav.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        std::fs::write(format!("{root}/fala-{index}.wav"), wav).expect("file");
        assert!(seconds > 0.5 && peak > 0.01 && peak.is_finite(), "no sound");
    }
}

/// A folder with a model already there and in use, as after downloading it.
fn voice_in_use(dir: &tempfile::TempDir) -> ravenrs_ai::VoiceManager {
    let installed = served();
    let folder = dir.path().join(installed.id);
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("model.gguf"), vec![7_u8; 200_000]).expect("file");
    std::fs::write(dir.path().join("in-use"), installed.id).expect("in use");
    let catalog: &'static [LocalModel] = Box::leak(Box::new([installed]));
    ravenrs_ai::VoiceManager::for_machine(catalog, dir.path(), &ROOMY)
}

fn ui_of_voice(manager: ravenrs_ai::VoiceManager) -> Ui {
    let mut ui = Ui::new(manager, Theme::raven_light());
    ui.set_viewport(Size::new(800.0, 900.0), 1.0);
    ui.update();
    ui
}

#[test]
fn the_voice_settings_show_only_with_a_voice_in_use() {
    let dir = tempfile::tempdir().expect("folder");
    let nothing: &'static [LocalModel] = Box::leak(Box::new([served()]));
    let mut ui = ui_of_voice(ravenrs_ai::VoiceManager::for_machine(nothing, dir.path(), &ROOMY));
    assert!(!says(&mut ui, "Hear a sample"));

    let dir = tempfile::tempdir().expect("folder");
    let mut ui = ui_of_voice(voice_in_use(&dir));
    assert!(says(&mut ui, "Hear a sample"));
    assert!(says(&mut ui, "Read answers aloud"));
    click(&mut ui, "Fine settings");
    assert!(says(&mut ui, "Temperature"));
    assert!(says(&mut ui, "Use the recommended values"));
}

#[test]
fn the_voice_settings_are_kept_and_can_go_back_to_the_recommended() {
    let dir = tempfile::tempdir().expect("folder");
    let manager = voice_in_use(&dir);
    assert_eq!(manager.settings(), ravenrs_ai::SpeechSettings::default());
    let mut ui = ui_of_voice(manager.clone());
    click_option(&mut ui, "Male");
    click(&mut ui, "Fine settings");
    click(&mut ui, "Steadier");
    assert_eq!(manager.settings().voice, ravenrs_ai::SpeechVoice::Male);
    assert_eq!(manager.settings().temperature, 0.4);
    assert_eq!(manager.settings().repetition_window, 64);
    ui.update();

    let again = voice_in_use(&dir);
    assert_eq!(again.settings().voice, ravenrs_ai::SpeechVoice::Male, "kept in the folder");
    assert_eq!(again.settings().temperature, 0.4);

    click(&mut ui, "Use the recommended values");
    assert_eq!(manager.settings().temperature, 1.0);
    assert_eq!(manager.settings().top_k, 128);
}

/// Runs only where the real voice model is, and hears what a chat would play:
/// the same command as above, with `real_voice`.
#[test]
#[ignore = "needs the real model"]
fn real_voice_plays_sentence_after_sentence() {
    use std::cell::RefCell;
    use std::rc::Rc;
    struct Heard(Rc<RefCell<Vec<f32>>>);
    impl ravenrs_widgets::SoundProvider for Heard {
        fn play(&self, clip: &ravenrs_widgets::SoundClip) {
            self.0.borrow_mut().push(clip.seconds());
        }
        fn speak(&self, _text: &str, _language: &str) {}
        fn stop_speaking(&self) {}
    }
    let root = std::env::var("RAVENRS_AI_MODELS").expect("RAVENRS_AI_MODELS");
    let model = ravenrs_ai::model_by_id("lfm3-audio-2b-q4").expect("known");
    let voice = ravenrs_ai::LocalVoice::new(ModelFolder::new(root), model);
    let heard = Rc::new(RefCell::new(Vec::new()));
    let sound = ravenrs_widgets::Sound::new(Heard(heard.clone()));
    let started = Instant::now();
    voice.say(
        "Você tem mil e duzentos reais nas contas. Quer ver os gastos do mês?".into(),
        ravenrs_ai::SpeechSettings::default(),
        ravenrs_widgets::UiWaker::default(),
    );
    while heard.borrow().len() < 2 {
        assert!(started.elapsed() < Duration::from_secs(60), "no sound");
        std::thread::sleep(Duration::from_millis(50));
        voice.play_ready(&sound);
        if heard.borrow().len() == 1 {
            eprintln!("first sentence after {:.1}s", started.elapsed().as_secs_f32());
        }
    }
    eprintln!("{:?}", heard.borrow());
}
