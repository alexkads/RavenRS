# ravenrs-animation

Motion primitives for [RavenRS](https://github.com/alexkads/RavenRS). Everything here is a pure function of time, with no timers, threads or frame loop, so it runs the same on the desktop, in WebAssembly and in headless tests.

- `Spring`: closed-form damped springs. The result does not depend on the frame rate.
- `CubicBezier` and `Easing`: CSS-compatible timing curves.
- `Animated<T>`: a value that can change target at any moment without a jump in position or velocity.
- OKLab interpolation for `Color`: midpoints keep their lightness and hue.
