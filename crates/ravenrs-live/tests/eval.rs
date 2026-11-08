//! The interpreter on its own, without host types.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::rc::Rc;

use ravenrs_live::{Registry, Value, run};

fn eval(source: &str) -> Value {
    run(source, None, Rc::new(Registry::new())).unwrap_or_else(|e| panic!("{source}: {e:?}"))
}

fn error(source: &str) -> ravenrs_live::LiveError {
    run(source, None, Rc::new(Registry::new())).unwrap_err().remove(0)
}

#[test]
fn numbers_text_and_casts() {
    assert_eq!(eval("fn f() -> i64 { 7 / 2 + 3 % 2 }"), Value::Int(4));
    assert_eq!(eval("fn f() -> f32 { 7 as f32 / 2.0 }"), Value::Float(3.5));
    assert_eq!(eval("fn f() -> f32 { (2.0f32).powi(3).max(1.0) }"), Value::Float(8.0));
    assert_eq!(
        eval(r#"fn f() -> String { let n = 3; format!("{n} of {} ({:.1}) {:>4}|", 5, 2.25, "ab") }"#),
        Value::Str("3 of 5 (2.2)   ab|".into())
    );
    assert_eq!(eval(r#"fn f() -> String { "a".to_owned() + "b" }"#), Value::Str("ab".into()));
}

#[test]
fn closures_iterators_and_control() {
    assert_eq!(eval("fn f() -> i64 { (1..=4).map(|n| n * n).filter(|n| *n > 1).sum() }"), Value::Int(29));
    assert_eq!(
        eval("fn f() -> Vec<(usize, char)> { \"ab\".chars().enumerate().collect() }"),
        Value::List(vec![
            Value::Tuple(vec![Value::Int(0), Value::Char('a')]),
            Value::Tuple(vec![Value::Int(1), Value::Char('b')])
        ])
    );
    assert_eq!(eval("fn f() -> i64 { let add = |a, b| a + b; add(2, 3) }"), Value::Int(5));
    assert_eq!(
        eval(
            "fn f() -> &'static str { match 7 { 0 => \"zero\", 1..=5 => \"small\", n if n % 2 == 1 => \"odd\", _ => \"even\" } }"
        ),
        Value::Str("odd".into())
    );
    assert_eq!(
        eval("fn f() -> i64 { let mut t = 0; for i in 0..10 { if i == 5 { break; } t += i; } t }"),
        Value::Int(10)
    );
    assert_eq!(eval("fn f() -> i64 { if let Some(x) = Some(4) { x } else { 0 } }"), Value::Int(4));
    assert_eq!(eval("fn f() -> i64 { helper(3) } fn helper(n: i64) -> i64 { n * 10 }"), Value::Int(30));
    assert_eq!(eval("const K: i64 = 6; fn f() -> i64 { K * 2 }"), Value::Int(12));
    assert_eq!(eval("fn f() -> Vec<i64> { vec![0; 3] }"), Value::List(vec![Value::Int(0); 3]));
}

#[test]
fn state_changes_through_closures() {
    let value = eval(
        "fn f() -> i64 { let clicks = State::new(0); let add = clicks.clone(); add.update(|n| *n += 2); clicks.update(|n| *n *= 5); clicks.get() }",
    );
    assert_eq!(value, Value::Int(10));
}

#[test]
fn mistakes_say_where_they_are() {
    let e = error("fn f() -> i64 {\n    let x = 1;\n    x + nope\n}");
    assert_eq!((e.line, e.column), (3, 9), "{e}");
    assert!(e.message.contains("nope"), "{e}");
    let e = error("fn f() -> i64 {\n    Widget::new()\n}");
    assert_eq!(e.line, 2);
    assert!(e.message.contains("does not know `Widget`"), "{e}");
    let e = error("fn f( {");
    assert!(e.message.contains("not Rust"), "{e}");
    let e = error("fn f() -> String { \"More\"xx }");
    assert!(e.message.contains("no suffix"), "{e}");
    let e = error("fn f() -> i64 { 1 / 0 }");
    assert!(e.message.contains("division by zero"));
    let e = error("fn f() { loop {} }");
    assert!(e.message.contains("not read"), "{e}");
}
