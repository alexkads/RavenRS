# ravenrs-app-i18n

The languages and the words of a [RavenRS](https://github.com/alexkads/RavenRS) business application, in one place.

Every text the screens show has a key, as `ui.save` or `report.show`. The framework comes with them in English and in Brazilian Portuguese; a language without a key falls back to English. An application or a module adds or replaces texts with a flat JSON of keys:

```rust
use ravenrs_app_i18n::{Catalog, Language};

let mut catalog = Catalog::builtin();
catalog.merge_json("pt-BR", r#"{ "ui.save": "Gravar" }"#).unwrap();
assert_eq!(catalog.texts(Language::BrazilianPortuguese).get("ui.save"), "Gravar");
```

With `ravenrs-app-ui`, the same JSON goes to `App::texts`, and every screen takes it.
