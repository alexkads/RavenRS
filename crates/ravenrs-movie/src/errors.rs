//! The last errors of the app, to go along with a bug report.

use std::collections::VecDeque;
use std::fmt::{self, Write as _};
use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// How many it keeps: what led to a bug is in the last ones.
const KEPT: usize = 30;

/// Keeps the last warnings and errors logged with `tracing`, in memory only, to give
/// to [`BugReporter::errors`](crate::BugReporter::errors), where the person sees them
/// before anything goes. Add it to the app's subscriber as a layer.
///
/// ```
/// use ravenrs_movie::RecentErrors;
/// use tracing_subscriber::layer::SubscriberExt as _;
///
/// let errors = RecentErrors::default();
/// let subscriber = tracing_subscriber::registry().with(errors.clone());
/// tracing::subscriber::with_default(subscriber, || tracing::error!(code = 7, "could not save"));
/// assert_eq!(errors.lines(), ["could not save code=7"]);
/// ```
#[derive(Clone, Debug, Default)]
pub struct RecentErrors(Arc<Mutex<VecDeque<String>>>);

impl RecentErrors {
    /// What it has, oldest first, with the person's home folder written as `~`, so
    /// that a path does not say whose computer this is.
    pub fn lines(&self) -> Vec<String> {
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
        let Ok(lines) = self.0.lock() else {
            return Vec::new();
        };
        lines
            .iter()
            .map(|line| if home.is_empty() { line.clone() } else { line.replace(&home, "~") })
            .collect()
    }
}

/// The message first, then the fields.
#[derive(Default)]
struct Line {
    message: String,
    fields: String,
}

impl Visit for Line {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.fields, " {}={value:?}", field.name());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            let _ = write!(self.fields, " {}={value}", field.name());
        }
    }
}

impl<S: Subscriber> Layer<S> for RecentErrors {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        if *event.metadata().level() > Level::WARN {
            return;
        }
        let mut line = Line::default();
        event.record(&mut line);
        // Logging from a thread that panicked holding it is not worth a second panic.
        if let Ok(mut lines) = self.0.lock() {
            if lines.len() == KEPT {
                lines.pop_front();
            }
            lines.push_back(line.message + &line.fields);
        }
    }
}
