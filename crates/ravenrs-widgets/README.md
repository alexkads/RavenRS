# ravenrs-widgets

Every RavenRS component (60+, including DataGrid and Ribbon), the `View`/`Component` API and the `Ui` runtime.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS). Most apps depend on the [`ravenrs`](https://crates.io/crates/ravenrs) facade instead, which re-exports it.

```rust
use ravenrs_widgets::{Button, Text, Ui, VStack};

use ravenrs_style::Theme;
let mut ui = Ui::new(VStack::new((Text::new("Hi"), Button::new("Go"))), Theme::raven_dark());
ui.update();
assert!(ui.element_count() > 2);
```

## License

MIT OR Apache-2.0.
