//! The language end to end: syntax, printing, evaluation, validation and SQL.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::collections::HashMap;

use proptest::prelude::*;
use ravenrs_criteria::functions::catalog;
use ravenrs_criteria::{
    Aggregate, BinaryOp, Category, Criteria, Culture, DateTime, Dialect, Environment, EvalError, FieldType,
    Function, GroupOp, Problem, Record, Schema, Scope, SqlError, SqlParam, UnaryOp, Value, to_sql, validate,
};
use serde_json::{Value as Json, json};

fn parse(s: &str) -> Criteria {
    Criteria::parse(s).unwrap_or_else(|e| panic!("{s}: {e}"))
}

fn env() -> Environment {
    Environment::new()
        .culture(Culture::pt_br())
        .at(DateTime::parse("2026-11-11T10:30:00").expect("valid"))
        .parameter("Min", 10_i64)
        .calculated("Total", parse("[Qty] * [Price]"))
}

fn eval_row(row: &Json, src: &str) -> Value {
    let env = env();
    Scope::new(&env).row(row).evaluate(&parse(src)).unwrap_or_else(|e| panic!("{src}: {e}"))
}

fn eval(src: &str) -> Value {
    eval_row(
        &json!({ "Qty": 3, "Price": 2.5, "Name": "Ana Lima", "Due": "2026-11-20", "Customer": { "City": "Recife" } }),
        src,
    )
}

fn n(x: f64) -> Value {
    Value::Number(x)
}

fn t(s: &str) -> Value {
    Value::from(s)
}

#[test]
fn operators_and_null_rules() {
    assert_eq!(eval("[Qty] * [Price] + 1"), n(8.5));
    assert_eq!(eval("[Total] / 3"), n(2.5), "calculated fields");
    assert_eq!(eval("[Customer.City]"), t("Recife"));
    assert_eq!(eval("'Olá, ' + [Name]"), t("Olá, Ana Lima"));
    assert_eq!(eval("7 % 4"), n(3.0));
    assert_eq!(eval("6 & 3"), n(2.0));
    assert_eq!(eval("6 | 3"), n(7.0));
    assert_eq!(eval("6 ^ 3"), n(5.0));
    assert_eq!(eval("~0"), n(-1.0));
    assert_eq!(eval("1 / 0"), Value::Null, "division by zero is null, not infinity");
    assert_eq!(eval("[Missing] + 1"), Value::Null);
    assert_eq!(eval("[Missing] = Null"), Value::Bool(true));
    assert_eq!(eval("[Missing] > 1"), Value::Bool(false));
    assert_eq!(eval("[Missing] Is Null"), Value::Bool(true));
    assert_eq!(eval("[Name] Is Not Null"), Value::Bool(true));
    assert_eq!(eval("? = Null"), Value::Bool(true), "a lone ? is null");
    assert_eq!(eval("1 = '1'"), Value::Bool(false));
    assert_eq!(eval("[Qty] In (1, 3)"), Value::Bool(true));
    assert_eq!(eval("[Qty] Not In (1, 3)"), Value::Bool(false));
    assert_eq!(eval("[Price] Between(2, 3)"), Value::Bool(true));
    assert_eq!(eval("[Price] Not Between(2, 3)"), Value::Bool(false));
    assert_eq!(eval("[Name] Like 'Ana%'"), Value::Bool(true));
    assert_eq!(eval("[Name] Like 'A_a L%a'"), Value::Bool(true));
    assert_eq!(eval("[Name] Like 'Lima'"), Value::Bool(false));
    assert_eq!(eval("[Name] Not Like '%x%'"), Value::Bool(true));
    assert_eq!(eval("[Qty] > ?Min Or Not ([Qty] = 3)"), Value::Bool(false));
    assert_eq!(eval("/* a comment */ 1 + /* another */ 2"), n(3.0));
    assert_eq!(eval("12m + 1.5f"), n(13.5), "type suffixes");
    assert_eq!(eval("{d0a1-22}"), t("d0a1-22"), "guids are text");
    assert_eq!(eval("[Parameters.Min]"), n(10.0), "the older parameter spelling");
}

#[test]
fn logical_and_text_functions() {
    assert_eq!(eval("Iif([Qty] > 5, 'big', [Qty] > 2, 'medium', 'small')"), t("medium"));
    assert_eq!(eval("IsNull([Missing], 'n/a')"), t("n/a"));
    assert_eq!(eval("IsNull([Missing])"), Value::Bool(true));
    assert_eq!(eval("IsNullOrEmpty('')"), Value::Bool(true));
    assert_eq!(eval("Ascii('A')"), n(65.0));
    assert_eq!(eval("Char(65)"), t("A"));
    assert_eq!(eval("CharIndex('Lima', [Name])"), n(4.0));
    assert_eq!(eval("CharIndex('zzz', [Name])"), n(-1.0));
    assert_eq!(eval("CharIndex('a', 'banana', 2)"), n(3.0));
    assert_eq!(eval("Concat('a', 1, True)"), t("a1true"));
    assert_eq!(eval("Insert('abc', 1, 'X')"), t("aXbc"));
    assert_eq!(eval("Len('coração')"), n(7.0));
    assert_eq!(eval("Upper(Substring([Name], 4))"), t("LIMA"));
    assert_eq!(eval("Substring('abcdef', 1, 2)"), t("bc"));
    assert_eq!(eval("Remove('abcdef', 1, 2)"), t("adef"));
    assert_eq!(eval("Remove('abcdef', 4)"), t("abcd"));
    assert_eq!(eval("Reverse('abc')"), t("cba"));
    assert_eq!(eval("PadLeft('7', 3, '0')"), t("007"));
    assert_eq!(eval("PadRight('7', 3, '-')"), t("7--"));
    assert_eq!(eval("Replace('a-b-c', '-', '+')"), t("a+b+c"));
    assert_eq!(eval("Concat('a', NewLine(), 'b')"), t("a\nb"));
    assert_eq!(eval("Rgb(255, 0, 16)"), t("#ff0010"));
    assert_eq!(eval("Argb(128, 255, 0, 0)"), t("#ff000080"));
    assert_eq!(eval("FormatString('{0:C2} em {1:dd/MM}', [Total], [Due])"), t("R$\u{a0}7,50 em 20/11"));
}

#[test]
fn math_functions() {
    assert_eq!(eval("Round(2.345, 2)"), n(2.35));
    assert_eq!(eval("Round(2.5)"), n(3.0), "halves round away from zero");
    assert_eq!(eval("Round(-2.5)"), n(-3.0));
    assert_eq!(eval("Floor(2.7) + Ceiling(2.1)"), n(5.0));
    assert_eq!(eval("Power(2, 10)"), n(1024.0));
    assert_eq!(eval("Sqr(16)"), n(4.0));
    assert_eq!(eval("Sqrt(-1)"), Value::Null);
    assert_eq!(eval("Sign(-3)"), n(-1.0));
    assert_eq!(eval("Log(100, 10)"), n(2.0));
    assert_eq!(eval("Log10(1000)"), n(3.0));
    assert_eq!(eval("Log(0)"), Value::Null);
    assert_eq!(eval("Max(2, 5) + Min(2, 5)"), n(7.0));
    assert_eq!(eval("ToInt(2.9)"), n(2.0));
    assert_eq!(eval("ToDecimal('2.5')"), n(2.5));
    let r = eval("Rnd()").as_number().expect("a number");
    assert!((0.0..1.0).contains(&r));
}

#[test]
fn date_functions_follow_the_environment_clock() {
    let date = |s: &str| Value::Date(DateTime::parse(s).expect("valid"));
    assert_eq!(eval("Today()"), date("2026-11-11"));
    assert_eq!(eval("Now()"), date("2026-11-11T10:30:00"));
    assert_eq!(eval("AddDays('2026-01-31', 1)"), date("2026-02-01"));
    assert_eq!(eval("AddMonths('2026-01-31', 1)"), date("2026-02-28"));
    assert_eq!(eval("AddYears('2024-02-29', 1)"), date("2025-02-28"));
    assert_eq!(eval("AddHours('2026-01-01T23:00', 2)"), date("2026-01-02T01:00"));
    assert_eq!(eval("AddMinutes('2026-01-01', -1)"), date("2025-12-31T23:59"));
    assert_eq!(eval("DateDiffDay(Today(), [Due])"), n(9.0));
    assert_eq!(eval("DateDiffMonth('2026-01-31', '2026-03-01')"), n(2.0));
    assert_eq!(eval("DateDiffYear('2025-12-31', '2026-01-01')"), n(1.0));
    assert_eq!(eval("DateDiffHour('2026-01-01T10:00', '2026-01-01T13:30')"), n(3.0));
    assert_eq!(eval("GetYear([Due]) + GetMonth([Due]) + GetDay([Due])"), n(2057.0));
    assert_eq!(eval("GetDayOfWeek('2026-11-01')"), n(0.0), "a Sunday");
    assert_eq!(eval("GetDayOfYear('2026-02-01')"), n(32.0));
    assert_eq!(eval("GetHour(Now()) * 100 + GetMinute(Now())"), n(1030.0));
    assert_eq!(eval("GetDate(Now())"), date("2026-11-11"));
    assert_eq!(eval("DateTimeFromParts(2026, 3, 4, 5, 6, 7)"), date("2026-03-04T05:06:07"));
    assert_eq!(eval("DateTimeFromParts(2026, 13, 1)"), Value::Null);
    assert_eq!(eval("LocalDateTimeTomorrow()"), date("2026-11-12"));
    assert_eq!(eval("LocalDateTimeThisWeek()"), date("2026-11-08"));
    assert_eq!(eval("LocalDateTimeLastMonth()"), date("2026-10-01"));
    assert_eq!(eval("LocalDateTimeNextYear()"), date("2027-01-01"));
    assert_eq!(eval("IsThisMonth('2026-11-30')"), Value::Bool(true));
    assert_eq!(eval("IsThisMonth('2026-12-01')"), Value::Bool(false));
    assert_eq!(eval("IsLastMonth('2026-10-31')"), Value::Bool(true));
    assert_eq!(eval("IsNextMonth('2026-12-31')"), Value::Bool(true));
    assert_eq!(eval("IsThisWeek('2026-11-14')"), Value::Bool(true));
    assert_eq!(eval("IsThisWeek('2026-11-15')"), Value::Bool(false));
    assert_eq!(
        eval("IsThisYear('2026-01-01') And IsLastYear('2025-06-01') And IsNextYear('2027-06-01')"),
        Value::Bool(true)
    );
    assert_eq!(eval("IsYearToDate('2026-11-11')"), Value::Bool(true));
    assert_eq!(eval("IsYearToDate('2026-11-12')"), Value::Bool(false));
    assert_eq!(eval("IsNovember(Now()) And Not IsMay(Now())"), Value::Bool(true));
    assert_eq!(eval("IsSameDay('2026-01-01T08:00', '2026-01-02', '2026-01-01T23:00')"), Value::Bool(true));
    assert_eq!(eval("InDateRange('2026-01-15', '2026-01-01', '2026-01-15')"), Value::Bool(true));
    assert_eq!(
        eval("InTimeRange('2026-01-01T23:30', '2026-01-01T22:00', '2026-01-02T02:00')"),
        Value::Bool(true),
        "across midnight"
    );
    assert_eq!(
        eval("OutOfTimeRange('2026-01-01T12:00', '2026-01-01T22:00', '2026-01-02T02:00')"),
        Value::Bool(true)
    );
}

#[test]
fn aggregates_over_the_scope() {
    let env = env();
    let rows = [
        json!({"Qty": 1, "Price": 10, "Region": "N"}),
        json!({"Qty": 2, "Price": 5, "Region": "S"}),
        json!({"Qty": 4, "Price": 1, "Region": "N"}),
        json!({"Qty": 4, "Price": 2, "Region": "N"}),
    ];
    let scope: Vec<&Json> = rows.iter().collect();
    let at = |i: usize, src: &str| {
        Scope::new(&env).row(&rows[i]).rows(&scope).position(Some(i)).evaluate(&parse(src)).expect(src)
    };
    let e = |src: &str| at(0, src);
    assert_eq!(e("Sum([Total])"), n(32.0));
    assert_eq!(e("sumSum([Total])"), n(32.0), "the sum-prefixed spelling");
    assert_eq!(e("Count()"), n(4.0));
    assert_eq!(e("Count([Missing])"), n(0.0));
    assert_eq!(e("Avg([Qty])"), n(11.0 / 4.0));
    assert_eq!(e("Min([Price])"), n(1.0));
    assert_eq!(e("Max([Price])"), n(10.0));
    assert_eq!(e("Median([Qty])"), n(3.0));
    assert_eq!(e("DistinctCount([Region])"), n(2.0));
    assert_eq!(e("DistinctSum([Qty])"), n(7.0));
    assert_eq!(e("First([Region])"), t("N"));
    assert_eq!(e("Last([Price])"), n(2.0));
    assert_eq!(e("Join([Region], '-')"), t("N-S-N-N"));
    assert_eq!(e("Exists([Qty] > 3)"), Value::Bool(true));
    assert_eq!(e("Exists([Qty] > 9)"), Value::Bool(false));
    assert_eq!(e("Single([Qty])"), Value::Null, "more than one row");
    assert_eq!(e("WeightedAvg([Price], [Qty])"), n((10.0 + 10.0 + 4.0 + 8.0) / 11.0));
    assert_eq!(e("VarP([Qty])").as_number().map(|v| (v * 1000.0).round()), Some(1688.0));
    assert_eq!(e("StdDev([Qty])").as_number().map(|v| (v * 1000.0).round()), Some(1500.0));
    assert_eq!(at(2, "RecordNumber()"), n(3.0));
    assert_eq!(at(1, "Percentage([Qty])"), n(2.0 / 11.0));
    assert_eq!(at(1, "PrevRowColumnValue([Region])"), t("N"));
    assert_eq!(at(1, "NextRowColumnValue([Region])"), t("N"));
    assert_eq!(at(0, "PrevRowColumnValue([Region])"), Value::Null);
    assert_eq!(at(3, "NextRowColumnValue([Region])"), Value::Null);
    assert_eq!(at(2, "CurrentRowIndexInGroup()"), n(2.0));
}

#[test]
fn aggregates_over_collections_with_conditions() {
    let order = json!({
        "Discount": 1,
        "Lines": [{ "Qty": 2, "Price": 3, "Sku": "a" }, { "Qty": 0, "Price": 4, "Sku": "b" }, { "Qty": 5, "Price": 1, "Sku": "c" }],
    });
    let e = |s: &str| eval_row(&order, s);
    assert_eq!(e("[Lines][].Sum([Qty] * [Price]) - [Discount]"), n(10.0));
    assert_eq!(e("[Lines][[Qty] > 0].Count()"), n(2.0));
    assert_eq!(e("[Lines][[Qty] > 0 And [Sku] <> 'c'].Sum([Qty])"), n(2.0));
    assert_eq!(e("[Lines][].Sum([Qty] * [^.Discount])"), n(7.0), "^ reads the owner of the collection");
    assert_eq!(e("[Lines][[Qty] > [^.Discount]].Count()"), n(2.0));
    assert_eq!(e("[Lines][].Exists()"), Value::Bool(true));
    assert_eq!(e("[Lines][[Qty] > 99].Exists()"), Value::Bool(false));
    assert_eq!(e("[Lines][[Sku] = 'b'].Single([Price])"), n(4.0));
    assert_eq!(e("[Lines][].Join([Sku])"), t("a, b, c"));
    assert_eq!(
        e("[Lines][[Sku] Like '[x]%'].Count()"),
        n(0.0),
        "brackets inside quotes do not end the field"
    );
    assert_eq!(e("[Nothing][].Count()"), n(0.0));
    assert_eq!(e("[Nothing][].Sum([Qty])"), Value::Null);
    assert_eq!(e("[Nothing][].Exists()"), Value::Bool(false));
}

#[test]
fn running_sums_are_incremental_and_correct() {
    let env = Environment::new();
    let rows: Vec<Json> = (1..=2000).map(|i| json!({ "V": i })).collect();
    let scope: Vec<&Json> = rows.iter().collect();
    let c = parse("RunningSum([V])");
    let start = std::time::Instant::now();
    for (i, row) in rows.iter().enumerate() {
        let k = (i + 1) as f64;
        assert_eq!(
            Scope::new(&env).row(row).rows(&scope).position(Some(i)).evaluate(&c),
            Ok(n(k * (k + 1.0) / 2.0))
        );
    }
    assert!(start.elapsed().as_secs() < 5, "not quadratic");
}

#[test]
fn custom_functions_errors_and_cycles() {
    let env = Environment::new()
        .register("Double", |a| Ok(n(a[0].as_number().unwrap_or(0.0) * 2.0)))
        .register("Boom", |_| Err("bad".to_owned()))
        .calculated("A", parse("[B] + 1"))
        .calculated("B", parse("[A] + 1"));
    let row = json!({});
    let scope = Scope::new(&env).row(&row);
    assert_eq!(scope.evaluate(&parse("DOUBLE(21)")), Ok(n(42.0)), "names are case-insensitive");
    assert_eq!(scope.evaluate(&parse("Nope(1)")), Err(EvalError::UnknownFunction("Nope".into())));
    assert_eq!(
        scope.evaluate(&parse("Boom()")),
        Err(EvalError::FunctionFailed { name: "Boom".into(), message: "bad".into() })
    );
    assert!(matches!(scope.evaluate(&parse("[A]")), Err(EvalError::Cycle(_))));
    assert_eq!(scope.evaluate(&parse("1 + 1")), Ok(n(2.0)), "an error does not stick to later evaluations");
    assert!(!scope.matches(&parse("Nope(1)")), "a failing filter does not match");
}

#[test]
fn any_row_type_can_be_evaluated() {
    #[derive(Debug)]
    struct Person {
        name: &'static str,
        age: i64,
        kids: Vec<Person>,
    }
    impl Record for Person {
        fn field(&self, path: &[String]) -> Option<Value> {
            match path.first()?.as_str() {
                "Name" => Some(self.name.into()),
                "Age" => Some(self.age.into()),
                _ => None,
            }
        }
        fn collection(&self, path: &[String]) -> Option<Vec<&Self>> {
            (path.first()? == "Kids").then(|| self.kids.iter().collect())
        }
    }
    let p = Person {
        name: "Ana",
        age: 40,
        kids: vec![
            Person { name: "Bia", age: 9, kids: vec![] },
            Person { name: "Caio", age: 12, kids: vec![] },
        ],
    };
    let env = Environment::new();
    let scope = Scope::new(&env).row(&p);
    assert_eq!(
        scope.evaluate(&parse("[Kids][[Age] > 10].Join([Name]) + ' de ' + [Name]")),
        Ok(t("Caio de Ana"))
    );

    let map: HashMap<String, Value> = HashMap::from([("A".to_owned(), n(2.0))]);
    let env = Environment::new();
    assert_eq!(Scope::new(&env).row(&map).evaluate(&parse("[A] * 2")), Ok(n(4.0)));
}

#[test]
fn trees_can_be_built_inspected_and_printed() {
    let built = Criteria::property("Age")
        .equals(Criteria::value(30_i64))
        .and(Criteria::property("Active"))
        .and(!Criteria::property("Vip"));
    assert_eq!(built.to_string(), "[Age] = 30 And [Active] And Not [Vip]");
    assert_eq!(parse(&built.to_string()).to_string(), built.to_string());
    assert!(matches!(&built, Criteria::Group(GroupOp::And, items) if items.len() == 3), "And flattens");
    let c = parse("[A] + ?P > Upper([B.C]) And [A] = ?P");
    assert_eq!(c.properties(), ["A", "B.C"]);
    assert_eq!(c.parameters(), ["P"]);
    assert!(c.any(&|n| matches!(n, Criteria::Function(Function::Upper, _))));
    for (src, printed) in [
        ("[a]+1", "[a] + 1"),
        ("(1+2)*3", "(1 + 2) * 3"),
        ("1+(2*3)", "1 + 2 * 3"),
        ("1-(2-3)", "1 - (2 - 3)"),
        ("(1-2)-3", "1 - 2 - 3"),
        ("not [a] and [b]", "Not [a] And [b]"),
        ("not ([a] and [b])", "Not ([a] And [b])"),
        ("[a] or [b] and [c]", "[a] Or [b] And [c]"),
        ("([a] or [b]) and [c]", "([a] Or [b]) And [c]"),
        ("'it''s'", "'it''s'"),
        ("#2026-11-01#", "#2026-11-01#"),
        ("[a] in (1,2)", "[a] In (1, 2)"),
        ("[a] not in (1)", "Not [a] In (1)"),
        ("[a] is not null", "Not [a] Is Null"),
        ("[a] between(1,2)", "[a] Between(1, 2)"),
        ("[a] like 'x%'", "[a] Like 'x%'"),
        ("iif([a],1,2)", "Iif([a], 1, 2)"),
        ("[l][[q]>0].sum([q])", "[l][[q] > 0].Sum([q])"),
        ("[l][].count()", "[l][].Count()"),
        ("sumSum([a])", "Sum([a])"),
        ("Custom1([a])", "Custom1([a])"),
        ("-[a]", "-[a]"),
        ("~1", "~1"),
    ] {
        assert_eq!(parse(src).to_string(), printed, "{src}");
        assert_eq!(parse(printed), parse(src), "{printed} reads back to the same tree");
    }
}

#[test]
fn syntax_errors_say_what_and_where() {
    let err = |s: &str| Criteria::parse(s).expect_err(s);
    assert_eq!(err("[A] +").message, "unexpected end of expression");
    assert_eq!(err("Total * 2").offset, 0);
    assert!(err("Total * 2").message.contains("[Total]"));
    assert!(err("Iif(1, 2)").message.contains("Iif(cond"));
    assert!(err("Iif(1, 2, 3, 4)").message.contains("wrong number"));
    assert!(err("Upper()").message.contains("Upper(text)"));
    assert!(err("Sum()").message.contains("Sum(value)"));
    assert_eq!(err("[A] = 'x").message, "unterminated text");
    assert_eq!(err("(1 + 2").message, "expected ')', found end of expression");
    assert_eq!(err("[A] @ 2").offset, 4);
    assert_eq!(err("[A").message, "missing ']'");
    assert_eq!(err("/* open").message, "unterminated comment");
    assert_eq!(err("#2026-13-01#").message, "invalid date");
    assert_eq!(err("[L][].Nope()").message, "unknown aggregate 'Nope'");
    assert_eq!(err("[L][[Q] >].Sum([Q])").offset, 9, "errors inside a condition point into the source");
    assert_eq!(err("[A] Is 1").message, "expected 'Null' after 'Is'");
    let deep = format!("{}1{}", "(".repeat(5000), ")".repeat(5000));
    assert!(err(&deep).message.contains("nested too deeply"));
    assert!(err(&"-".repeat(5000)).message.contains("nested too deeply"));
}

#[test]
fn validation_finds_unknown_names() {
    let schema = Schema::new()
        .field("Name", FieldType::Text)
        .field("Customer.City", FieldType::Text)
        .parameter("Min", FieldType::Number)
        .function("Tax")
        .collection("Lines", Schema::new().field("Qty", FieldType::Number));
    let ok = parse("[Name] = [Customer.City] And [Lines][[Qty] > ?Min].Sum([Qty] * Tax([^.Name])) > 0");
    assert_eq!(validate(&ok, &schema), []);
    let bad = parse(
        "[Nme] = ?Nope And Foo(1) And [Lines][].Sum([Qt]) > 0 And [Others][].Count() > 0 And [Nme] = 1",
    );
    assert_eq!(
        validate(&bad, &schema),
        [
            Problem::UnknownField("Nme".into()),
            Problem::UnknownParameter("Nope".into()),
            Problem::UnknownFunction("Foo".into()),
            Problem::UnknownField("Qt".into()),
            Problem::UnknownCollection("Others".into()),
        ]
    );
    assert_eq!(Problem::UnknownField("X".into()).to_string(), "unknown field 'X'");
}

#[test]
fn sql_is_parameterized_and_quoted_per_dialect() {
    let c = parse("[Total] > 100 And [Name] = 'x'");
    let pg = to_sql(&c, Dialect::Postgres).expect("translatable");
    assert_eq!(pg.text, r#""Total" > $1 AND "Name" = $2"#);
    assert_eq!(pg.params, [SqlParam::Value(n(100.0)), SqlParam::Value(t("x"))]);
    assert_eq!(to_sql(&c, Dialect::MySql).expect("ok").text, "`Total` > ? AND `Name` = ?");
    assert_eq!(to_sql(&c, Dialect::SqlServer).expect("ok").text, "[Total] > @p1 AND [Name] = @p2");
    assert_eq!(to_sql(&c, Dialect::Sqlite).expect("ok").text, r#""Total" > ?1 AND "Name" = ?2"#);

    let sql = |s: &str| to_sql(&parse(s), Dialect::Postgres).expect(s).text;
    assert_eq!(sql("[A.B] = ?P"), r#""A"."B" = $1"#);
    assert_eq!(sql("[A] = Null"), r#""A" IS NULL"#);
    assert_eq!(sql("[A] <> Null"), r#""A" IS NOT NULL"#);
    assert_eq!(sql("[A] Is Not Null"), r#"NOT ("A" IS NULL)"#);
    assert_eq!(sql("[A] In (1, 2)"), r#""A" IN ($1, $2)"#);
    assert_eq!(sql("[A] Between(1, 2)"), r#""A" BETWEEN $1 AND $2"#);
    assert_eq!(sql("([A] Or [B]) And [C]"), r#"("A" OR "B") AND "C""#);
    assert_eq!(sql("Upper([A]) Like 'X%'"), r#"UPPER("A") LIKE $1"#);
    assert_eq!(sql("Iif([A] > 1, 'y', 'n')"), r#"CASE WHEN "A" > $1 THEN $2 ELSE $3 END"#);
    assert_eq!(sql("IsNull([A], 0)"), r#"COALESCE("A", $1)"#);
    assert_eq!(sql("Sum([A] * 2)"), r#"SUM("A" * $1)"#);
    assert_eq!(sql("Count()"), "COUNT(*)");
    assert_eq!(sql("GetYear([D]) = 2026"), r#"EXTRACT(YEAR FROM "D") = $1"#);
    assert_eq!(to_sql(&parse("GetYear([D])"), Dialect::SqlServer).expect("ok").text, "DATEPART(YEAR, [D])");
    assert_eq!(to_sql(&parse("Len([A])"), Dialect::MySql).expect("ok").text, "CHAR_LENGTH(`A`)");
}

#[test]
fn hostile_values_and_names_cannot_change_the_query() {
    let c = parse("[Name] = 'x''; DROP TABLE users; --'");
    let q = to_sql(&c, Dialect::Postgres).expect("translatable");
    assert_eq!(q.text, r#""Name" = $1"#);
    assert_eq!(q.params, [SqlParam::Value(t("x'; DROP TABLE users; --"))]);

    let c = parse(r#"[a"b; DROP TABLE t; --] = 1"#);
    assert_eq!(to_sql(&c, Dialect::Postgres).expect("ok").text, r#""a""b; DROP TABLE t; --" = $1"#);
    assert_eq!(to_sql(&parse("[a`b] = 1"), Dialect::MySql).expect("ok").text, "`a``b` = ?");

    // Search text is data: its wildcards are escaped and it is bound.
    let q = to_sql(&parse("Contains([A], '50%_off')"), Dialect::Postgres).expect("ok");
    assert_eq!(q.text, r#""A" LIKE $1 ESCAPE '\'"#);
    assert_eq!(q.params, [SqlParam::Value(t("%50\\%\\_off%"))]);
}

#[test]
fn what_sql_cannot_express_is_an_error_not_a_wrong_query() {
    let unsupported = |s: &str| matches!(to_sql(&parse(s), Dialect::Postgres), Err(SqlError::Unsupported(_)));
    assert!(unsupported("[L][].Sum([A])"));
    assert!(unsupported("Median([A])"));
    assert!(unsupported("Custom([A])"));
    assert!(unsupported("[^.A] = 1"));
    assert!(unsupported("IsThisMonth([D])"));
    assert!(unsupported("Contains([A], [B])"));
}

#[test]
fn the_catalog_documents_every_function() {
    let all = catalog();
    assert!(
        all.iter().all(|f| !f.description.is_empty() && f.signature.contains(f.name) && !f.name.is_empty())
    );
    for category in [
        Category::Logical,
        Category::String,
        Category::Math,
        Category::DateTime,
        Category::Reporting,
        Category::Aggregate,
    ] {
        assert!(all.iter().any(|f| f.category == category), "{category:?}");
    }
    // Everything in the catalog parses when called with its smallest arity.
    for f in Function::all() {
        let (min, _) = f.arity();
        let args = vec!["1"; if f == Function::Iif { 3 } else { min }].join(", ");
        Criteria::parse(&format!("{}({args})", f.name())).unwrap_or_else(|e| panic!("{}: {e}", f.name()));
    }
    for a in Aggregate::all() {
        let args = vec!["[x]"; a.arity().0].join(", ");
        Criteria::parse(&format!("{}({args})", a.name())).unwrap_or_else(|e| panic!("{}: {e}", a.name()));
    }
    assert_eq!(Criteria::parse("Min(1)").map(|c| matches!(c, Criteria::Aggregate(_))), Ok(true));
    assert!(matches!(parse("-1"), Criteria::Unary(UnaryOp::Minus, _)));
    assert!(matches!(parse("1 & 2"), Criteria::Binary(BinaryOp::BitAnd, ..)));
}

fn arb_criteria() -> impl Strategy<Value = Criteria> {
    let leaf = prop_oneof![
        (0u32..1000).prop_map(|i| Criteria::value(f64::from(i))),
        "[a-z ']{0,6}".prop_map(Criteria::value),
        any::<bool>().prop_map(Criteria::value),
        Just(Criteria::Value(Value::Null)),
        prop_oneof![Just("A"), Just("B.C"), Just("^.D")].prop_map(Criteria::property),
        "[a-z]{1,4}".prop_map(Criteria::Parameter),
    ];
    leaf.prop_recursive(5, 48, 4, |inner| {
        let binary = prop_oneof![
            Just(BinaryOp::Add),
            Just(BinaryOp::Sub),
            Just(BinaryOp::Mul),
            Just(BinaryOp::Div),
            Just(BinaryOp::Eq),
            Just(BinaryOp::Lt),
            Just(BinaryOp::Like),
            Just(BinaryOp::BitOr),
            Just(BinaryOp::BitXor),
            Just(BinaryOp::BitAnd),
        ];
        prop_oneof![
            (binary, inner.clone(), inner.clone()).prop_map(|(op, a, b)| Criteria::Binary(
                op,
                Box::new(a),
                Box::new(b)
            )),
            proptest::collection::vec(inner.clone(), 2..4).prop_map(|v| Criteria::Group(GroupOp::And, v)),
            proptest::collection::vec(inner.clone(), 2..4).prop_map(|v| Criteria::Group(GroupOp::Or, v)),
            inner.clone().prop_map(|a| Criteria::Unary(UnaryOp::Not, Box::new(a))),
            inner.clone().prop_map(|a| Criteria::Unary(UnaryOp::Minus, Box::new(a))),
            inner.clone().prop_map(|a| Criteria::IsNull(Box::new(a))),
            (inner.clone(), proptest::collection::vec(inner.clone(), 1..3))
                .prop_map(|(a, v)| Criteria::In(Box::new(a), v)),
            (inner.clone(), inner.clone(), inner.clone()).prop_map(|(a, b, c)| Criteria::Between(
                Box::new(a),
                Box::new(b),
                Box::new(c)
            )),
            inner.clone().prop_map(|a| Criteria::Function(Function::Upper, vec![a])),
        ]
    })
}

fn sample_rows() -> Vec<Json> {
    vec![json!({ "A": 1, "B": { "C": "x" } }), json!({ "A": 2.5, "B": { "C": "y" } }), json!({})]
}

proptest! {
    /// Printing and parsing again gives the same tree.
    #[test]
    fn printing_round_trips(c in arb_criteria()) {
        let printed = c.to_string();
        let back = Criteria::parse(&printed).map_err(|e| TestCaseError::fail(format!("{printed}: {e}")))?;
        prop_assert_eq!(&back, &c, "{}", printed);
    }

    /// Evaluation never panics, and a printed tree evaluates the same.
    #[test]
    fn evaluation_is_total(c in arb_criteria()) {
        let env = Environment::new().at(DateTime::parse("2026-01-01").expect("valid"));
        let rows = sample_rows();
        let scope: Vec<&Json> = rows.iter().collect();
        for row in &rows {
            let s = Scope::new(&env).row(row).rows(&scope);
            let a = s.evaluate(&c);
            let b = s.evaluate(&Criteria::parse(&c.to_string()).expect("round trip"));
            prop_assert_eq!(format!("{a:?}"), format!("{b:?}"));
        }
    }

    /// SQL translation never panics, and every placeholder has its parameter.
    #[test]
    fn sql_placeholders_match_their_parameters(c in arb_criteria()) {
        if let Ok(q) = to_sql(&c, Dialect::Sqlite) {
            for i in 1..=q.params.len() {
                prop_assert!(q.text.contains(&format!("?{i}")), "{} {:?}", q.text, q.params);
            }
            let beyond = format!("?{}", q.params.len() + 1);
            prop_assert!(!q.text.contains(beyond.as_str()));
        }
    }

    /// The parser never panics, whatever the input.
    #[test]
    fn parser_is_total(src in "\\PC{0,80}") {
        let _ = Criteria::parse(&src);
    }

    /// Filter logic matches Rust's on arbitrary numbers.
    #[test]
    fn comparisons_match_rust(a in -1000i32..1000, b in -1000i32..1000) {
        let env = Environment::new().parameter("a", i64::from(a)).parameter("b", i64::from(b));
        let row = json!({});
        let s = Scope::new(&env).row(&row);
        prop_assert_eq!(s.matches(&parse("?a < ?b")), a < b);
        prop_assert_eq!(s.matches(&parse("?a = ?b")), a == b);
        prop_assert_eq!(s.matches(&parse("?a < ?b Or ?a >= ?b")), true);
        prop_assert_eq!(s.evaluate(&parse("?a + ?b")), Ok(n(f64::from(a) + f64::from(b))));
    }
}
