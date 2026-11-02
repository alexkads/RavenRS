# ravenrs-wayland-gestures

Touchpad pinches on Wayland, for RavenRS. winit 0.30 only delivers them on macOS and iOS;
this crate listens on the display winit already has open.

It holds the only `unsafe` call in the workspace, and should go away once RavenRS moves to a
winit that delivers these gestures on Wayland.
