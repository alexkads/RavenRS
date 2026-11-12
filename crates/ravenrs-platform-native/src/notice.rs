//! The system's own notifications, for what must reach someone outside the window.

/// Shows notifications in the system's own place: the Notification Center on macOS,
/// a toast on Windows and the notification server on Linux, which plain GNOME has
/// even without a tray.
///
/// Use it for what must reach the person while they are looking elsewhere: a reminder,
/// a long export that finished. Inside the window, a toast of `ravenrs-widgets` is better.
///
/// Needs the `notifications` feature. Without it, the type still exists, so the app
/// compiles the same, and [`send`](Self::send) returns an error.
///
/// ```no_run
/// use ravenrs_platform_native::SystemNotifier;
///
/// let notifier = SystemNotifier::new("Notes").bundle_id("com.example.notes");
/// std::thread::spawn(move || notifier.send("Saved", "Your notes are safe."));
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(not(feature = "notifications"), allow(dead_code, reason = "nothing to show them with"))]
pub struct SystemNotifier {
    name: String,
    bundle_id: Option<String>,
    window: Option<String>,
}

impl SystemNotifier {
    /// Notifications from the app called `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), bundle_id: None, window: None }
    }

    /// The identifier in the app's `Info.plist`. On macOS it says whose the
    /// notifications are; without one the system stops to ask for an app.
    pub fn bundle_id(mut self, id: impl Into<String>) -> Self {
        self.bundle_id = Some(id.into());
        self
    }

    /// On Linux, a click on the notification brings back the window of the running
    /// app with this [`AppIdentity`](crate::AppIdentity) id, if it closes to the tray.
    /// The other systems do not tell of the click.
    pub fn opens_window(mut self, id: impl Into<String>) -> Self {
        self.window = Some(id.into());
        self
    }

    /// Shows one. It waits for the system to answer, so call it away from the UI
    /// thread. `Ok` means the system took it: the person may still have them turned off.
    pub fn send(&self, title: &str, body: &str) -> Result<(), String> {
        system::send(self, title, body)
    }
}

#[cfg(feature = "notifications")]
mod system {
    use super::SystemNotifier;

    pub(super) fn send(notifier: &SystemNotifier, title: &str, body: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        if let Some(id) = &notifier.bundle_id {
            // Refused outside the installed app, and then it goes out in Finder's name.
            let _ = notify_rust::set_application(id);
        }
        let mut notification = notify_rust::Notification::new();
        notification.appname(&notifier.name).summary(title).body(body);
        show(notifier, notification)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn show(notifier: &SystemNotifier, mut notification: notify_rust::Notification) -> Result<(), String> {
        notification.action("default", &notifier.name);
        let handle = notification.show().map_err(|e| e.to_string())?;
        if let Some(id) = notifier.window.clone() {
            // Waiting for the click lasts as long as the notification does.
            std::thread::spawn(move || {
                handle.wait_for_action(|action| {
                    if action == "default" {
                        crate::tray::ask_running_copy(&id);
                    }
                });
            });
        }
        Ok(())
    }

    #[cfg(not(all(unix, not(target_os = "macos"))))]
    fn show(_: &SystemNotifier, notification: notify_rust::Notification) -> Result<(), String> {
        notification.show().map(|_| ()).map_err(|e| e.to_string())
    }
}

#[cfg(not(feature = "notifications"))]
mod system {
    use super::SystemNotifier;

    pub(super) fn send(_: &SystemNotifier, _: &str, _: &str) -> Result<(), String> {
        Err("this build has no system notifications".to_owned())
    }
}
