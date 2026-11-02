//! Touchpad pinches on Wayland. winit 0.30 delivers them only on macOS and iOS, so this
//! listens for them on the connection winit already has: gestures go to the client that owns
//! the window, and a connection of our own would never hear of them.
//!
//! This crate holds the only `unsafe` call in the workspace, and goes away with a winit that
//! delivers these gestures on Wayland.

use std::ffi::c_void;
use std::ptr::NonNull;

/// Turns the total scale a gesture reports into how much it changed since the last event.
#[derive(Clone, Copy, Debug)]
pub struct PinchTracker {
    last: f64,
}

impl Default for PinchTracker {
    fn default() -> Self {
        Self { last: 1.0 }
    }
}

impl PinchTracker {
    /// A gesture began: scales count from here.
    pub fn begin(&mut self) {
        self.last = 1.0;
    }

    /// The gesture stands at this scale. Returns the change as a fraction, `0.1` for 10% bigger.
    pub fn update(&mut self, scale: f64) -> f32 {
        if scale <= 0.0 || !scale.is_finite() {
            return 0.0;
        }
        let change = scale / self.last.max(f64::EPSILON) - 1.0;
        self.last = scale;
        change as f32
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
mod listener {
    use wayland_backend::client::Backend;
    use wayland_client::protocol::{wl_pointer, wl_registry, wl_seat};
    use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
    use wayland_protocols::wp::pointer_gestures::zv1::client::{
        zwp_pointer_gesture_pinch_v1 as pinch, zwp_pointer_gestures_v1 as gestures,
    };

    use super::{NonNull, PinchTracker, c_void};

    /// The version of the protocol this asks for: the first has everything it needs.
    const GESTURES_VERSION: u32 = 1;
    const SEAT_VERSION: u32 = 1;

    struct Listening<F> {
        on_pinch: F,
        tracker: PinchTracker,
        seat: Option<wl_seat::WlSeat>,
        gestures: Option<gestures::ZwpPointerGesturesV1>,
        /// Kept alive while it is being listened to.
        held: Option<(wl_pointer::WlPointer, pinch::ZwpPointerGesturePinchV1)>,
    }

    impl<F: Fn(f32) + 'static> Listening<F> {
        fn connect(&mut self, qh: &QueueHandle<Self>) {
            if self.held.is_some() {
                return;
            }
            if let (Some(seat), Some(gestures)) = (&self.seat, &self.gestures) {
                let pointer = seat.get_pointer(qh, ());
                let pinch = gestures.get_pinch_gesture(&pointer, qh, ());
                self.held = Some((pointer, pinch));
            }
        }
    }

    impl<F: Fn(f32) + 'static> Dispatch<wl_registry::WlRegistry, ()> for Listening<F> {
        fn event(
            state: &mut Self,
            registry: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            (): &(),
            _: &Connection,
            qh: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global { name, interface, .. } = event {
                if interface == wl_seat::WlSeat::interface().name && state.seat.is_none() {
                    state.seat = Some(registry.bind(name, SEAT_VERSION, qh, ()));
                } else if interface == gestures::ZwpPointerGesturesV1::interface().name {
                    state.gestures = Some(registry.bind(name, GESTURES_VERSION, qh, ()));
                }
                state.connect(qh);
            }
        }
    }

    impl<F: Fn(f32) + 'static> Dispatch<wl_seat::WlSeat, ()> for Listening<F> {
        fn event(
            _: &mut Self,
            _: &wl_seat::WlSeat,
            _: wl_seat::Event,
            (): &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    impl<F: Fn(f32) + 'static> Dispatch<wl_pointer::WlPointer, ()> for Listening<F> {
        fn event(
            _: &mut Self,
            _: &wl_pointer::WlPointer,
            _: wl_pointer::Event,
            (): &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    impl<F: Fn(f32) + 'static> Dispatch<gestures::ZwpPointerGesturesV1, ()> for Listening<F> {
        fn event(
            _: &mut Self,
            _: &gestures::ZwpPointerGesturesV1,
            _: gestures::Event,
            (): &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }

        event_created_child!(Listening<F>, gestures::ZwpPointerGesturesV1, []);
    }

    impl<F: Fn(f32) + 'static> Dispatch<pinch::ZwpPointerGesturePinchV1, ()> for Listening<F> {
        fn event(
            state: &mut Self,
            _: &pinch::ZwpPointerGesturePinchV1,
            event: pinch::Event,
            (): &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match event {
                pinch::Event::Begin { .. } => state.tracker.begin(),
                pinch::Event::Update { scale, .. } => {
                    let change = state.tracker.update(scale);
                    if change != 0.0 {
                        (state.on_pinch)(change);
                    }
                }
                _ => {}
            }
        }
    }

    /// See [`crate::listen`].
    pub(super) fn listen<F>(display: NonNull<c_void>, on_pinch: F) -> Result<(), String>
    where
        F: Fn(f32) + Send + 'static,
    {
        // SAFETY: `listen` requires a live `wl_display` that outlives the program's use of it.
        // libwayland is made for several queues over one display, and this one is our own.
        #[allow(unsafe_code)]
        let backend = unsafe { Backend::from_foreign_display(display.as_ptr().cast()) };
        let connection = Connection::from_backend(backend);
        let mut queue = connection.new_event_queue();
        let qh = queue.handle();
        let _registry = connection.display().get_registry(&qh, ());
        let mut state =
            Listening { on_pinch, tracker: PinchTracker::default(), seat: None, gestures: None, held: None };
        queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
        if state.gestures.is_none() {
            return Err("the compositor offers no pointer gestures".to_owned());
        }
        std::thread::Builder::new()
            .name("ravenrs-wayland-gestures".to_owned())
            .spawn(move || while queue.blocking_dispatch(&mut state).is_ok() {})
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// Calls `on_pinch` on another thread with how much each touchpad pinch changed, as a
/// fraction. Off Wayland's systems it does nothing.
///
/// `display` must be the live `wl_display` of the program's windows, for as long as the
/// program runs. Fails if the compositor has no pointer gestures.
pub fn listen<F>(display: NonNull<c_void>, on_pinch: F) -> Result<(), String>
where
    F: Fn(f32) + Send + 'static,
{
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        listener::listen(display, on_pinch)
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    {
        let _ = (display, on_pinch);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PinchTracker;

    #[test]
    fn the_total_scale_becomes_changes_that_add_up_to_it() {
        let mut tracker = PinchTracker::default();
        tracker.begin();
        let mut zoom = 1.0_f32;
        for scale in [1.1, 1.3, 1.5] {
            zoom *= 1.0 + tracker.update(scale);
        }
        assert!((zoom - 1.5).abs() < 0.001, "{zoom}");
    }

    #[test]
    fn a_new_gesture_counts_from_one_again() {
        let mut tracker = PinchTracker::default();
        tracker.update(2.0);
        tracker.begin();
        assert!((tracker.update(1.1) - 0.1).abs() < 0.001);
    }

    #[test]
    fn a_broken_scale_changes_nothing() {
        let mut tracker = PinchTracker::default();
        assert_eq!(tracker.update(0.0), 0.0);
        assert_eq!(tracker.update(f64::NAN), 0.0);
    }
}
