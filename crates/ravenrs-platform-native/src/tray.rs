//! The icon beside the clock that keeps an app running with its window closed.
//!
//! Each system has its own way of showing it. macOS and Windows use `tray-icon`, on
//! the thread of the event loop. Linux uses `ksni`, a StatusNotifierItem in pure Rust,
//! so that building an app asks for no GTK; there the icon only appears if something
//! hosts it, which plain GNOME does not. That is why [`show`] can fail, and why
//! whoever calls it then closes the app: an app hidden behind an icon nobody draws
//! could never be closed again.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use ravenrs_core::ImageData;

/// Keeps the app running beside the clock when its window is closed. Give it to
/// [`AppIdentity::close_to_tray`](crate::AppIdentity::close_to_tray).
///
/// The menu has two items: one opens the window again, in the state it was left,
/// and the other really closes the app. Where the system shows no such icon, such as
/// plain GNOME, closing the window closes the app as usual. Needs the `tray` feature.
///
/// ```
/// use ravenrs_platform_native::SystemTray;
///
/// let tray = SystemTray::new("Notes").open("Open Notes").quit("Quit");
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(not(feature = "tray"), allow(dead_code, reason = "no icon to show them on"))]
pub struct SystemTray {
    name: String,
    open: String,
    quit: String,
}

impl SystemTray {
    /// An icon for the app called `name`, which is also its tooltip.
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self { open: format!("Open {name}"), quit: "Quit".to_owned(), name }
    }

    /// Sets the words of the item that opens the window again.
    pub fn open(mut self, label: impl Into<String>) -> Self {
        self.open = label.into();
        self
    }

    /// Sets the words of the item that closes the app.
    pub fn quit(mut self, label: impl Into<String>) -> Self {
        self.quit = label.into();
        self
    }
}

/// What the person asked for, from the icon or from opening the app a second time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(feature = "tray"), allow(dead_code, reason = "only the icon offers to quit"))]
pub(crate) enum TrayAction {
    Open,
    Quit,
}

/// Runs on whatever thread the system answers on.
pub(crate) type Notify = Arc<dyn Fn(TrayAction) + Send + Sync>;

/// Puts the icon on screen, until the result is dropped. `Err` says why there is none.
pub(crate) fn show(tray: &SystemTray, id: &str, icon: &ImageData, notify: Notify) -> Result<Shown, String> {
    system::show(tray, id, icon, notify).map(Shown)
}

/// An icon on screen.
pub(crate) struct Shown(#[allow(dead_code, reason = "holding it is what keeps the icon")] system::Icon);

#[cfg(all(feature = "tray", any(target_os = "macos", target_os = "windows")))]
mod system {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{TrayIcon, TrayIconBuilder};

    use super::{ImageData, Notify, SystemTray, TrayAction};

    const OPEN: &str = "ravenrs:open";
    const QUIT: &str = "ravenrs:quit";

    pub(super) type Icon = TrayIcon;

    pub(super) fn show(
        tray: &SystemTray,
        _id: &str,
        icon: &ImageData,
        notify: Notify,
    ) -> Result<Icon, String> {
        // Only the first one stays, and they all say the same.
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id.as_ref() {
            OPEN => notify(TrayAction::Open),
            QUIT => notify(TrayAction::Quit),
            _ => {}
        }));
        let open = MenuItem::with_id(OPEN, &tray.open, true, None);
        let quit = MenuItem::with_id(QUIT, &tray.quit, true, None);
        let menu =
            Menu::with_items(&[&open, &PredefinedMenuItem::separator(), &quit]).map_err(|e| e.to_string())?;
        let image = tray_icon::Icon::from_rgba(icon.pixels().to_vec(), icon.width(), icon.height())
            .map_err(|e| e.to_string())?;
        TrayIconBuilder::new()
            .with_icon(image)
            .with_tooltip(&tray.name)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(true)
            .build()
            .map_err(|e| e.to_string())
    }
}

#[cfg(all(feature = "tray", target_os = "linux"))]
#[path = "tray_linux.rs"]
mod system;

#[cfg(not(all(feature = "tray", any(target_os = "macos", target_os = "windows", target_os = "linux"))))]
mod system {
    use super::{ImageData, Notify, SystemTray};

    pub(super) type Icon = ();

    pub(super) fn show(_: &SystemTray, _: &str, _: &ImageData, _: Notify) -> Result<Icon, String> {
        Err("this build has no tray icon".to_owned())
    }
}

/// Whether the system has this app in front, where it can tell. A click on the app's
/// icon in the Dock brings it to the front, and macOS says nothing else that winit hears.
#[cfg(all(feature = "tray", target_os = "macos"))]
pub(crate) fn app_is_active() -> Option<bool> {
    let thread = objc2::MainThreadMarker::new()?;
    Some(objc2_app_kit::NSApplication::sharedApplication(thread).isActive())
}

#[cfg(not(all(feature = "tray", target_os = "macos")))]
pub(crate) fn app_is_active() -> Option<bool> {
    None
}

/// How long one running copy of the app takes to answer another, at most.
const ANSWER_TIME: Duration = Duration::from_millis(500);

/// Tells a copy of the app already running to open its window. `true` if there is one.
pub(crate) fn ask_running_copy(id: &str) -> bool {
    let (address, token) = (instance_address(id), instance_token(id));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, ANSWER_TIME) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(ANSWER_TIME));
    let _ = stream.set_write_timeout(Some(ANSWER_TIME));
    if stream.write_all(token.as_bytes()).is_err() {
        return false;
    }
    let mut answer = vec![0; token.len()];
    // Something else on this port does not know the token, and is none of our business.
    stream.read_exact(&mut answer).is_ok() && answer == token.as_bytes()
}

/// Listens for other copies of the app for as long as it runs: with a window closed
/// behind an icon, opening the app again must bring that window back, and not start
/// a second app over the same data.
pub(crate) fn listen_for_other_copies(id: &str, notify: Notify) -> Result<(), String> {
    let listener = TcpListener::bind(instance_address(id)).map_err(|e| e.to_string())?;
    let token = instance_token(id);
    std::thread::Builder::new()
        .name("ravenrs-instance".to_owned())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                if answer_copy(stream, &token) {
                    notify(TrayAction::Open);
                }
            }
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn answer_copy(mut stream: TcpStream, token: &str) -> bool {
    let _ = stream.set_read_timeout(Some(ANSWER_TIME));
    let _ = stream.set_write_timeout(Some(ANSWER_TIME));
    let mut asked = vec![0; token.len()];
    stream.read_exact(&mut asked).is_ok()
        && asked == token.as_bytes()
        && stream.write_all(token.as_bytes()).is_ok()
}

/// Told apart by person too, so that two people on one computer each have their app.
fn instance_token(id: &str) -> String {
    let person = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
    format!("ravenrs-instance:{id}:{person}\n")
}

/// A port of the dynamic range, the same for every copy of this app of this person.
fn instance_address(id: &str) -> SocketAddr {
    const FIRST_PORT: u32 = 49152;
    const PORTS: u32 = 16384;
    let hash = instance_token(id)
        .bytes()
        .fold(0x811c_9dc5_u32, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193));
    let port = u16::try_from(FIRST_PORT + hash % PORTS).unwrap_or(u16::MAX);
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "tests fail loudly")]

    use std::sync::mpsc::channel;

    use super::*;

    #[test]
    fn a_second_copy_finds_the_first_and_opens_its_window() {
        let id = format!("ravenrs-test-{}", std::process::id());
        assert!(!ask_running_copy(&id), "nobody is running yet");

        let (send, heard) = channel();
        let notify: Notify = Arc::new(move |action| send.send(action).expect("the test is listening"));
        listen_for_other_copies(&id, notify).expect("the port is free");

        assert!(ask_running_copy(&id));
        assert_eq!(heard.recv_timeout(Duration::from_secs(5)), Ok(TrayAction::Open));
    }

    #[test]
    fn something_else_on_the_port_is_not_a_copy_of_the_app() {
        let id = format!("ravenrs-test-other-{}", std::process::id());
        let listener = TcpListener::bind(instance_address(&id)).expect("the port is free");
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n");
            }
        });
        assert!(!ask_running_copy(&id));
    }

    #[test]
    fn the_address_stays_the_same_and_in_the_dynamic_range() {
        let address = instance_address("notes");
        assert_eq!(address, instance_address("notes"));
        assert!(address.ip().is_loopback() && address.port() >= 49152);
    }
}
