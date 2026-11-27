//! YearView: the twelve months of a year with marks, today, the day chosen, the months as
//! buttons and how many go on a line; or the months of a season without the year above them.
use ravenrs::prelude::*;

/// The control registered in the center.
pub const ID: &str = "YearViewDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Ano";

/// Every public feature of YearView on one page.
#[derive(Debug, Default)]
pub struct YearViewPage;

impl View for YearViewPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let year = cx.use_state(|| 2026);
        let chosen: State<Option<Date>> = cx.use_state(|| None);
        let said = cx.use_state(|| "Nenhum dia escolhido.".to_owned());
        let (picked, opened) = (said.clone(), said.clone());
        let today = Date::new(2026, 11, 15).unwrap_or_else(|| unreachable!("a real date"));
        let summer = Date::new(2025, 12, 1).unwrap_or_else(|| unreachable!("a real date"));
        let season_year = State::new(2026);
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new("Os doze meses com marcas nos dias, ou só os meses de uma estação.")
                    .foreground(Semantic::InkMuted),
                YearView::new(year.binding())
                    .selected(chosen.binding())
                    .today(today)
                    .columns(4)
                    .marks(|d| (d.day() == 8).then(|| DayMark::new().tone(Tone::Accent).label("Ensaio")))
                    .on_pick(move |d| picked.set(format!("Escolhido: {d}")))
                    .on_month(move |d| opened.set(format!("Mês aberto: {}/{}", d.month(), d.year()))),
                Text::new(said.get()),
                Text::new("Verão, sem o ano em cima").style(TextStyle::Headline),
                YearView::new(season_year.binding()).months(summer, 3).header(false).columns(3),
            ))
            .spacing(16.0)
            .padding(16.0)
            .align(Align::Start),
        )
    }
}
