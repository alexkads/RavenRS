# ravenrs-junit

JUnit XML, both ways: what test tools write and build servers read.

Maven Surefire, pytest, `cargo nextest`, Jest and the end-to-end runner of RavenRS
(`ravenrs-e2e-runner`) report their tests as JUnit XML, and a CI (Raven CI's plugin `junit`) reads it
to show the count and the tests that broke. This crate is the one place that knows the format, for
writing and for reading, so the two never drift.

## What it has

| Item | What it does |
|---|---|
| `Suites`, `Suite`, `Case`, `Outcome`, `Problem` | The model: a run, its suites, their cases, and how each ended (passed, failure, error, skipped), with the message, the type and the text. Times in milliseconds. |
| `read(xml)` | Reads a `<testsuites>` or a lone `<testsuite>`, suites inside suites (one list, in the order they open), text in CDATA and entities, `time` in seconds with a fraction. Unknown elements and attributes are passed over. Broken XML is an `Error` with the byte where it broke. |
| `Suites::to_xml()` | Writes it, two spaces of indent, the counts worked out from the cases, escaped for XML 1.0 (control characters left out). `errors` is written only when there are some. |
| `Summary` | Tests, failures, errors, skipped, time and the full name of each case that failed or errored: what a screen shows at a glance. `Suites::summary()` and `Suite::summary()`. |

## How to use it

```rust
use ravenrs_junit::{Case, Outcome, Suite, Suites, read};

let suites = Suites::named("loja").suite(
    Suite::new("carrinho")
        .case(Case::new("carrinho", "soma os itens").time_ms(12))
        .case(Case::new("carrinho", "aplica o cupom").outcome(Outcome::failure("esperava 90"))),
);
let xml = suites.to_xml();
let summary = read(&xml)?.summary();
assert_eq!((summary.tests, summary.failures), (2, 1));
# Ok::<(), ravenrs_junit::Error>(())
```

## Who uses it

- `ravenrs-e2e-runner` writes its `--format junit` with it.
- Raven CI's plugin `junit` (`tools/raven-ci/crates/raven-ci-plugin-junit`) reads the reports of a
  build with it and keeps the summary.
