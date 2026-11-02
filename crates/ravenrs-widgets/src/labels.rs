//! The words the built-in components show or read aloud, in one place.

use crate::context::{BuildCx, NodeId};
use crate::date::Date;
use crate::grid::Summary;
use crate::recurrence::{Frequency, RecurrenceEnd, RecurrenceRule};
use crate::view::View;

/// The words the built-in components use. Start from [`Labels::default`]
/// (English) or [`Labels::pt_br`], change what you need, and give it to
/// [`Localized`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Labels {
    /// Closes a dialog or a sheet.
    pub close: String,
    /// The name of a [`HelpTip`](crate::HelpTip), before what it explains.
    pub help: String,
    /// A sidebar's name.
    pub navigation: String,
    /// A sidebar trigger's name.
    pub toggle_sidebar: String,
    /// Dismisses a toast.
    pub dismiss: String,
    /// A [`Chat`](crate::Chat)'s send button.
    pub send: String,
    /// A [`Chat`](crate::Chat)'s button to hear an answer.
    pub listen: String,
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
    /// A code block's copy button.
    pub copy: String,
    /// A code block's copy button, right after copying.
    pub copied: String,
    /// The list of versions in a [`ReleaseNotes`](crate::ReleaseNotes).
    pub release_versions: String,
    /// Goes to the next newer version in a [`ReleaseNotes`](crate::ReleaseNotes).
    pub release_newer: String,
    /// Goes to the next older version in a [`ReleaseNotes`](crate::ReleaseNotes).
    pub release_older: String,
    /// Between the position and the total, as in "2 of 5".
    pub release_of: String,
    /// The diagnostics panel's title.
    pub diagnostics: String,
    /// The button that starts a performance test.
    pub diagnostics_test: String,
    /// A performance test is running.
    pub diagnostics_testing: String,
    /// What to do while a performance test runs.
    pub diagnostics_testing_detail: String,
    /// A performance test found the app fluid.
    pub diagnostics_test_good: String,
    /// A performance test found visible stutter.
    pub diagnostics_test_fair: String,
    /// A performance test found the app slow.
    pub diagnostics_test_poor: String,
    /// After the rate a performance test measured.
    pub diagnostics_test_fps: String,
    /// A performance test found nothing to blame.
    pub diagnostics_test_cause_none: String,
    /// A performance test blames drawing.
    pub diagnostics_test_cause_gpu: String,
    /// A performance test blames building the screen.
    pub diagnostics_test_cause_interface: String,
    /// Diagnostics: frames are drawn on the CPU.
    pub diagnostics_software: String,
    /// What drawing on the CPU means, and what to do about it.
    pub diagnostics_software_detail: String,
    /// Diagnostics: the app was built without optimizations.
    pub diagnostics_unoptimized: String,
    /// What a build without optimizations means, and what to do about it.
    pub diagnostics_unoptimized_detail: String,
    /// Diagnostics: GPUs left out because their drivers are not conformant.
    pub diagnostics_hidden: String,
    /// Diagnostics: no GPU could be used.
    pub diagnostics_no_gpu: String,
    /// Diagnostics: the time of a whole frame.
    pub diagnostics_frame: String,
    /// Diagnostics: the time spent rebuilding and laying out the UI.
    pub diagnostics_update: String,
    /// Diagnostics: the time spent waiting for a texture to draw on.
    pub diagnostics_acquire: String,
    /// Diagnostics: the time spent recording and submitting GPU work.
    pub diagnostics_render: String,
    /// Diagnostics: the button that opens a new bug report in the browser.
    pub diagnostics_report_bug: String,
    /// Diagnostics: the title of what a bug report would share.
    pub diagnostics_report_shares: String,
    /// Diagnostics: what a bug report would share and that nothing is sent yet.
    pub diagnostics_report_shares_detail: String,
    /// Diagnostics: the browser could not be opened for a bug report.
    pub diagnostics_report_failed: String,
    /// What to do about the browser not opening.
    pub diagnostics_report_failed_detail: String,
    /// The start of a new bug report: the questions for the person to answer.
    pub diagnostics_report_questions: String,
    /// After a report cut short to fit in a URL.
    pub diagnostics_report_truncated: String,
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
    /// Above the errors that keep a form from being saved.
    pub form_errors: String,
    /// A [`LookupField`](crate::LookupField)'s button that empties it.
    pub clear: String,
    /// A question's button that gives up on it.
    pub cancel: String,
    /// The menu of commands that did not fit.
    pub more_commands: String,
    /// A number's decimal separator.
    pub decimal_separator: char,
    /// A number's thousands separator.
    pub group_separator: char,
    /// A [`TimePicker`](crate::TimePicker)'s hint and button.
    pub pick_time: String,
    /// A number field's button that adds a step.
    pub step_up: String,
    /// A number field's button that takes a step away.
    pub step_down: String,
    /// After the count of events an [`EventCalendar`](crate::EventCalendar) day has no room for.
    pub calendar_more: String,
    /// The header over a [`Scheduler`](crate::Scheduler)'s resources.
    pub scheduler_resources: String,
    /// Zooms a scheduler's time axis in.
    pub zoom_in: String,
    /// Zooms a scheduler's time axis out.
    pub zoom_out: String,
    /// A recurrence editor's name.
    pub recurrence_name: String,
    /// Once, daily, weekly, monthly and yearly, as an editor offers them.
    pub recurrence_frequencies: [String; 5],
    /// Before the interval, such as "Every".
    pub recurrence_every: String,
    /// The singular and plural of day, week, month and year.
    pub recurrence_units: [(String, String); 4],
    /// A rule repeating every day, week, month and year, in words.
    pub recurrence_each: [String; 4],
    /// What says when a rule stops.
    pub recurrence_ends_name: String,
    /// Never, on a date and after a number of times, as an editor offers them.
    pub recurrence_ends: [String; 3],
    /// After the number of times.
    pub recurrence_times: String,
    /// Before the last day, in words.
    pub recurrence_until: String,
    /// Before the next days, in words.
    pub recurrence_next: String,
    /// What a grid's group panel says while nothing is grouped.
    pub grid_drag_hint: String,
    /// Before a column a grid is grouped by.
    pub grid_grouped_by: String,
    /// Stops grouping a grid by a column.
    pub grid_remove_group: String,
    /// Opens every group of a grid.
    pub grid_expand_all: String,
    /// Closes every group of a grid.
    pub grid_collapse_all: String,
    /// Sorts a grid's column from smallest to largest.
    pub grid_sort_ascending: String,
    /// Sorts a grid's column from largest to smallest.
    pub grid_sort_descending: String,
    /// Stops sorting by a grid's column.
    pub grid_clear_sort: String,
    /// Groups a grid by a column.
    pub grid_group_by_column: String,
    /// Stops grouping a grid by a column, in its menu.
    pub grid_ungroup_column: String,
    /// Keeps a grid's column in place.
    pub grid_pin_column: String,
    /// Lets a grid's column scroll again.
    pub grid_unpin_column: String,
    /// Hides a grid's column.
    pub grid_hide_column: String,
    /// Fits a grid's column to its content.
    pub grid_fit_column: String,
    /// Shows a grid's group panel.
    pub grid_show_panel: String,
    /// Hides a grid's group panel.
    pub grid_hide_panel: String,
    /// Opens a grid's column menu.
    pub grid_column_menu: String,
    /// Filters a grid's column by its values.
    pub grid_filter_values: String,
    /// Removes a grid's filters.
    pub grid_clear_filter: String,
    /// Before what a grid is filtering by.
    pub grid_filter_label: String,
    /// Every value of a grid's column, in its filter.
    pub grid_all_values: String,
    /// A grid's menu of saved arrangements.
    pub grid_scenario: String,
    /// Saves how a grid is arranged, under a new name.
    pub grid_scenario_save: String,
    /// Saves over the scenario in use.
    pub grid_scenario_update: String,
    /// Deletes the scenario in use.
    pub grid_scenario_delete: String,
    /// Puts a grid back as it first was.
    pub grid_scenario_original: String,
    /// What to call a scenario.
    pub grid_scenario_name: String,
    /// Confirms a new scenario.
    pub grid_save: String,
    /// Before how many rows a grid has per page.
    pub grid_rows_per_page: String,
    /// Asks a server for a grid's rows again.
    pub grid_try_again: String,
    /// Before a page's number in a grid's pager.
    pub grid_page: String,
    /// A grid's pager goes back.
    pub grid_previous: String,
    /// A grid's pager goes on.
    pub grid_next: String,
    /// Opens the editor of a column's summary.
    pub grid_edit_summary: String,
    /// Before the column whose summary is being edited.
    pub grid_summary_of: String,
    /// No summary for a column.
    pub grid_summary_none: String,
    /// A column's summary from a formula.
    pub grid_summary_formula: String,
    /// What a formula looks like.
    pub grid_formula_hint: String,
    /// Before what a formula comes to.
    pub grid_formula_result: String,
    /// Where a formula could not be worked out.
    pub grid_formula_error: String,
    /// Before the columns a formula can use.
    pub grid_formula_columns: String,
    /// Before the functions a formula can use.
    pub grid_formula_functions: String,
    /// Puts a formula to use.
    pub grid_apply: String,
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

fn pairs<const N: usize>(words: [(&str, &str); N]) -> [(String, String); N] {
    words.map(|(one, many)| (one.to_owned(), many.to_owned()))
}

fn owned<const N: usize>(words: [&str; N]) -> [String; N] {
    words.map(str::to_owned)
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            close: "Close".into(),
            help: "Help".into(),
            navigation: "Navigation".into(),
            toggle_sidebar: "Toggle sidebar".into(),
            dismiss: "Dismiss".into(),
            send: "Send".into(),
            listen: "Listen".into(),
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
            copy: "Copy".into(),
            copied: "Copied".into(),
            diagnostics: "Diagnostics".into(),
            diagnostics_test: "Test performance".into(),
            diagnostics_testing: "Measuring performance".into(),
            diagnostics_testing_detail: "This takes 5 seconds. Leave the window as it is.".into(),
            diagnostics_test_good: "Performance is good".into(),
            diagnostics_test_fair: "Performance is fair".into(),
            diagnostics_test_poor: "Performance is poor".into(),
            diagnostics_test_fps: "frames per second".into(),
            diagnostics_test_cause_none: "The app should feel fluid on this computer.".into(),
            diagnostics_test_cause_gpu:
                "The time goes into drawing: this computer's graphics cannot redraw the window in time."
                    .into(),
            diagnostics_test_cause_interface: "The time goes into building the screen, on the processor."
                .into(),
            diagnostics_software: "Drawing on the CPU".into(),
            diagnostics_software_detail:
                "The graphics card is not being used, so the interface will be slow.".into(),
            diagnostics_unoptimized: "Built without optimizations".into(),
            diagnostics_unoptimized_detail: "Everything runs many times slower. Build with --release.".into(),
            diagnostics_hidden: "GPU left out for its driver".into(),
            diagnostics_no_gpu: "No usable GPU".into(),
            diagnostics_frame: "Frame".into(),
            diagnostics_update: "Update".into(),
            diagnostics_acquire: "Acquire".into(),
            diagnostics_render: "Render".into(),
            diagnostics_report_bug: "Report bug".into(),
            diagnostics_report_shares: "What a bug report shares".into(),
            diagnostics_report_shares_detail: "Opens a page in your browser with the text below, for you to \
                review and send yourself. Nothing is sent from here. It has the system's name, the GPU, \
                driver file names and frame times, with no personal folders and none of your data."
                .into(),
            diagnostics_report_failed: "Could not open the browser".into(),
            diagnostics_report_failed_detail: "Copy the text below and paste it into a new bug report."
                .into(),
            diagnostics_report_questions: "**What happened**\n\n\n**Steps to reproduce**\n1. \n\n\
                **What you expected**\n\n\n**Diagnostics report**"
                .into(),
            diagnostics_report_truncated: "The report was cut short to fit in the link. The diagnostics \
                panel has the whole text."
                .into(),
            release_versions: "Versions".into(),
            release_newer: "Newer".into(),
            release_older: "Older".into(),
            release_of: "of".into(),
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
            form_errors: "Please review the form".into(),
            clear: "Clear".into(),
            cancel: "Cancel".into(),
            more_commands: "More".into(),
            decimal_separator: '.',
            group_separator: ',',
            pick_time: "Pick a time".into(),
            step_up: "Increase".into(),
            step_down: "Decrease".into(),
            calendar_more: "more".into(),
            scheduler_resources: "Resources".into(),
            zoom_in: "Zoom in".into(),
            zoom_out: "Zoom out".into(),
            recurrence_name: "Repeats".into(),
            recurrence_frequencies: owned(["Once", "Daily", "Weekly", "Monthly", "Yearly"]),
            recurrence_every: "Every".into(),
            recurrence_units: pairs([
                ("day", "days"),
                ("week", "weeks"),
                ("month", "months"),
                ("year", "years"),
            ]),
            recurrence_each: owned(["Every day", "Every week", "Every month", "Every year"]),
            recurrence_ends_name: "Ends".into(),
            recurrence_ends: owned(["Never", "On a date", "After"]),
            recurrence_times: "times".into(),
            recurrence_until: "until".into(),
            recurrence_next: "Next".into(),
            grid_edit_summary: "Edit summary".into(),
            grid_summary_of: "Summary of".into(),
            grid_summary_none: "None".into(),
            grid_summary_formula: "Formula".into(),
            grid_formula_hint: "Sum([Seats]) / Count()".into(),
            grid_formula_result: "Result".into(),
            grid_formula_error: "Error".into(),
            grid_formula_columns: "Columns".into(),
            grid_formula_functions: "Functions".into(),
            grid_apply: "Apply".into(),
            grid_page: "Page".into(),
            grid_previous: "Previous".into(),
            grid_next: "Next".into(),
            grid_drag_hint: "Drag a column here to group by it".into(),
            grid_grouped_by: "Grouped by".into(),
            grid_remove_group: "Remove grouping by".into(),
            grid_expand_all: "Expand all".into(),
            grid_collapse_all: "Collapse all".into(),
            grid_sort_ascending: "Sort ascending".into(),
            grid_sort_descending: "Sort descending".into(),
            grid_clear_sort: "Clear sorting".into(),
            grid_group_by_column: "Group by this column".into(),
            grid_ungroup_column: "Ungroup".into(),
            grid_pin_column: "Pin to the left".into(),
            grid_unpin_column: "Unpin".into(),
            grid_hide_column: "Hide column".into(),
            grid_fit_column: "Fit width".into(),
            grid_show_panel: "Show group panel".into(),
            grid_hide_panel: "Hide group panel".into(),
            grid_column_menu: "Column menu".into(),
            grid_filter_values: "Filter by value".into(),
            grid_clear_filter: "Clear filter".into(),
            grid_filter_label: "Filter".into(),
            grid_all_values: "All".into(),
            grid_scenario: "Scenario".into(),
            grid_scenario_save: "Save scenario…".into(),
            grid_scenario_update: "Update".into(),
            grid_scenario_delete: "Delete".into(),
            grid_scenario_original: "Original layout".into(),
            grid_scenario_name: "Scenario name".into(),
            grid_save: "Save".into(),
            grid_rows_per_page: "Rows per page".into(),
            grid_try_again: "Try again".into(),
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
            help: "Ajuda".into(),
            navigation: "Navegação".into(),
            toggle_sidebar: "Alternar barra lateral".into(),
            dismiss: "Dispensar".into(),
            send: "Enviar".into(),
            listen: "Ouvir".into(),
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
            copy: "Copiar".into(),
            copied: "Copiado".into(),
            diagnostics: "Diagnóstico".into(),
            diagnostics_test: "Testar desempenho".into(),
            diagnostics_testing: "Medindo o desempenho".into(),
            diagnostics_testing_detail: "Leva 5 segundos. Deixe a janela como está.".into(),
            diagnostics_test_good: "Desempenho bom".into(),
            diagnostics_test_fair: "Desempenho razoável".into(),
            diagnostics_test_poor: "Desempenho ruim".into(),
            diagnostics_test_fps: "quadros por segundo".into(),
            diagnostics_test_cause_none: "O app deve parecer fluido neste computador.".into(),
            diagnostics_test_cause_gpu: "O tempo vai no desenho: a parte gráfica deste computador não consegue redesenhar a janela a tempo.".into(),
            diagnostics_test_cause_interface: "O tempo vai em montar a tela, no processador.".into(),
            diagnostics_software: "Desenhando na CPU".into(),
            diagnostics_software_detail:
                "A placa de vídeo não está sendo usada, então a interface fica lenta.".into(),
            diagnostics_unoptimized: "Compilado sem otimizações".into(),
            diagnostics_unoptimized_detail: "Tudo roda muitas vezes mais devagar. Compile com --release."
                .into(),
            diagnostics_hidden: "GPU deixada de fora pelo driver".into(),
            diagnostics_no_gpu: "Nenhuma GPU utilizável".into(),
            diagnostics_frame: "Quadro".into(),
            diagnostics_update: "Atualização".into(),
            diagnostics_acquire: "Aquisição".into(),
            diagnostics_render: "Render".into(),
            diagnostics_report_bug: "Relatar bug".into(),
            diagnostics_report_shares: "O que um relato de bug compartilha".into(),
            diagnostics_report_shares_detail: "Abre uma página no navegador com o texto abaixo, para você \
                revisar e enviar. Nada é enviado daqui. Ele traz o nome do sistema, a GPU, nomes de arquivos \
                de driver e tempos de quadro, sem pastas pessoais e sem nenhum dado seu."
                .into(),
            diagnostics_report_failed: "Não foi possível abrir o navegador".into(),
            diagnostics_report_failed_detail: "Copie o texto abaixo e cole em um novo relato de bug.".into(),
            diagnostics_report_questions: "**O que aconteceu**\n\n\n**Passos para reproduzir**\n1. \n\n\
                **O que você esperava**\n\n\n**Relatório de diagnóstico**"
                .into(),
            diagnostics_report_truncated: "O relatório foi cortado para caber no link. O painel de \
                diagnóstico tem o texto inteiro."
                .into(),
            release_versions: "Versões".into(),
            release_newer: "Mais recente".into(),
            release_older: "Mais antiga".into(),
            release_of: "de".into(),
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
            form_errors: "Revise o formulário".into(),
            clear: "Limpar".into(),
            cancel: "Cancelar".into(),
            more_commands: "Mais".into(),
            decimal_separator: ',',
            group_separator: '.',
            pick_time: "Escolher horário".into(),
            step_up: "Aumentar".into(),
            step_down: "Diminuir".into(),
            calendar_more: "mais".into(),
            scheduler_resources: "Recursos".into(),
            zoom_in: "Aproximar".into(),
            zoom_out: "Afastar".into(),
            recurrence_name: "Repetição".into(),
            recurrence_frequencies: owned(["Uma vez", "Diária", "Semanal", "Mensal", "Anual"]),
            recurrence_every: "A cada".into(),
            recurrence_units: pairs([
                ("dia", "dias"),
                ("semana", "semanas"),
                ("mês", "meses"),
                ("ano", "anos"),
            ]),
            recurrence_each: owned(["Todo dia", "Toda semana", "Todo mês", "Todo ano"]),
            recurrence_ends_name: "Termina".into(),
            recurrence_ends: owned(["Nunca", "Em uma data", "Depois de"]),
            recurrence_times: "vezes".into(),
            recurrence_until: "até".into(),
            recurrence_next: "Próximas".into(),
            grid_edit_summary: "Editar resumo".into(),
            grid_summary_of: "Resumo de".into(),
            grid_summary_none: "Nenhum".into(),
            grid_summary_formula: "Fórmula".into(),
            grid_formula_hint: "Sum([Licenças]) / Count()".into(),
            grid_formula_result: "Resultado".into(),
            grid_formula_error: "Erro".into(),
            grid_formula_columns: "Colunas".into(),
            grid_formula_functions: "Funções".into(),
            grid_apply: "Aplicar".into(),
            grid_page: "Página".into(),
            grid_previous: "Anterior".into(),
            grid_next: "Próxima".into(),
            grid_drag_hint: "Arraste uma coluna para cá para agrupar".into(),
            grid_grouped_by: "Agrupado por".into(),
            grid_remove_group: "Desagrupar".into(),
            grid_expand_all: "Expandir tudo".into(),
            grid_collapse_all: "Recolher tudo".into(),
            grid_sort_ascending: "Ordenar crescente".into(),
            grid_sort_descending: "Ordenar decrescente".into(),
            grid_clear_sort: "Limpar ordenação".into(),
            grid_group_by_column: "Agrupar por esta coluna".into(),
            grid_ungroup_column: "Desagrupar esta coluna".into(),
            grid_pin_column: "Fixar à esquerda".into(),
            grid_unpin_column: "Desafixar".into(),
            grid_hide_column: "Ocultar coluna".into(),
            grid_fit_column: "Ajustar largura".into(),
            grid_show_panel: "Mostrar painel de agrupamento".into(),
            grid_hide_panel: "Ocultar painel de agrupamento".into(),
            grid_column_menu: "Menu da coluna".into(),
            grid_filter_values: "Filtrar por valor".into(),
            grid_clear_filter: "Limpar filtro".into(),
            grid_filter_label: "Filtro".into(),
            grid_all_values: "Todos".into(),
            grid_scenario: "Cenário".into(),
            grid_scenario_save: "Gravar cenário…".into(),
            grid_scenario_update: "Atualizar".into(),
            grid_scenario_delete: "Excluir".into(),
            grid_scenario_original: "Layout original".into(),
            grid_scenario_name: "Nome do cenário".into(),
            grid_save: "Gravar".into(),
            grid_rows_per_page: "Linhas por página".into(),
            grid_try_again: "Tentar de novo".into(),
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

    /// The word for one period of this frequency; `count` picks singular or plural.
    pub fn recurrence_unit(&self, frequency: Frequency, count: u32) -> &str {
        let unit = match frequency {
            Frequency::Daily => &self.recurrence_units[0],
            Frequency::Weekly => &self.recurrence_units[1],
            Frequency::Yearly => &self.recurrence_units[3],
            _ => &self.recurrence_units[2],
        };
        if count > 1 { &unit.1 } else { &unit.0 }
    }

    /// A rule in words, with its next days: `Every month, 3 times. Next: 05/11/2026, 05/12/2026`.
    pub fn recurrence_summary(&self, rule: &RecurrenceRule, start: Date) -> String {
        let how = match (rule.frequency, rule.interval.max(1)) {
            (Frequency::Daily, 1) => self.recurrence_each[0].clone(),
            (Frequency::Weekly, 1) => self.recurrence_each[1].clone(),
            (Frequency::Monthly, 1) => self.recurrence_each[2].clone(),
            (Frequency::Yearly, 1) => self.recurrence_each[3].clone(),
            (Frequency::Daily | Frequency::Weekly | Frequency::Monthly | Frequency::Yearly, n) => {
                format!("{} {n} {}", self.recurrence_every, self.recurrence_unit(rule.frequency, n))
            }
            _ => self.recurrence_frequencies[0].clone(),
        };
        let until = match rule.end {
            RecurrenceEnd::Until(day) => format!(" {} {}", self.recurrence_until, (self.short_date)(day)),
            RecurrenceEnd::After(count) => format!(", {count} {}", self.recurrence_times),
            _ => String::new(),
        };
        let next: Vec<String> = rule.days_from(start).take(3).map(self.short_date).collect();
        format!("{how}{until}. {}: {}", self.recurrence_next, next.join(", "))
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
