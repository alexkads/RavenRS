//! What the update says on screen: the banner and the news.

use ravenrs::prelude::*;

use crate::model::parse_date;
use crate::update::{self, News, UpdateStatus, Updater};

/// Connects the updater to the UI and takes in what its thread finished.
/// That thread wakes the UI once, when done, so nothing runs while it works.
pub(crate) struct UpdatePump(pub Updater);

impl View for UpdatePump {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.0.connect(cx.ui_waker());
        self.0.poll();
        cx.empty()
    }
}

/// The banner under the toolbar; absent when there is nothing to say.
pub(crate) fn banner(updater: &Updater, news_open: &State<bool>) -> Option<impl View + use<>> {
    let status = updater.status();
    let (tone, title, message): (Tone, String, String) = match &status {
        UpdateStatus::Quiet => return None,
        UpdateStatus::Available(n) => (
            if n.importante { Tone::Warning } else { Tone::Accent },
            format!("Versão {} disponível", n.versao),
            n.titulo.clone(),
        ),
        UpdateStatus::Development(n) => (
            Tone::Neutral,
            format!("Versão {} no repositório", n.versao),
            "Esta cópia foi compilada à mão: atualize com git pull.".to_owned(),
        ),
        UpdateStatus::Compiling(n) => (
            Tone::Accent,
            format!("Instalando a versão {}", n.versao),
            "Compilando em segundo plano. Pode continuar usando o app.".to_owned(),
        ),
        UpdateStatus::Ready(n) => {
            (Tone::Success, format!("Versão {} instalada", n.versao), "Reabra o app para usar.".to_owned())
        }
        UpdateStatus::Failed(n, reason) => (
            Tone::Danger,
            format!("A versão {} não foi instalada", n.versao),
            format!("O app continua funcionando como estava. {reason}"),
        ),
    };
    let compiling = matches!(status, UpdateStatus::Compiling(_));
    let action: Option<AnyView> = match status {
        UpdateStatus::Available(n) | UpdateStatus::Failed(n, _) => {
            let retry = matches!(updater.status(), UpdateStatus::Failed(..));
            let updater = updater.clone();
            Some(Box::new(
                Button::new(if retry { "Tentar de novo" } else { "Atualizar" })
                    .size(ButtonSize::Small)
                    .icon(IconName::Download)
                    .on_click(move || updater.install(n.clone())),
            ))
        }
        UpdateStatus::Ready(_) => {
            let updater = updater.clone();
            Some(Box::new(
                Button::new("Reabrir agora")
                    .size(ButtonSize::Small)
                    .icon(IconName::RotateCcw)
                    .on_click(move || updater.reopen()),
            ))
        }
        _ => None,
    };
    let news = news_open.clone();
    Some(
        HStack::new((
            compiling.then(|| Spinner::new().size(16.0)),
            Badge::new(title).tone(tone),
            Text::new(message).style(TextStyle::Callout).truncate().grow(1.0),
            Button::new("Ver novidades")
                .size(ButtonSize::Small)
                .style(ButtonStyle::Ghost)
                .on_click(move || news.set(true)),
            action,
        ))
        .spacing(10.0)
        .align(Align::Center)
        .padding((10.0, 24.0))
        .background(Semantic::Surface),
    )
}

fn news_block(news: &News) -> Disclosure {
    let day = news.data.as_deref().and_then(parse_date).map(crate::model::date_label);
    let title = match day {
        Some(day) => format!("Versão {} · {day}", news.versao),
        None => format!("Versão {}", news.versao),
    };
    let items: Vec<AnyView> = news
        .novidades
        .iter()
        .map(|line| Box::new(Text::new(format!("• {line}")).style(TextStyle::Callout)) as AnyView)
        .collect();
    Disclosure::new(
        title,
        VStack::new((
            Text::new(news.titulo.clone()),
            VStack::new(items).spacing(6.0),
            Text::new(news.por_que_atualizar.clone())
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
        ))
        .spacing(10.0),
    )
}

/// Every version so far, plus the announced one when it is not in this copy's history.
pub(crate) fn news_sheet(updater: &Updater, open: &State<bool>) -> impl View + use<> {
    let mut all = update::history();
    let announced = match updater.status() {
        UpdateStatus::Available(n)
        | UpdateStatus::Compiling(n)
        | UpdateStatus::Ready(n)
        | UpdateStatus::Failed(n, _)
        | UpdateStatus::Development(n) => Some(n),
        UpdateStatus::Quiet => None,
    };
    if let Some(news) = announced
        && !all.iter().any(|known| known.versao == news.versao)
    {
        all.insert(0, news);
    }
    let blocks: Vec<Disclosure> =
        all.iter().enumerate().map(|(i, news)| news_block(news).open(i == 0)).collect();
    Sheet::new(open.binding(), "Novidades").width(460.0).content(ScrollView::new((Accordion::new(blocks),)))
}
