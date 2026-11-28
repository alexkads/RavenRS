//! The actions and the screen of a period closing: approve with the document, reopen with a reason
//! and open the versions kept, each checked against its hash.

use std::rc::Rc;

use ravenrs_app::{
    Action, ActionCategory, ActionInput, AttachmentStore, CLOSING_CURRENT, FieldKind, Message, MessageKind,
    Module, Object, ObjectSpace, Operation, Selection, Value, ViewController, approve, archived_document,
    closing_versions, reopen,
};
use ravenrs_app_ui::Session;
use ravenrs_report::Document;
use ravenrs_report_export::ExportFormat;
use ravenrs_widgets::{
    Alert, AnyView, Badge, BuildCx, Button, ButtonSize, ButtonStyle, HStack, NodeId, Text, Tone, VStack,
    View, ViewExt,
};

use crate::ReportWords;

/// What makes the document a period is approved with, from the period as the space reads it.
pub type ClosingDocument = dyn Fn(&dyn ObjectSpace, &Object) -> Result<Document, String>;

/// Gives a period entity, declared with `EntityInfo::period_closing`, the actions "Approve
/// closing", "Reopen closing" and "Archived versions", on one record of its lists and forms.
///
/// Approving renders the document, keeps it as PDF in `files` and writes the next version in one
/// commit; the commit itself refuses it while a record keeps the period open. Reopening asks for
/// the reason. Each action needs `Write` on the period, so a role without it does not see them.
/// The versions are read through the space, so whoever cannot read them is denied, and a document
/// that does not match its hash is not given.
///
/// ```no_run
/// use std::rc::Rc;
/// use ravenrs_app::MemoryAttachmentStore;
/// use ravenrs_app_report::ClosingModule;
/// use ravenrs_report::Document;
///
/// let module = ClosingModule::new("Term", Rc::new(MemoryAttachmentStore::default()), |_space, term| {
///     let _ = term;
///     Ok(Document::default())
/// })
/// .approver(|| "ana".to_owned());
/// # let _ = module;
/// ```
#[derive(Clone)]
pub struct ClosingModule {
    period: String,
    files: Rc<dyn AttachmentStore>,
    document: Rc<ClosingDocument>,
    approver: Rc<dyn Fn() -> String>,
    words: ReportWords,
}

impl std::fmt::Debug for ClosingModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClosingModule").field("period", &self.period).finish_non_exhaustive()
    }
}

impl ClosingModule {
    /// The actions of the closing of `period`, which keep the documents in `files`.
    pub fn new(
        period: impl Into<String>,
        files: Rc<dyn AttachmentStore>,
        document: impl Fn(&dyn ObjectSpace, &Object) -> Result<Document, String> + 'static,
    ) -> Self {
        Self {
            period: period.into(),
            files,
            document: Rc::new(document),
            approver: Rc::new(String::new),
            words: ReportWords::default(),
        }
    }

    /// Who approves, as kept in the version: the person signed in, for one.
    pub fn approver(mut self, who: impl Fn() -> String + 'static) -> Self {
        self.approver = Rc::new(who);
        self
    }

    /// What its actions and screen say.
    pub fn words(mut self, words: ReportWords) -> Self {
        self.words = words;
        self
    }

    /// The ids of its actions: approve, reopen and the archived versions.
    pub fn action_ids(&self) -> [String; 3] {
        let period = &self.period;
        [
            format!("{period}.ApproveClosing"),
            format!("{period}.ReopenClosing"),
            format!("{period}.ClosingVersions"),
        ]
    }

    fn actions(&self) -> Vec<Action> {
        let [approve_id, reopen_id, versions_id] = self.action_ids();
        let on_one = |action: Action| {
            action
                .category(ActionCategory::Edit)
                .target_entity(self.period.clone())
                .selection(Selection::RequireSingle)
                .requires(Operation::Write)
        };
        let me = self.clone();
        let approving = on_one(Action::simple(approve_id, self.words.closing_approve.clone()))
            .icon("lock")
            .confirmation(self.words.closing_confirm.clone())
            .on_execute(move |cx| {
                let period = cx.selected.first().ok_or("this action needs a period")?;
                let key = me.key_of(cx.space.borrow().entities(), period)?;
                let document = (me.document)(&*cx.space.borrow(), period)?;
                let pdf = ravenrs_report_export::export(&document, ExportFormat::Pdf, None)
                    .map_err(|e| e.to_string())?;
                let version = approve(
                    &mut *cx.space.borrow_mut(),
                    &*me.files,
                    &me.period,
                    &key,
                    &pdf.bytes,
                    &(me.approver)(),
                )
                .map_err(|e| e.to_string())?;
                let number = version.integer("version").unwrap_or_default();
                me.tell(cx, format!("{} {number}.", me.words.closing_approved));
                Ok(())
            });
        let me = self.clone();
        let reopening =
            on_one(Action::parametrized(reopen_id, self.words.closing_reopen.clone(), FieldKind::LongText))
                .icon("unlock")
                .on_execute(move |cx| {
                    let period = cx.selected.first().ok_or("this action needs a period")?;
                    let key = me.key_of(cx.space.borrow().entities(), period)?;
                    let reason = match &cx.input {
                        ActionInput::Parameter(Value::Text(reason)) => reason.clone(),
                        _ => String::new(),
                    };
                    reopen(&mut *cx.space.borrow_mut(), &me.period, &key, &reason)
                        .map_err(|e| e.to_string())?;
                    me.tell(cx, me.words.closing_reopened_done.clone());
                    Ok(())
                });
        let me = self.clone();
        let versions = Action::simple(versions_id, self.words.closing_documents.clone())
            .category(ActionCategory::View)
            .target_entity(self.period.clone())
            .selection(Selection::RequireSingle)
            .icon("archive")
            .on_execute(move |cx| {
                let session = Session::of(cx).ok_or("this action needs the screens")?;
                let period = cx.selected.first().ok_or("this action needs a period")?;
                let key = me.key_of(cx.space.borrow().entities(), period)?;
                let screen = VersionsScreen { module: me.clone(), session: session.clone(), key };
                session
                    .present(me.words.closing_documents.clone(), move || Box::new(screen.clone()) as AnyView);
                Ok(())
            });
        vec![approving, reopening, versions]
    }

    fn key_of(&self, entities: Vec<ravenrs_app::EntityInfo>, period: &Object) -> Result<Value, String> {
        let info =
            entities.iter().find(|entity| entity.name == self.period).ok_or("the period is not an entity")?;
        let field = info.key_field().ok_or("the period has no key")?;
        Ok(period.get(&field.name))
    }

    fn tell(&self, cx: &ravenrs_app::ActionContext, said: String) {
        if let Some(session) = Session::of(cx) {
            session.records_changed();
            session.show_message(Message::new(said, MessageKind::Success));
        }
    }
}

/// The versions of one period, with their status, hash and document.
#[derive(Clone)]
struct VersionsScreen {
    module: ClosingModule,
    session: Session,
    key: Value,
}

impl VersionsScreen {
    /// What a version is: current, reopened as the last one, or replaced by a later one.
    fn status(&self, version: &Object, last: bool) -> (String, Tone) {
        let words = &self.module.words;
        if version.text("status") == CLOSING_CURRENT {
            (words.closing_current.clone(), Tone::Success)
        } else if last {
            (words.closing_reopened.clone(), Tone::Warning)
        } else {
            (words.closing_replaced.clone(), Tone::Neutral)
        }
    }
}

impl View for VersionsScreen {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let words = &self.module.words;
        let space = self.session.frame().space();
        let read = closing_versions(&*space.borrow(), &self.module.period, &self.key);
        let content: AnyView = match read {
            Err(error) => {
                Box::new(Alert::new(words.closing_documents.clone(), error.to_string()).tone(Tone::Danger))
            }
            Ok(versions) if versions.is_empty() => Box::new(Text::new(words.closing_none.clone())),
            Ok(versions) => {
                let picker = cx.file_picker();
                let count = versions.len();
                let rows: Vec<AnyView> = versions
                    .iter()
                    .enumerate()
                    .map(|(at, version)| {
                        let (status, tone) = self.status(version, at + 1 == count);
                        let number = version.integer("version").unwrap_or_default();
                        let digest: String = version.text("digest").chars().take(12).collect();
                        let approved = {
                            let at = match version.get("approved_at") {
                                Value::Date(at) => at.to_string(),
                                _ => String::new(),
                            };
                            format!("{} · {at}", version.text("approved_by"))
                        };
                        let (session, module, picker) =
                            (self.session.clone(), self.module.clone(), picker.clone());
                        let version_key = version.get("id");
                        let name = format!("{}-{}-v{number}.pdf", module.period, version.text("period"));
                        let open = Button::new(words.closing_open.clone())
                            .style(ButtonStyle::Secondary)
                            .size(ButtonSize::Small)
                            .on_click(move || {
                                let space = session.frame().space();
                                let versions =
                                    ravenrs_app::closing_versions_entity(&*space.borrow(), &module.period);
                                let read = versions.and_then(|versions| {
                                    archived_document(
                                        &*space.borrow(),
                                        &*module.files,
                                        &versions,
                                        &version_key,
                                    )
                                });
                                match read {
                                    Ok(bytes) => session.hand_file(name.clone(), bytes, &picker),
                                    Err(error) => {
                                        session.show_message(Message::new(
                                            error.to_string(),
                                            MessageKind::Error,
                                        ));
                                    }
                                }
                            });
                        Box::new(
                            HStack::new((
                                Text::new(format!("{} {number}", words.closing_version)),
                                Text::new(approved).grow(1.0),
                                Text::new(digest),
                                Badge::new(status).tone(tone),
                                open,
                            ))
                            .spacing(12.0),
                        ) as AnyView
                    })
                    .collect();
                Box::new(VStack::new(rows).spacing(8.0))
            }
        };
        content.padding(16.0).build(cx)
    }
}

impl ViewController for ClosingModule {
    fn id(&self) -> &str {
        "PeriodClosing"
    }

    fn actions(&self) -> Vec<Action> {
        ClosingModule::actions(self)
    }
}

impl Module for ClosingModule {
    fn name(&self) -> &str {
        "PeriodClosing"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(self.clone())]
    }
}
