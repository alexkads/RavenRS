# ravenrs-app-spec

What `#[derive(BusinessObject)]` and `#[derive(BusinessEnum)]` read from a struct or an enum, as plain data. The derive in `ravenrs-app-derive` turns it into code; `ravenrs-app`, with its `source` feature, turns it into entities while a program runs, as the RavenRS playground does. Both accept and refuse the same things, with the same messages.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS).

```rust
let file = ravenrs_app_spec::parse_file(
    r#"
    #[derive(BusinessObject)]
    #[object(caption = "Banks")]
    struct Bank {
        #[field(typed_key, caption = "Code")]
        code: String,
    }
    "#,
)
.map_err(|errors| errors[0].message.clone())?;
assert_eq!(file.objects[0].name.to_string(), "Bank");
# Ok::<(), String>(())
```

## License

MIT OR Apache-2.0.
