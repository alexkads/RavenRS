//! `RichText`, `AttachmentStrip`, `AudioPlayerButton`, `SelectionBar` and media in `Chat`.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{Event, ImageData, Key, Modifiers, Point, PointerButton, Primitive, Rect, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn running(root: impl View + 'static, width: f32) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_dark());
    ui.set_viewport(Size::new(width, 700.0), 1.0);
    settle(&mut ui);
    ui
}

fn settle(ui: &mut Ui) {
    for _ in 0..4 {
        ui.update();
    }
}

fn press(ui: &mut Ui, key: Key) {
    ui.handle_event(&Event::KeyDown { key, modifiers: Modifiers::default() });
    settle(ui);
}

fn click_at(ui: &mut Ui, at: Point) {
    ui.handle_event(&Event::PointerMove { position: at });
    ui.update();
    ui.handle_event(&Event::PointerDown { position: at, button: PointerButton::Primary });
    ui.handle_event(&Event::PointerUp { position: at, button: PointerButton::Primary });
    settle(ui);
}

fn node(ui: &mut Ui, role: Role, name: &str) -> (u64, Rect) {
    let tree = ui.access_tree();
    let found = tree
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {role:?} called {name}: {:#?}", names(&tree)));
    (found.id, found.bounds)
}

fn names(tree: &ravenrs_core::AccessTree) -> Vec<(Role, String)> {
    tree.nodes.iter().filter_map(|n| n.name.clone().map(|name| (n.role, name))).collect()
}

fn has(ui: &mut Ui, role: Role, name: &str) -> bool {
    ui.access_tree().nodes.iter().any(|n| n.role == role && n.name.as_deref() == Some(name))
}

fn click(ui: &mut Ui, role: Role, name: &str) {
    let (_, bounds) = node(ui, role, name);
    click_at(ui, Point::new(bounds.x() + bounds.width() / 2.0, bounds.y() + bounds.height() / 2.0));
}

fn tab_to(ui: &mut Ui, name: &str) {
    for _ in 0..40 {
        press(ui, Key::Tab);
        if ui.access_tree().focused().and_then(|n| n.name.clone()).as_deref() == Some(name) {
            return;
        }
    }
    panic!("no focus stop called {name}");
}

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> (Log, Log) {
    let log = Log::default();
    (log.clone(), log)
}

fn glyph_runs(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count()
}

mod rich_text {
    use super::*;

    #[test]
    fn plain_text_costs_what_a_text_costs() {
        let words = "Nothing to format here,\nnot even on the second line";
        let (plain, text) = (running(RichText::new(words), 400.0), running(Text::new(words), 400.0));
        assert_eq!(plain.element_count(), text.element_count());
        assert_eq!(glyph_runs(&plain), glyph_runs(&text));
    }

    #[test]
    fn a_screen_reader_gets_the_sentence_without_marks_and_not_word_by_word() {
        let mut ui = running(RichText::new("*Ready* to _go_ `now`"), 400.0);
        assert!(has(&mut ui, Role::Label, "Ready to go now"));
        assert!(!has(&mut ui, Role::Label, "Ready"), "{:?}", names(&ui.access_tree()));
    }

    #[test]
    fn a_link_opens_by_click_enter_and_space() {
        let (opened, seen) = log();
        let address = "https://example.com/a";
        let text = RichText::new(format!("*See* {address}.")).on_link(move |a| seen.borrow_mut().push(a));
        let mut ui = running(text, 500.0);
        click(&mut ui, Role::Link, address);
        assert_eq!(*opened.borrow(), [address]);
        tab_to(&mut ui, address);
        press(&mut ui, Key::Enter);
        press(&mut ui, Key::Space);
        assert_eq!(opened.borrow().len(), 3);
    }

    #[test]
    fn without_a_handler_a_link_is_not_a_control() {
        let mut ui = running(RichText::new("see https://example.com"), 500.0);
        assert!(!ui.access_tree().nodes.iter().any(|n| n.role == Role::Link));
    }

    #[test]
    fn two_links_in_the_same_text_are_two_stops() {
        let (opened, seen) = log();
        let text =
            RichText::new("https://a.io and\n* https://b.io").on_link(move |a| seen.borrow_mut().push(a));
        let mut ui = running(text, 500.0);
        tab_to(&mut ui, "https://a.io");
        press(&mut ui, Key::Tab);
        press(&mut ui, Key::Enter);
        assert_eq!(*opened.borrow(), ["https://b.io"]);
    }

    #[test]
    fn it_wraps_to_the_width_it_is_offered() {
        let words = "*Bold words* and many more plain words that have to go on to other lines ".repeat(3);
        let height = |width: f32| {
            let mut ui = running(RichText::new(words.clone()), width);
            let tree = ui.access_tree();
            tree.nodes.iter().find(|n| n.role == Role::Label).expect("the text").bounds
        };
        let (wide, narrow) = (height(900.0), height(220.0));
        assert!(narrow.height() > wide.height() * 2.0, "{wide:?} {narrow:?}");
        assert!(narrow.width() <= 220.0, "{narrow:?}");
    }

    #[test]
    fn line_breaks_lists_and_quotes_each_take_a_line() {
        let bounds = |text: &str| {
            let mut ui = running(RichText::new(text), 500.0);
            let tree = ui.access_tree();
            tree.nodes.iter().find(|n| n.role == Role::Label).expect("the text").bounds
        };
        let one = bounds("*a*").height();
        assert_eq!(bounds("*a*\nb\n\n* c\n> d").height(), one * 5.0);
    }

    #[test]
    fn links_only_shows_the_marks() {
        let mut ui = running(RichText::new("*a* https://a.io").links_only().on_link(|_| {}), 500.0);
        assert!(has(&mut ui, Role::Label, "*a* https://a.io"));
        assert!(has(&mut ui, Role::Link, "https://a.io"));
    }
}

mod attachment_strip {
    use super::*;

    #[test]
    fn it_says_what_each_file_is_and_a_button_takes_it_out() {
        let (removed, told) = log();
        let strip = AttachmentStrip::new([
            Attachment::new("report.pdf").size(1536),
            Attachment::new("photo.jpg").icon(IconName::Image),
        ])
        .on_remove(move |i| told.borrow_mut().push(i.to_string()));
        let mut ui = running(strip, 600.0);
        assert!(has(&mut ui, Role::ListItem, "report.pdf, 1.5 KB"));
        assert!(has(&mut ui, Role::ListItem, "photo.jpg"));
        click(&mut ui, Role::Button, "Remove photo.jpg");
        assert_eq!(*removed.borrow(), ["1"]);
        tab_to(&mut ui, "Remove report.pdf");
        press(&mut ui, Key::Enter);
        assert_eq!(*removed.borrow(), ["1", "0"]);
    }

    #[test]
    fn no_handler_no_button() {
        let mut ui = running(AttachmentStrip::new([Attachment::new("a.txt")]), 600.0);
        assert!(!ui.access_tree().nodes.iter().any(|n| n.role == Role::Button));
    }

    #[test]
    fn it_asks_how_to_send_only_when_there_is_a_choice() {
        let (chosen, told) = log();
        let strip = AttachmentStrip::new([
            Attachment::new("one.pdf").send_as(["document"], 0, |_| {}),
            Attachment::new("two.jpg")
                .send_as(["photo", "document"], 0, move |i| told.borrow_mut().push(i.to_string())),
        ]);
        let mut ui = running(strip, 600.0);
        assert!(!has(&mut ui, Role::Button, "Send as document"));
        click(&mut ui, Role::Button, "Send as photo");
        click(&mut ui, Role::MenuItem, "document");
        assert_eq!(*chosen.borrow(), ["1"]);
    }

    #[test]
    fn a_long_name_gives_way_and_the_button_stays_inside() {
        let strip = AttachmentStrip::new([Attachment::new("a name far too long to fit ".repeat(10)).size(9)])
            .on_remove(|_| {});
        let mut ui = running(strip, 300.0);
        let tree = ui.access_tree();
        let button = tree.nodes.iter().find(|n| n.role == Role::Button).expect("the button");
        assert!(button.bounds.x() + button.bounds.width() <= 300.0, "{:?}", button.bounds);
    }

    #[test]
    fn a_thumbnail_takes_the_place_of_the_icon() {
        let pixel =
            ImageData::from_fn(8, 8, |_, _| ravenrs_core::Color::rgb(1.0, 0.0, 0.0)).expect("an image");
        let ui = running(AttachmentStrip::new([Attachment::new("a.jpg").thumbnail(pixel)]), 400.0);
        assert!(ui.scene().primitives().iter().any(|p| matches!(p, Primitive::Image(_))));
    }
}

mod selection_bar {
    use super::*;

    fn with_exit(count: usize, exits: Log) -> SelectionBar {
        SelectionBar::new(count).on_exit(move || exits.borrow_mut().push("exit".into()))
    }

    #[test]
    fn it_counts_with_the_labels_of_the_ui() {
        for (count, says) in [(0, "Nothing selected"), (1, "1 selected"), (4, "4 selected")] {
            let mut ui = running(SelectionBar::new(count), 500.0);
            assert!(has(&mut ui, Role::Status, says), "{count}");
        }
        let mut ui = running(SelectionBar::new(0).hint("Click the messages"), 500.0);
        assert!(has(&mut ui, Role::Status, "Click the messages"));
    }

    #[test]
    fn escape_and_the_button_leave() {
        let (exits, told) = log();
        let mut ui = running(with_exit(2, told), 500.0);
        press(&mut ui, Key::Escape);
        assert_eq!(exits.borrow().len(), 1);
        click(&mut ui, Role::Button, "Leave selection");
        assert_eq!(exits.borrow().len(), 2);
    }

    #[test]
    fn without_a_way_out_escape_goes_on_to_others() {
        let mut ui = running(SelectionBar::new(2), 500.0);
        assert!(!has(&mut ui, Role::Button, "Leave selection"));
        assert!(!ui.handle_event(&Event::KeyDown { key: Key::Escape, modifiers: Modifiers::default() }));
    }

    #[test]
    fn actions_run_and_wait_for_something_chosen() {
        let action = |told: Log| {
            ToolbarAction::new("Copy").icon(IconName::Copy).icon_only().on_click(move || {
                told.borrow_mut().push("copy".into());
            })
        };
        let (copied, told) = log();
        let mut ui = running(SelectionBar::new(2).action(action(told)), 500.0);
        click(&mut ui, Role::Button, "Copy");
        assert_eq!(copied.borrow().len(), 1);

        let (copied, told) = log();
        let mut ui = running(SelectionBar::new(0).action(action(told)), 500.0);
        let tree = ui.access_tree();
        let copy = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Copy")).expect("the action");
        assert!(copy.disabled);
        click(&mut ui, Role::Button, "Copy");
        assert!(copied.borrow().is_empty());
    }

    #[test]
    fn actions_sit_at_the_far_end() {
        let bar = SelectionBar::new(1).action(ToolbarAction::new("Copy").icon(IconName::Copy).icon_only());
        let mut ui = running(VStack::new((bar,)).align(ravenrs_layout::Align::Stretch), 500.0);
        let (_, copy) = node(&mut ui, Role::Button, "Copy");
        assert!(copy.x() + copy.width() > 450.0, "{copy:?}");
    }
}

mod audio_player {
    use super::*;

    /// Writes down what it is asked to say.
    #[derive(Clone, Default)]
    struct Heard(Log);

    impl SoundProvider for Heard {
        fn play(&self, _clip: &SoundClip) {
            self.0.borrow_mut().push("play".into());
        }
        fn speak(&self, text: &str, _language: &str) {
            self.0.borrow_mut().push(format!("speak {text}"));
        }
        fn stop_speaking(&self) {
            self.0.borrow_mut().push("stop".into());
        }
        fn speak_clip(&self, clip: &SoundClip) {
            self.0.borrow_mut().push(format!("clip {}", clip.seconds()));
        }
    }

    fn clip(seconds: usize) -> SoundClip {
        SoundClip::new(vec![0.0; seconds * 100], 100)
    }

    fn with_sound(view: impl View + 'static) -> (Ui, Log) {
        let heard = Heard::default();
        let log = heard.0.clone();
        let mut ui = Ui::new(view, Theme::raven_dark());
        ui.set_sound(heard);
        ui.set_viewport(Size::new(500.0, 500.0), 1.0);
        ui.set_time(10.0);
        settle(&mut ui);
        (ui, log)
    }

    fn progress(ui: &mut Ui) -> f64 {
        let tree = ui.access_tree();
        let bar = tree.nodes.iter().find(|n| n.role == Role::ProgressIndicator).expect("the progress");
        bar.numeric.as_ref().expect("a value").value
    }

    #[test]
    fn it_plays_shows_where_it_is_and_ends_by_itself() {
        let (mut ui, heard) = with_sound(AudioPlayerButton::new().clip(clip(5)));
        assert_eq!(progress(&mut ui), 0.0);
        tab_to(&mut ui, "Play audio");
        press(&mut ui, Key::Enter);
        assert_eq!(*heard.borrow(), ["clip 5"]);
        assert!(has(&mut ui, Role::Button, "Stop"), "the button changes meaning");
        assert_eq!(ui.access_tree().focused().and_then(|n| n.name.clone()).as_deref(), Some("Stop"));

        let wake = ui.next_wake().expect("wakes while playing");
        assert!(wake > 10.0 && wake <= 11.0, "{wake}");
        ui.set_time(12.5);
        settle(&mut ui);
        assert!((progress(&mut ui) - 2.5).abs() < 0.01);
        assert!(ui.next_wake().is_some_and(|wake| wake <= 13.0));

        ui.set_time(15.5);
        settle(&mut ui);
        assert!(has(&mut ui, Role::Button, "Play audio"));
        assert_eq!(progress(&mut ui), 0.0);
        assert_eq!(ui.next_wake(), None, "it asks for nothing at rest");
    }

    #[test]
    fn stop_stops_the_sound() {
        let (mut ui, heard) = with_sound(AudioPlayerButton::new().clip(clip(5)));
        click(&mut ui, Role::Button, "Play audio");
        click(&mut ui, Role::Button, "Stop");
        assert_eq!(*heard.borrow(), ["clip 5", "stop"]);
        assert!(has(&mut ui, Role::Button, "Play audio"));
    }

    #[test]
    fn playing_another_sends_the_first_back_to_rest() {
        let both =
            VStack::new((AudioPlayerButton::new().clip(clip(5)), AudioPlayerButton::new().clip(clip(7))));
        let (mut ui, heard) = with_sound(both);
        click(&mut ui, Role::Button, "Play audio");
        click(&mut ui, Role::Button, "Play audio");
        assert_eq!(*heard.borrow(), ["clip 5", "clip 7"]);
        ui.set_time(11.0);
        settle(&mut ui);
        let tree = ui.access_tree();
        let stops = tree.nodes.iter().filter(|n| n.name.as_deref() == Some("Stop")).count();
        assert_eq!(stops, 1);
    }

    #[test]
    fn the_app_can_deliver_the_audio_after_the_press() {
        struct Late(State<Option<SoundClip>>, Log);
        impl View for Late {
            fn build(&self, cx: &mut BuildCx<'_>) -> ravenrs_widgets::NodeId {
                let asked = self.1.clone();
                let mut player = AudioPlayerButton::new()
                    .seconds(5.0)
                    .on_request(move || asked.borrow_mut().push("asked".into()));
                if let Some(clip) = self.0.get() {
                    player = player.clip(clip);
                }
                cx.child(&player)
            }
        }
        let arrives = State::new(None);
        let (asked, told) = log();
        let (mut ui, heard) = with_sound(Late(arrives.clone(), told));
        click(&mut ui, Role::Button, "Play audio");
        assert_eq!(*asked.borrow(), ["asked"]);
        assert!(heard.borrow().is_empty());
        let tree = ui.access_tree();
        assert!(
            tree.nodes.iter().find(|n| n.name.as_deref() == Some("Play audio")).expect("button").disabled
        );
        arrives.set(Some(clip(5)));
        settle(&mut ui);
        assert_eq!(*heard.borrow(), ["clip 5"]);
        assert!(has(&mut ui, Role::Button, "Stop"));
    }

    #[test]
    fn with_nothing_to_play_it_is_disabled() {
        let (mut ui, heard) = with_sound(AudioPlayerButton::new());
        click(&mut ui, Role::Button, "Play audio");
        assert!(heard.borrow().is_empty());
    }

    #[test]
    fn the_transcript_opens_and_closes() {
        let (mut ui, _) = with_sound(AudioPlayerButton::new().clip(clip(1)).transcript("What was said"));
        assert!(!has(&mut ui, Role::Label, "What was said"));
        let tree = ui.access_tree();
        let toggle = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Transcript")).expect("toggle");
        assert_eq!(toggle.expanded, Some(false));
        click(&mut ui, Role::Button, "Transcript");
        assert!(has(&mut ui, Role::Label, "What was said"));
        click(&mut ui, Role::Button, "Transcript");
        assert!(!has(&mut ui, Role::Label, "What was said"));
    }

    #[test]
    fn a_blank_transcript_says_there_is_no_speech() {
        let open = State::new(true);
        let player = AudioPlayerButton::new().transcript("  ").transcript_open(open.binding());
        let (mut ui, _) = with_sound(player);
        assert!(has(&mut ui, Role::Label, "(no speech)"));
    }

    #[test]
    fn it_can_ask_for_a_transcript() {
        let (asked, told) = log();
        let player =
            AudioPlayerButton::new().on_transcribe(move || told.borrow_mut().push("transcribe".into()));
        let (mut ui, _) = with_sound(player);
        click(&mut ui, Role::Button, "Transcribe");
        assert_eq!(asked.borrow().len(), 1);
        let (mut ui, _) = with_sound(AudioPlayerButton::new().on_transcribe(|| {}).transcribing(true));
        let tree = ui.access_tree();
        assert!(
            tree.nodes.iter().find(|n| n.name.as_deref() == Some("Transcribing…")).expect("button").disabled
        );
        assert!(!has(&mut ui, Role::Button, "Transcribe"));
    }
}

mod chat_media {
    use super::*;

    fn chat(messages: Vec<ChatMessage>) -> (Ui, State<Vec<ChatMessage>>) {
        let state = State::new(messages);
        let chat = Chat::new(state.binding());
        (running(VStack::new((chat,)).height(600.0), 700.0), state)
    }

    fn picture() -> ImageData {
        ImageData::from_fn(32, 32, |_, _| ravenrs_core::Color::rgb(0.2, 0.5, 0.9)).expect("an image")
    }

    #[test]
    fn a_message_without_media_is_what_it_always_was() {
        let (mut ui, _) = chat(vec![ChatMessage::person("Hello *there*")]);
        assert!(has(&mut ui, Role::Label, "Hello *there*"), "marks stay literal unless asked");
        let writing = ChatMessage::assistant_writing(Default::default());
        let (with_spinner, _) = chat(vec![writing]);
        assert!(with_spinner.next_wake().is_some(), "an empty answer says it is on its way");
    }

    #[test]
    fn a_formatted_message_loses_its_marks_and_its_links_reach_the_chat() {
        let (opened, told) = log();
        let state = State::new(vec![ChatMessage::assistant("*Hi* https://a.io").formatted()]);
        let chat = Chat::new(state.binding()).on_link(move |address| told.borrow_mut().push(address));
        let mut ui = running(VStack::new((chat,)).height(600.0), 700.0);
        assert!(has(&mut ui, Role::Label, "Hi https://a.io"));
        click(&mut ui, Role::Link, "https://a.io");
        assert_eq!(*opened.borrow(), ["https://a.io"]);
    }

    #[test]
    fn a_photo_opens_and_holds_its_place_until_it_arrives() {
        let (opened, told) = log();
        let open = move || told.borrow_mut().push("open".into());
        let photo = ChatMedia::photo(Some(picture())).alt("A beach").on_open(open.clone());
        let (mut ui, _) = chat(vec![ChatMessage::person("").media(photo)]);
        click(&mut ui, Role::Button, "Open A beach");
        assert_eq!(opened.borrow().len(), 1);
        tab_to(&mut ui, "Open A beach");
        press(&mut ui, Key::Space);
        assert_eq!(opened.borrow().len(), 2);
        assert!(ui.scene().primitives().iter().any(|p| matches!(p, Primitive::Image(_))));

        let photo = |image| ChatMedia::photo(image).alt("A beach");
        let (mut with, _) = chat(vec![ChatMessage::assistant("").media(photo(Some(picture())))]);
        let (mut without, _) = chat(vec![ChatMessage::assistant("").media(photo(None))]);
        let frame = |ui: &mut Ui| node(ui, Role::Image, "A beach").1.size;
        assert_eq!(frame(&mut with), frame(&mut without));
        assert!(has(&mut without, Role::Label, "Loading photo…"));
    }

    #[test]
    fn empty_text_with_media_is_not_an_answer_on_its_way() {
        let (mut with, _) = chat(vec![ChatMessage::assistant("").media(ChatMedia::document("a.pdf"))]);
        assert!(has(&mut with, Role::Group, "a.pdf"));
        assert_eq!(with.next_wake(), None, "a spinner would say an answer is on its way");
    }

    #[test]
    fn a_document_says_what_it_is_and_opens() {
        let (opened, told) = log();
        let document = ChatMedia::document("contract.pdf")
            .size(2048)
            .on_open(move || told.borrow_mut().push("open".into()));
        let (mut ui, _) = chat(vec![ChatMessage::person("See this").media(document)]);
        click(&mut ui, Role::Button, "Open contract.pdf, 2.0 KB");
        assert_eq!(opened.borrow().len(), 1);
        assert!(has(&mut ui, Role::Label, "See this"), "the text stays under it");
    }

    #[test]
    fn a_recording_plays_inside_the_bubble() {
        let audio = ChatMedia::audio(|| {
            AudioPlayerButton::new().clip(SoundClip::new(vec![0.0; 300], 100)).transcript("Hello")
        });
        let (mut ui, _) = chat(vec![ChatMessage::person("").media(audio)]);
        let (_, play) = node(&mut ui, Role::Button, "Play audio");
        assert!(play.width() > 0.0);
        let bar = ui.access_tree().nodes.iter().find(|n| n.role == Role::ProgressIndicator).map(|n| n.bounds);
        assert!(bar.expect("the progress").width() > 30.0, "{bar:?}");
        click(&mut ui, Role::Button, "Play audio");
        assert!(has(&mut ui, Role::Button, "Stop"));
    }

    #[test]
    fn a_card_and_listening_still_work_with_media() {
        let state = State::new(vec![
            ChatMessage::assistant("Answer")
                .media(ChatMedia::document("a.pdf"))
                .card(|| Box::new(Button::new("Act"))),
        ]);
        let chat = Chat::new(state.binding()).on_listen(|_| {});
        let mut ui = running(VStack::new((chat,)).height(600.0), 700.0);
        assert!(has(&mut ui, Role::Button, "Act"));
        assert!(has(&mut ui, Role::Button, "Listen"));
    }
}
