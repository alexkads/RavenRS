//! Background work on the screen: what it hands back is kept each time it wakes the screen, and
//! what runs shows how far it got, with a way to stop it.

use std::sync::Arc;

use ravenrs_app::Job;
use ravenrs_layout::Align;
use ravenrs_style::TextStyle;
use ravenrs_widgets::{
    AnyView, BuildCx, Button, ButtonSize, ButtonStyle, HStack, ProgressBar, Spinner, Text, VStack,
};

use crate::session::Session;

/// Keeps what the frame's background work hands back, from the build of what hosts the frame's
/// windows, with no element of its own: it is woken, keeps it, and lets what shows records read
/// again.
pub(crate) fn keep(cx: &mut BuildCx<'_>, session: &Session) {
    let frame = session.frame();
    let waker = cx.ui_waker();
    // Once for the frame: the work wakes this screen from its own thread.
    let wired = frame.clone();
    cx.use_state(move || wired.set_job_waker(Arc::new(move || waker.wake())));
    if frame.apply_jobs() {
        session.after_background_work();
    }
}

/// What runs now, each with how far it got and a button that asks it to stop. Nothing while
/// nothing runs.
pub(crate) fn running(session: &Session) -> Option<AnyView> {
    let jobs = session.frame().jobs();
    if jobs.is_empty() {
        return None;
    }
    let words = session.words();
    let shown: Vec<AnyView> = jobs.into_iter().map(|job| one(&job, words)).collect();
    Some(Box::new(VStack::new(shown).spacing(8.0).align(Align::Stretch)))
}

fn one(job: &Job, words: &crate::Words) -> AnyView {
    let (done, doing) = job.progress();
    let stopping = job.is_cancelled();
    let doing = match (stopping, doing.is_empty()) {
        (true, _) => words.stopping.clone(),
        (false, true) => words.working.clone(),
        (false, false) => doing,
    };
    let progress: AnyView = match done {
        Some(done) => Box::new(ProgressBar::new(done).label(job.caption()).detail(doing).percent()),
        None => Box::new(
            VStack::new((
                Text::new(job.caption()).style(TextStyle::Caption),
                HStack::new((Spinner::new().size(14.0), Text::new(doing).style(TextStyle::Caption)))
                    .spacing(6.0),
            ))
            .spacing(4.0),
        ),
    };
    let asking = job.clone();
    let cancel = (!stopping).then(|| {
        Button::new(words.cancel.clone())
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Small)
            .on_click(move || asking.cancel())
    });
    Box::new(VStack::new((progress, cancel)).spacing(4.0).align(Align::Stretch))
}
