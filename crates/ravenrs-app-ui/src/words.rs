//! [`Words`]: what the generated screens say, in the application's language.

use ravenrs_app::RuleWords;
use ravenrs_app_i18n::{Language, Texts};

/// The words the generated screens use.
#[derive(Clone, Debug)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Words {
    /// Lists the keys of what can be done.
    pub shortcuts: String,
    /// Gets back to a locked window.
    pub unlock: String,
    /// What was typed on a locked window is not it.
    pub unlock_wrong: String,
    /// Creates a record.
    pub new: String,
    /// Removes what is selected.
    pub delete: String,
    /// Keeps the changes.
    pub save: String,
    /// Keeps the changes and leaves.
    pub save_and_close: String,
    /// Keeps the changes and starts another.
    pub save_and_new: String,
    /// Throws away the changes and leaves.
    pub cancel: String,
    /// The record before.
    pub previous: String,
    /// The record after.
    pub next: String,
    /// Reads again.
    pub refresh: String,
    /// Goes to the screen before.
    pub back: String,
    /// The question before removing.
    pub delete_question: String,
    /// What the removal does.
    pub delete_detail: String,
    /// What the question before any other action says besides: neutral, since it can be changed
    /// again.
    pub confirm_detail: String,
    /// The question before losing changes.
    pub discard_question: String,
    /// What losing changes does.
    pub discard_detail: String,
    /// Agrees to lose them.
    pub discard: String,
    /// A record being created.
    pub new_record: String,
    /// The menu of the ways to see a list.
    pub variant: String,
    /// Puts an existing record on a list inside a form.
    pub link: String,
    /// Takes a record off a list inside a form, without removing it.
    pub unlink: String,
    /// Where a list inside a form would be, while its record is not kept yet.
    pub save_first: String,
    /// The menu of the filters ready on a list.
    pub filter: String,
    /// Makes a new record from the one selected.
    pub clone: String,
    /// Why a record cannot be cloned while it has changes.
    pub clone_unsaved: String,
    /// Takes a file out, to wherever people say.
    pub save_file: String,
    /// The question when rules only warn.
    pub warning_question: String,
    /// Goes on in spite of what rules warn about.
    pub warning_answer: String,
    /// The question when someone else changed the same fields, where `{0}` is which ones.
    pub conflict_question: String,
    /// What keeping one's own changes does, where `{0}` is which fields.
    pub conflict_detail: String,
    /// Keeps one's own changes over the other person's.
    pub conflict_answer: String,
    /// What a way of showing a list says when something it needs is missing.
    pub list_editor_needs: String,
    /// A row of a timeline for records that point to no resource.
    pub no_resource: String,
    /// The series of a chart that counts records.
    pub chart_count: String,
    /// Things to be reminded of.
    pub reminders: String,
    /// Background work that cannot say what it is doing.
    pub working: String,
    /// Background work asked to stop, until it does.
    pub stopping: String,
    /// Reminds again later.
    pub snooze: String,
    /// Stops reminding.
    pub dismiss: String,
    /// Opens what a reminder is about.
    pub open: String,
    /// A field that is on.
    pub yes: String,
    /// A field that is off.
    pub no: String,
    /// What people are told when something is refused.
    pub access_denied: String,
    /// What shows in place of a field the person cannot read.
    pub protected_value: String,
    /// Enters the application.
    pub sign_in: String,
    /// Leaves the application.
    pub sign_out: String,
    /// Says signing in did not work, without saying why.
    pub sign_in_failed: String,
    /// The name someone signs in with.
    pub user_name: String,
    /// Their password.
    pub password: String,
    /// The password they have now.
    pub current_password: String,
    /// The password they want.
    pub new_password: String,
    /// The same again.
    pub repeat_password: String,
    /// Signing in could not be tried.
    pub sign_in_unavailable: String,
    /// The two are not the same.
    pub passwords_differ: String,
    /// It is too short.
    pub password_too_short: String,
    /// The current one is not right.
    pub password_wrong_current: String,
    /// When the new password is the one they already have.
    pub password_same: String,
    /// It was changed.
    pub password_saved: String,
    /// Keeps the new password.
    pub save_password: String,
    /// Changes one's own password.
    pub change_password: String,
    /// Gives someone a password.
    pub set_password: String,
    /// Why they have to choose one now.
    pub must_change_password: String,
    /// What the first screen of a new installation says.
    pub first_user: String,
    /// Creates the first person.
    pub create_user: String,
    /// The editor of the application's model.
    pub model_editor: String,
    /// Looks for a node of the model.
    pub search_model: String,
    /// What shows before a node is picked.
    pub pick_model_node: String,
    /// Applies what was typed.
    pub apply: String,
    /// Throws away the person's own changes to the model.
    pub restore_model: String,
    /// Why what is on a form cannot be kept any more.
    pub record_gone: String,
    /// What fails a rule.
    pub rules: RuleWords,
    /// In front of money.
    pub money_prefix: String,
    /// Between thousands.
    pub thousands: char,
    /// Before decimals.
    pub decimal: char,
    /// How a date shows in lists, such as `dd/MM/yyyy`.
    pub date_format: String,
    /// How a date with a time shows in lists, such as `dd/MM/yyyy HH:mm`.
    pub date_time_format: String,
}

impl Default for Words {
    fn default() -> Self {
        Self::from_texts(&Texts::new(Language::English))
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Words {
    /// The texts of a catalog, in its language. What it does not have comes from English.
    pub fn from_texts(texts: &Texts) -> Self {
        Self {
            shortcuts: texts.get("ui.shortcuts"),
            unlock: texts.get("ui.unlock"),
            unlock_wrong: texts.get("ui.unlock_wrong"),
            new: texts.get("ui.new"),
            delete: texts.get("ui.delete"),
            save: texts.get("ui.save"),
            save_and_close: texts.get("ui.save_and_close"),
            save_and_new: texts.get("ui.save_and_new"),
            cancel: texts.get("ui.cancel"),
            previous: texts.get("ui.previous"),
            next: texts.get("ui.next"),
            refresh: texts.get("ui.refresh"),
            back: texts.get("ui.back"),
            delete_question: texts.get("ui.delete_question"),
            delete_detail: texts.get("ui.delete_detail"),
            confirm_detail: texts.get("ui.confirm_detail"),
            discard_question: texts.get("ui.discard_question"),
            discard_detail: texts.get("ui.discard_detail"),
            discard: texts.get("ui.discard"),
            new_record: texts.get("ui.new_record"),
            variant: texts.get("ui.variant"),
            link: texts.get("ui.link"),
            unlink: texts.get("ui.unlink"),
            save_first: texts.get("ui.save_first"),
            filter: texts.get("ui.filter"),
            clone: texts.get("ui.clone"),
            clone_unsaved: texts.get("ui.clone_unsaved"),
            save_file: texts.get("ui.save_file"),
            warning_question: texts.get("ui.warning_question"),
            warning_answer: texts.get("ui.warning_answer"),
            conflict_question: texts.get("ui.conflict_question"),
            conflict_detail: texts.get("ui.conflict_detail"),
            conflict_answer: texts.get("ui.conflict_answer"),
            list_editor_needs: texts.get("ui.list_editor_needs"),
            no_resource: texts.get("ui.no_resource"),
            chart_count: texts.get("ui.chart_count"),
            reminders: texts.get("ui.reminders"),
            working: texts.get("ui.working"),
            stopping: texts.get("ui.stopping"),
            snooze: texts.get("ui.snooze"),
            dismiss: texts.get("ui.dismiss"),
            open: texts.get("ui.open"),
            yes: texts.get("ui.yes"),
            no: texts.get("ui.no"),
            access_denied: texts.get("ui.access_denied"),
            protected_value: texts.get("ui.protected_value"),
            sign_in: texts.get("ui.sign_in"),
            sign_out: texts.get("ui.sign_out"),
            sign_in_failed: texts.get("ui.sign_in_failed"),
            user_name: texts.get("ui.user_name"),
            password: texts.get("ui.password"),
            current_password: texts.get("ui.current_password"),
            new_password: texts.get("ui.new_password"),
            repeat_password: texts.get("ui.repeat_password"),
            sign_in_unavailable: texts.get("ui.sign_in_unavailable"),
            passwords_differ: texts.get("ui.passwords_differ"),
            password_too_short: texts.get("ui.password_too_short"),
            password_same: texts.get("ui.password_same"),
            password_wrong_current: texts.get("ui.password_wrong_current"),
            password_saved: texts.get("ui.password_saved"),
            save_password: texts.get("ui.save_password"),
            change_password: texts.get("ui.change_password"),
            set_password: texts.get("ui.set_password"),
            must_change_password: texts.get("ui.must_change_password"),
            first_user: texts.get("ui.first_user"),
            create_user: texts.get("ui.create_user"),
            model_editor: texts.get("ui.model_editor"),
            search_model: texts.get("ui.search_model"),
            pick_model_node: texts.get("ui.pick_model_node"),
            apply: texts.get("ui.apply"),
            restore_model: texts.get("ui.restore_model"),
            record_gone: texts.get("ui.record_gone"),
            rules: RuleWords::from_texts(texts),
            money_prefix: texts.get("format.money_prefix"),
            thousands: texts.char("format.thousands", ','),
            decimal: texts.char("format.decimal", '.'),
            date_format: texts.get("format.date"),
            date_time_format: texts.get("format.date_time"),
        }
    }

    /// In Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self::from_texts(&Texts::new(Language::BrazilianPortuguese))
    }
}
