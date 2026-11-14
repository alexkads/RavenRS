# ravenrs-app-a11y

The accessibility of a [RavenRS](https://github.com/alexkads/RavenRS) business application, so that every application made with `ravenrs-app` reaches screen readers and the keyboard with nothing written for them.

- **Names from the model.** A field or an action is called by its caption, unless it says otherwise with `accessible_name` and `accessible_description`. The model keeps them on the form's items as `AccessibleName` and `AccessibleDescription`, where a translation changes them as it changes the caption.
- **Focus order.** `EntityInfo::focus_order` puts fields first on the form, in the same order on the screen and for Tab.
- **Announcements.** An `Announcer` says what changed, such as a record kept or deleted, as a status or an alert, through a `LiveRegion` that takes no room.
- **Words.** The words of the announcements have keys under `a11y.`, in English and Brazilian Portuguese. `AccessWords::from_lookup` takes them from wherever the application keeps its words.
- **Audit.** `Audit::new().app(&mut ui)` walks an application with no window: every item of the navigation and the buttons it is told to press. It reports a control with no name, a repeated id, focus that is held, text and focus ring under the WCAG AA contrast on its themes, and two commands on the same keys. `Audit::items` lists the items of the navigation, and `Audit::item` audits one of them by its name, so a large application can audit each screen in a test of its own, and the tests run in parallel.

```rust
use ravenrs_app_a11y::Audit;

let mut ui = ravenrs_widgets::Ui::new(my_app(), ravenrs_style::Theme::raven_dark());
ui.set_viewport(ravenrs_core::Size::new(1200.0, 800.0), 1.0);
Audit::new().open("New").app(&mut ui).assert_clean();
```

`ravenrs-app-ui` uses all of it: it names fields and commands from the model, announces saving and deleting, and puts the region in its window.
