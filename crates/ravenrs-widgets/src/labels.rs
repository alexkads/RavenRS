//! The words the built-in components show or read aloud, in one place.

use crate::context::{BuildCx, NodeId};
use crate::date::Date;
use crate::grid::Summary;
use crate::view::View;

/// The words the built-in components use. Start from [`Labels::default`]
/// (English) or [`Labels::pt_br`], change what you need, and give it to
/// [`Localized`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Labels {
    /// Closes a dialog or a sheet.
    pub close: String,
    /// A sidebar's name.
    pub navigation: String,
    /// A sidebar trigger's name.
    pub toggle_sidebar: String,
    /// Dismisses a toast.
    pub dismiss: String,
    /// A spinner's name.
    pub loading: String,
    /// A stepper's minus button.
    pub decrease: String,
    /// A stepper's plus button.
    pub increase: String,
    /// The error of an empty [`FormField`](crate::FormField) marked as required.
    pub required: String,
    /// A date picker with no date.
    pub pick_date: String,
    /// A combobox with no choice.
    pub select: String,
    /// A combobox's search field.
    pub search: String,
    /// A combobox's search that found nothing.
    pub no_results: String,
    /// A command palette's field.
    pub search_commands: String,
    /// A command palette's search that found nothing.
    pub no_commands: String,
    /// A command palette's name.
    pub command_palette: String,
    /// A theme switcher's name.
    pub theme: String,
    /// The dark theme.
    pub theme_dark: String,
    /// The light theme.
    pub theme_light: String,
    /// The glass theme.
    pub theme_glass: String,
    /// A calendar's back button.
    pub previous_month: String,
    /// A calendar's forward button.
    pub next_month: String,
    /// Marks today in a calendar.
    pub today: String,
    /// Sunday to Saturday, short, as a calendar's columns.
    pub weekdays: [String; 7],
    /// January to December.
    pub months: [String; 12],
    /// Between a month and its year, such as `" "` or `" de "`.
    pub month_year: String,
    /// Writes a date the way people here do, for a date picker.
    pub short_date: fn(Date) -> String,
    /// A grid's search field.
    pub grid_search: String,
    /// A grid's grouping menu.
    pub grid_group: String,
    /// Removes a grid's grouping.
    pub grid_clear_grouping: String,
    /// A grid's column menu.
    pub grid_columns: String,
    /// Puts a grid's sorting and filters back.
    pub grid_reset: String,
    /// A grid's export button.
    pub grid_export: String,
    /// A grid with nothing to show.
    pub grid_no_rows: String,
    /// A grid's name.
    pub grid_name: String,
    /// A row's check box.
    pub grid_select_row: String,
    /// A column sorted up.
    pub grid_sorted_ascending: String,
    /// A column sorted down.
    pub grid_sorted_descending: String,
    /// A text column's filter hint.
    pub grid_filter_hint: String,
    /// A number column's filter hint.
    pub grid_number_filter_hint: String,
    /// Before a column's name, in its filter's name.
    pub grid_filter_column: String,
    /// A group with no value.
    pub grid_empty: String,
    /// Between shown and total rows.
    pub grid_of: String,
    /// After the row count.
    pub grid_rows: String,
    /// After the count of selected rows.
    pub grid_selected: String,
    /// Count, sum, average, smallest and largest, as a grid's footer calls them.
    pub grid_summaries: [String; 5],
}

fn english_date(date: Date) -> String {
    date.short()
}

fn brazilian_date(date: Date) -> String {
    format!("{:02}/{:02}/{:04}", date.day(), date.month(), date.year())
}

fn owned<const N: usize>(words: [&str; N]) -> [String; N] {
    words.map(str::to_owned)
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            close: "Close".into(),
            navigation: "Navigation".into(),
            toggle_sidebar: "Toggle sidebar".into(),
            dismiss: "Dismiss".into(),
            loading: "Loading".into(),
            decrease: "Decrease".into(),
            increase: "Increase".into(),
            required: "This field is required.".into(),
            pick_date: "Pick a date".into(),
            select: "Select…".into(),
            search: "Search…".into(),
            no_results: "No results.".into(),
            search_commands: "Type a command or search…".into(),
            no_commands: "No matching commands.".into(),
            command_palette: "Command palette".into(),
            theme: "Theme".into(),
            theme_dark: "Dark".into(),
            theme_light: "Light".into(),
            theme_glass: "Glass".into(),
            previous_month: "Previous month".into(),
            next_month: "Next month".into(),
            today: "Today".into(),
            weekdays: owned(["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]),
            months: owned([
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ]),
            month_year: " ".into(),
            short_date: english_date,
            grid_search: "Search all columns".into(),
            grid_group: "Group".into(),
            grid_clear_grouping: "Clear grouping".into(),
            grid_columns: "Columns".into(),
            grid_reset: "Reset".into(),
            grid_export: "Export CSV".into(),
            grid_no_rows: "No rows match the current filters.".into(),
            grid_name: "Data grid".into(),
            grid_select_row: "Select row".into(),
            grid_sorted_ascending: "Sorted ascending".into(),
            grid_sorted_descending: "Sorted descending".into(),
            grid_filter_hint: "Filter…".into(),
            grid_number_filter_hint: "e.g. >100".into(),
            grid_filter_column: "Filter".into(),
            grid_empty: "(empty)".into(),
            grid_of: "of".into(),
            grid_rows: "rows".into(),
            grid_selected: "selected".into(),
            grid_summaries: owned(["Count", "Sum", "Avg", "Min", "Max"]),
        }
    }
}

impl Labels {
    /// Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self {
            close: "Fechar".into(),
            navigation: "Navegação".into(),
            toggle_sidebar: "Alternar barra lateral".into(),
            dismiss: "Dispensar".into(),
            loading: "Carregando".into(),
            decrease: "Diminuir".into(),
            increase: "Aumentar".into(),
            required: "Preencha este campo.".into(),
            pick_date: "Escolha uma data".into(),
            select: "Selecione…".into(),
            search: "Buscar…".into(),
            no_results: "Nada encontrado.".into(),
            search_commands: "Digite um comando ou busque…".into(),
            no_commands: "Nenhum comando encontrado.".into(),
            command_palette: "Paleta de comandos".into(),
            theme: "Tema".into(),
            theme_dark: "Escuro".into(),
            theme_light: "Claro".into(),
            theme_glass: "Vidro".into(),
            previous_month: "Mês anterior".into(),
            next_month: "Próximo mês".into(),
            today: "Hoje".into(),
            weekdays: owned(["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"]),
            months: owned([
                "janeiro",
                "fevereiro",
                "março",
                "abril",
                "maio",
                "junho",
                "julho",
                "agosto",
                "setembro",
                "outubro",
                "novembro",
                "dezembro",
            ]),
            month_year: " de ".into(),
            short_date: brazilian_date,
            grid_search: "Buscar em todas as colunas".into(),
            grid_group: "Agrupar".into(),
            grid_clear_grouping: "Desfazer agrupamento".into(),
            grid_columns: "Colunas".into(),
            grid_reset: "Limpar".into(),
            grid_export: "Exportar CSV".into(),
            grid_no_rows: "Nenhuma linha atende aos filtros.".into(),
            grid_name: "Grade de dados".into(),
            grid_select_row: "Selecionar linha".into(),
            grid_sorted_ascending: "Em ordem crescente".into(),
            grid_sorted_descending: "Em ordem decrescente".into(),
            grid_filter_hint: "Filtrar…".into(),
            grid_number_filter_hint: "ex.: >100".into(),
            grid_filter_column: "Filtrar".into(),
            grid_empty: "(vazio)".into(),
            grid_of: "de".into(),
            grid_rows: "linhas".into(),
            grid_selected: "selecionadas".into(),
            grid_summaries: owned(["Qtd.", "Soma", "Média", "Mín.", "Máx."]),
        }
    }

    /// A month and its year: `November 2026`, `novembro de 2026`.
    pub fn month_title(&self, date: Date) -> String {
        let month = &self.months[usize::from(date.month().clamp(1, 12)) - 1];
        format!("{month}{}{}", self.month_year, date.year())
    }

    /// A full date, for screen readers: `15 November 2026`, `15 novembro de 2026`.
    pub fn long_date(&self, date: Date) -> String {
        format!("{} {}", date.day(), self.month_title(date))
    }

    /// What a grid's footer calls this summary.
    pub fn summary(&self, summary: Summary) -> &str {
        let index = match summary {
            Summary::Count => 0,
            Summary::Sum => 1,
            Summary::Average => 2,
            Summary::Min => 3,
            _ => 4,
        };
        &self.grid_summaries[index]
    }
}

/// Everything inside uses these words instead of the English defaults: wrap the
/// root of an app, or just the part that speaks another language.
///
/// ```
/// use ravenrs_widgets::{DatePicker, Date, Labels, Localized};
/// use ravenrs_reactive::State;
///
/// let day = State::new(Date::new(2026, 11, 15));
/// let app = Localized::new(Labels::pt_br(), DatePicker::new(day.binding(), Date::new(2026, 11, 1).unwrap_or(Date::from_days(0))));
/// # let _ = app;
/// ```
#[derive(Debug)]
pub struct Localized<V> {
    labels: std::rc::Rc<Labels>,
    content: V,
}

impl<V: View> Localized<V> {
    /// `content`, using `labels`.
    pub fn new(labels: Labels, content: V) -> Self {
        Self { labels: std::rc::Rc::new(labels), content }
    }
}

impl<V: View> View for Localized<V> {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let outer = cx.replace_labels(self.labels.clone());
        let node = cx.child(&self.content);
        cx.replace_labels(outer);
        node
    }
}
