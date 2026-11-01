# ravenrs-criteria

A criteria and expression language, independent of any UI. The same expressions filter a grid, compute a report total, guard a security rule, or become a SQL `WHERE`.

- **A public tree:** parse from text, build in code, walk it, and print it back to text that parses to the same tree.
- **Evaluation over any rows:** implement `Record` for your type; JSON and maps work out of the box.
- **Validation:** check fields, parameters and functions against a `Schema` before anything runs.
- **Parameterized SQL:** `to_sql` for PostgreSQL, MySQL, SQL Server and SQLite; values are always bound, never concatenated.
- **A function catalog:** over 130 functions and aggregates, each with a signature and description, ready for an expression editor.
- **Custom functions:** `Environment::register`.

This crate is part of [RavenRS](https://github.com/alexkads/RavenRS).

```rust
use ravenrs_criteria::{Criteria, Environment, Scope, Value};
use serde_json::json;

let order = json!({ "Items": [{ "Qty": 2, "Price": 5.0 }, { "Qty": 0, "Price": 9.0 }] });
let total = Criteria::parse("[Items][[Qty] > 0].Sum([Qty] * [Price])").unwrap();
let env = Environment::new();
assert_eq!(Scope::new(&env).row(&order).evaluate(&total), Ok(Value::Number(10.0)));
```

## License

MIT OR Apache-2.0.
