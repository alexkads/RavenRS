//! `Chat`, `AttachmentStrip`, `AudioPlayerButton` and `SelectionBar`, every feature: a
//! conversation with a store's assistant that answers with marks and links, the files about to
//! go with a message, recordings that arrive late or ask for their transcript, and a bar over
//! the orders picked. Every event writes what happened in a line.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "ChatDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Conversa";

/// What the chat offers to ask before the first message.
pub const EXAMPLES: [&str; 2] = ["Onde está meu pedido?", "Quero trocar um item"];
/// The orders the selection bar acts on.
pub const ORDERS: [&str; 3] = ["Pedido 101", "Pedido 102", "Pedido 103"];
/// What the recording that asks for its transcript says, once it gets one.
pub const TRANSCRIPT: &str = "Oi, o pedido 102 chega amanhã de manhã.";

/// A second of a low tone, computed here: an app delivers its own recording.
fn tone(seconds: f32) -> SoundClip {
    let rate = 22_050_u32;
    let samples: Vec<f32> = (0..(rate as f32 * seconds) as usize)
        .map(|i| (i as f32 / rate as f32 * 220.0 * std::f32::consts::TAU).sin() * 0.2)
        .collect();
    SoundClip::new(samples, rate)
}

/// The page.
#[derive(Debug, Default)]
pub struct ChatPage;

impl View for ChatPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let messages = cx.use_state(Vec::<ChatMessage>::new);
        let busy = cx.use_state(|| false);
        let files = cx.use_state(|| {
            vec![
                ("nota-fiscal.pdf".to_string(), 120_000_u64, IconName::FileText),
                ("foto-da-caixa.jpg".to_string(), 2_400_000, IconName::Image),
                ("planilha.xlsx".to_string(), 48_000, IconName::Table),
            ]
        });
        let photo_as = cx.use_state(|| 0_usize);
        let late_clip = cx.use_state(|| None::<SoundClip>);
        let loading = cx.use_state(|| false);
        let transcript = cx.use_state(|| None::<String>);
        let transcribing = cx.use_state(|| false);
        let transcript_open = cx.use_state(|| true);
        let picked = [cx.use_state(|| false), cx.use_state(|| false), cx.use_state(|| false)];
        let said = cx.use_state(|| "Nada aconteceu ainda.".to_string());
        let narrow = cx.viewport().width < 720.0;
        let wide = if narrow { (cx.viewport().width - 32.0).max(240.0) } else { 460.0 };

        // The chat: whoever answers adds to the messages; here, at once, with marks and a link.
        let (answers, sent, heard, followed) = (messages.clone(), said.clone(), said.clone(), said.clone());
        let chat = Chat::new(messages.binding())
            .placeholder("Pergunte sobre o pedido")
            .intro("A assistente da loja responde na hora. Escolha uma pergunta ou escreva a sua.")
            .examples(EXAMPLES)
            .busy(busy.get())
            .on_send(move |text| {
                sent.set(format!("Enviado: {text}"));
                answers.update(|all| {
                    all.push(
                        ChatMessage::assistant(format!(
                            "*Recebi:* {text}. Acompanhe em https://ravenrs.dev/pedidos ou responda aqui."
                        ))
                        .formatted(),
                    );
                });
            })
            .on_listen(move |text| heard.set(format!("Ouvir: {text}")))
            .on_link(move |address| followed.set(format!("Link: {address}")));

        // The files: the app holds the list, and changes it when told.
        let (removed, list, chose) = (said.clone(), files.clone(), photo_as.clone());
        let strip = AttachmentStrip::new(files.get().into_iter().map(|(name, size, icon)| {
            let file = Attachment::new(name.clone()).size(size).icon(icon);
            if icon == IconName::Image {
                let chose = chose.clone();
                file.send_as(["foto", "documento"], photo_as.get(), move |how| chose.set(how))
            } else {
                file
            }
        }))
        .on_remove(move |index| {
            let name = list.get().get(index).map(|f| f.0.clone()).unwrap_or_default();
            list.update(|all| {
                if index < all.len() {
                    all.remove(index);
                }
            });
            removed.set(format!("Tirou {name}"));
        });

        // A recording that arrives when asked for, and one that asks for its transcript.
        let (asked, clip_in, wait) = (said.clone(), late_clip.clone(), loading.clone());
        let late = AudioPlayerButton::new().seconds(1.5).loading(loading.get());
        let late = match late_clip.get() {
            Some(clip) => late.clip(clip),
            None => late.on_request(move || {
                wait.set(true);
                asked.set("Pediu o áudio".into());
                // The app would fetch it; here it is ready at once.
                clip_in.set(Some(tone(1.5)));
                wait.set(false);
            }),
        };
        let (want, write) = (said.clone(), transcribing.clone());
        let to_write = AudioPlayerButton::new().clip(tone(2.0)).transcribing(transcribing.get());
        let to_write = match transcript.get() {
            Some(text) => to_write.transcript(text).transcript_open(transcript_open.binding()),
            None => to_write.on_transcribe(move || {
                write.set(true);
                want.set("Pediu a transcrição".into());
            }),
        };
        let (deliver, done) = (transcript.clone(), transcribing.clone());

        // The bar over what is picked.
        let count = picked.iter().filter(|p| p.get()).count();
        let boxes: Vec<Toggle> =
            ORDERS.iter().zip(&picked).map(|(name, pick)| Toggle::new(pick.binding()).label(*name)).collect();
        let (left, archived, clear) = (said.clone(), said.clone(), picked.clone());
        let bar = SelectionBar::new(count)
            .hint(if narrow { "Marque pedidos" } else { "Marque pedidos para agir sobre eles" })
            .on_exit(move || {
                clear.iter().for_each(|p| p.set(false));
                left.set("Saiu da seleção".into());
            })
            .action(
                ToolbarAction::new("Arquivar")
                    .icon(IconName::Folder)
                    .on_click(move || archived.set(format!("Arquivou {count} pedidos"))),
            );

        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Toggle::new(busy.binding()).label("Resposta a caminho"),
                VStack::new((chat,)).height(if narrow { 360.0 } else { 420.0 }).width(wide),
                Text::new("Arquivos da mensagem").style(TextStyle::Headline),
                VStack::new((strip,)).width(wide),
                Text::new(format!("A foto vai como: {}", ["foto", "documento"][photo_as.get().min(1)])),
                Text::new("Áudios").style(TextStyle::Headline),
                VStack::new((late, to_write)).spacing(8.0).width(wide),
                Button::new("Entregar a transcrição").style(ButtonStyle::Secondary).on_click(move || {
                    deliver.set(Some(TRANSCRIPT.into()));
                    done.set(false);
                }),
                VStack::new((
                    Text::new("Seleção").style(TextStyle::Headline),
                    VStack::new(boxes).spacing(4.0),
                    VStack::new((bar,)).width(wide),
                ))
                .spacing(12.0)
                .align(Align::Start),
                Text::new(said.get()),
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Start),
        )
    }
}
