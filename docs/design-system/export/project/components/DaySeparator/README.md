# DaySeparator

A line with a word in the middle that divides a list by day.

**Family:** Display and feedback · **Rust:** `ravenrs::prelude::DaySeparator`

## Stories

- **Example**: 85 × 80 pt, settings `{}`

## Sizing

Hugs its content: 85 × 80 pt in a parent of 760 pt; in a parent that stretches its children, as a form column, it takes the whole width.

## Behavior

- **Accessibility**: role `Separator` "Today".

## API

Builder methods: `new`, `of`.

## Example

```rust
fn day_separator() -> impl View {
    let today = Date::new(2026, 11, 4).unwrap_or_else(|| Date::from_days(0));
    VStack::new((
        DaySeparator::of(today, today),
        DaySeparator::of(today.add_days(-1), today),
        DaySeparator::new("Last week"),
    ))
    .spacing(16.0)
    .align(Align::Stretch)
}
```

Every number above is in `DaySeparator.json` beside this card.
