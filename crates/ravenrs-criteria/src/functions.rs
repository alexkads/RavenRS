//! The function catalog: one table that drives parsing, arity checks and the
//! documentation an expression editor shows.

/// What a function is for, as an expression editor groups them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Category {
    /// Conditions and null handling.
    Logical,
    /// Text.
    String,
    /// Numbers.
    Math,
    /// Dates and times.
    DateTime,
    /// Formatting and the page or row being printed.
    Reporting,
    /// Summaries over rows.
    Aggregate,
}

/// A catalog entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionInfo {
    /// The name, as written in expressions.
    pub name: &'static str,
    /// The group it belongs to.
    pub category: Category,
    /// How it is called, such as `Round(value, digits)`.
    pub signature: &'static str,
    /// What it does.
    pub description: &'static str,
}

macro_rules! functions {
    ($($variant:ident $name:literal $min:literal $max:literal $cat:ident $sig:literal $doc:literal;)*) => {
        /// The built-in scalar functions. Names are case-insensitive in expressions.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        #[allow(missing_docs, reason = "documented by the catalog, see Function::info")]
        pub enum Function { $($variant,)* }

        const FUNCTIONS: &[(Function, usize, usize, FunctionInfo)] = &[
            $((Function::$variant, $min, $max, FunctionInfo {
                name: $name, category: Category::$cat, signature: $sig, description: $doc,
            }),)*
        ];
    };
}

functions! {
    // Logical
    Iif "Iif" 3 255 Logical "Iif(cond, value, ..., else)" "The value of the first condition that holds, else the last argument.";
    IsNull "IsNull" 1 2 Logical "IsNull(value[, fallback])" "With one argument, whether it is null; with two, the first that is not null.";
    IsNullOrEmpty "IsNullOrEmpty" 1 1 Logical "IsNullOrEmpty(text)" "Whether the text is null or empty.";
    // String
    Ascii "Ascii" 1 1 String "Ascii(text)" "The code of the first character.";
    Char "Char" 1 1 String "Char(code)" "The character with this code.";
    CharIndex "CharIndex" 2 3 String "CharIndex(needle, text[, start])" "The position of needle in text, or -1.";
    Concat "Concat" 1 255 String "Concat(a, b, ...)" "Joins values as text.";
    Contains "Contains" 2 2 String "Contains(text, part)" "Whether text contains part.";
    EndsWith "EndsWith" 2 2 String "EndsWith(text, part)" "Whether text ends with part.";
    Insert "Insert" 3 3 String "Insert(text, position, part)" "Inserts part at a position.";
    Len "Len" 1 1 String "Len(text)" "The number of characters.";
    Lower "Lower" 1 1 String "Lower(text)" "Lowercase.";
    NewLine "NewLine" 0 0 String "NewLine()" "A line break.";
    PadLeft "PadLeft" 2 3 String "PadLeft(text, width[, char])" "Pads on the left up to a width.";
    PadRight "PadRight" 2 3 String "PadRight(text, width[, char])" "Pads on the right up to a width.";
    Remove "Remove" 2 3 String "Remove(text, start[, count])" "Removes characters.";
    Replace "Replace" 3 3 String "Replace(text, old, new)" "Replaces every occurrence.";
    Reverse "Reverse" 1 1 String "Reverse(text)" "The characters in reverse order.";
    StartsWith "StartsWith" 2 2 String "StartsWith(text, part)" "Whether text starts with part.";
    Substring "Substring" 2 3 String "Substring(text, start[, count])" "Part of a text, from position 0.";
    ToStr "ToStr" 1 1 String "ToStr(value)" "The value as text.";
    Trim "Trim" 1 1 String "Trim(text)" "Removes spaces around the text.";
    Upper "Upper" 1 1 String "Upper(text)" "Uppercase.";
    // Math
    Abs "Abs" 1 1 Math "Abs(x)" "Absolute value.";
    Acos "Acos" 1 1 Math "Acos(x)" "Arc cosine.";
    Asin "Asin" 1 1 Math "Asin(x)" "Arc sine.";
    Atn "Atn" 1 1 Math "Atn(x)" "Arc tangent.";
    Atn2 "Atn2" 2 2 Math "Atn2(y, x)" "Arc tangent of y/x.";
    BigMul "BigMul" 2 2 Math "BigMul(a, b)" "The product of two integers.";
    Ceiling "Ceiling" 1 1 Math "Ceiling(x)" "The next integer up.";
    Cos "Cos" 1 1 Math "Cos(x)" "Cosine.";
    Cosh "Cosh" 1 1 Math "Cosh(x)" "Hyperbolic cosine.";
    Exp "Exp" 1 1 Math "Exp(x)" "e to the power x.";
    Floor "Floor" 1 1 Math "Floor(x)" "The next integer down.";
    Log "Log" 1 2 Math "Log(x[, base])" "Logarithm, natural by default.";
    Log10 "Log10" 1 1 Math "Log10(x)" "Base 10 logarithm.";
    Max2 "Max" 2 2 Math "Max(a, b)" "The larger of two values.";
    Min2 "Min" 2 2 Math "Min(a, b)" "The smaller of two values.";
    Power "Power" 2 2 Math "Power(x, y)" "x to the power y.";
    Rnd "Rnd" 0 0 Math "Rnd()" "A random number from 0 up to 1.";
    Round "Round" 1 2 Math "Round(x[, digits])" "Rounds halves away from zero.";
    Sign "Sign" 1 1 Math "Sign(x)" "-1, 0 or 1.";
    Sin "Sin" 1 1 Math "Sin(x)" "Sine.";
    Sinh "Sinh" 1 1 Math "Sinh(x)" "Hyperbolic sine.";
    Sqr "Sqr" 1 1 Math "Sqr(x)" "Square root.";
    Sqrt "Sqrt" 1 1 Math "Sqrt(x)" "Square root.";
    Tan "Tan" 1 1 Math "Tan(x)" "Tangent.";
    Tanh "Tanh" 1 1 Math "Tanh(x)" "Hyperbolic tangent.";
    ToDecimal "ToDecimal" 1 1 Math "ToDecimal(x)" "The value as a number.";
    ToDouble "ToDouble" 1 1 Math "ToDouble(x)" "The value as a number.";
    ToFloat "ToFloat" 1 1 Math "ToFloat(x)" "The value as a number.";
    ToInt "ToInt" 1 1 Math "ToInt(x)" "The value as an integer, truncated.";
    ToLong "ToLong" 1 1 Math "ToLong(x)" "The value as an integer, truncated.";
    ToNumber "ToNumber" 1 1 Math "ToNumber(x)" "The value as a number.";
    // Date and time: arithmetic
    AddDays "AddDays" 2 2 DateTime "AddDays(date, n)" "n days later.";
    AddHours "AddHours" 2 2 DateTime "AddHours(date, n)" "n hours later.";
    AddMinutes "AddMinutes" 2 2 DateTime "AddMinutes(date, n)" "n minutes later.";
    AddMonths "AddMonths" 2 2 DateTime "AddMonths(date, n)" "n months later.";
    AddSeconds "AddSeconds" 2 2 DateTime "AddSeconds(date, n)" "n seconds later.";
    AddYears "AddYears" 2 2 DateTime "AddYears(date, n)" "n years later.";
    DateDiffDay "DateDiffDay" 2 2 DateTime "DateDiffDay(from, to)" "Day boundaries between two dates.";
    DateDiffHour "DateDiffHour" 2 2 DateTime "DateDiffHour(from, to)" "Whole hours between two dates.";
    DateDiffMinute "DateDiffMinute" 2 2 DateTime "DateDiffMinute(from, to)" "Whole minutes between two dates.";
    DateDiffMonth "DateDiffMonth" 2 2 DateTime "DateDiffMonth(from, to)" "Month boundaries between two dates.";
    DateDiffSecond "DateDiffSecond" 2 2 DateTime "DateDiffSecond(from, to)" "Whole seconds between two dates.";
    DateDiffYear "DateDiffYear" 2 2 DateTime "DateDiffYear(from, to)" "Year boundaries between two dates.";
    DateTimeFromParts "DateTimeFromParts" 3 6 DateTime "DateTimeFromParts(y, m, d[, h, min, s])" "A date from its parts.";
    // Date and time: parts
    GetDate "GetDate" 1 1 DateTime "GetDate(date)" "The date without the time.";
    GetDay "GetDay" 1 1 DateTime "GetDay(date)" "The day of the month.";
    GetDayOfWeek "GetDayOfWeek" 1 1 DateTime "GetDayOfWeek(date)" "The day of the week, 0 is Sunday.";
    GetDayOfYear "GetDayOfYear" 1 1 DateTime "GetDayOfYear(date)" "The day of the year, from 1.";
    GetHour "GetHour" 1 1 DateTime "GetHour(date)" "The hour.";
    GetMinute "GetMinute" 1 1 DateTime "GetMinute(date)" "The minute.";
    GetMonth "GetMonth" 1 1 DateTime "GetMonth(date)" "The month, 1 to 12.";
    GetSecond "GetSecond" 1 1 DateTime "GetSecond(date)" "The second.";
    GetYear "GetYear" 1 1 DateTime "GetYear(date)" "The year.";
    Now "Now" 0 0 DateTime "Now()" "The current date and time.";
    Today "Today" 0 0 DateTime "Today()" "The current date.";
    UtcNow "UtcNow" 0 0 DateTime "UtcNow()" "The current date and time; the environment's clock.";
    // Date and time: relative dates
    LocalDateTimeToday "LocalDateTimeToday" 0 0 DateTime "LocalDateTimeToday()" "The start of today.";
    LocalDateTimeNow "LocalDateTimeNow" 0 0 DateTime "LocalDateTimeNow()" "Now.";
    LocalDateTimeTomorrow "LocalDateTimeTomorrow" 0 0 DateTime "LocalDateTimeTomorrow()" "The start of tomorrow.";
    LocalDateTimeYesterday "LocalDateTimeYesterday" 0 0 DateTime "LocalDateTimeYesterday()" "The start of yesterday.";
    LocalDateTimeDayAfterTomorrow "LocalDateTimeDayAfterTomorrow" 0 0 DateTime "LocalDateTimeDayAfterTomorrow()" "The start of the day after tomorrow.";
    LocalDateTimeThisWeek "LocalDateTimeThisWeek" 0 0 DateTime "LocalDateTimeThisWeek()" "The start of this week (Sunday).";
    LocalDateTimeLastWeek "LocalDateTimeLastWeek" 0 0 DateTime "LocalDateTimeLastWeek()" "The start of last week.";
    LocalDateTimeNextWeek "LocalDateTimeNextWeek" 0 0 DateTime "LocalDateTimeNextWeek()" "The start of next week.";
    LocalDateTimeTwoWeeksAway "LocalDateTimeTwoWeeksAway" 0 0 DateTime "LocalDateTimeTwoWeeksAway()" "The start of the week after next.";
    LocalDateTimeThisMonth "LocalDateTimeThisMonth" 0 0 DateTime "LocalDateTimeThisMonth()" "The first day of this month.";
    LocalDateTimeLastMonth "LocalDateTimeLastMonth" 0 0 DateTime "LocalDateTimeLastMonth()" "The first day of last month.";
    LocalDateTimeNextMonth "LocalDateTimeNextMonth" 0 0 DateTime "LocalDateTimeNextMonth()" "The first day of next month.";
    LocalDateTimeTwoMonthsAway "LocalDateTimeTwoMonthsAway" 0 0 DateTime "LocalDateTimeTwoMonthsAway()" "The first day of the month after next.";
    LocalDateTimeThisYear "LocalDateTimeThisYear" 0 0 DateTime "LocalDateTimeThisYear()" "The first day of this year.";
    LocalDateTimeLastYear "LocalDateTimeLastYear" 0 0 DateTime "LocalDateTimeLastYear()" "The first day of last year.";
    LocalDateTimeNextYear "LocalDateTimeNextYear" 0 0 DateTime "LocalDateTimeNextYear()" "The first day of next year.";
    LocalDateTimeTwoYearsAway "LocalDateTimeTwoYearsAway" 0 0 DateTime "LocalDateTimeTwoYearsAway()" "The first day of the year after next.";
    LocalDateTimeYearBeforeToday "LocalDateTimeYearBeforeToday" 0 0 DateTime "LocalDateTimeYearBeforeToday()" "Today, one year ago.";
    // Date and time: predicates
    InDateRange "InDateRange" 3 3 DateTime "InDateRange(date, from, to)" "Whether a date's day is in a range, ends included.";
    InTimeRange "InTimeRange" 3 3 DateTime "InTimeRange(date, from, to)" "Whether a time of day is in a range, which may span midnight.";
    OutOfTimeRange "OutOfTimeRange" 3 3 DateTime "OutOfTimeRange(date, from, to)" "Whether a time of day is outside a range.";
    IsSameDay "IsSameDay" 2 255 DateTime "IsSameDay(date, other, ...)" "Whether a date falls on the same day as any other.";
    IsThisWeek "IsThisWeek" 1 1 DateTime "IsThisWeek(date)" "Whether the date is in this week.";
    IsThisMonth "IsThisMonth" 1 1 DateTime "IsThisMonth(date)" "Whether the date is in this month.";
    IsLastMonth "IsLastMonth" 1 1 DateTime "IsLastMonth(date)" "Whether the date is in last month.";
    IsNextMonth "IsNextMonth" 1 1 DateTime "IsNextMonth(date)" "Whether the date is in next month.";
    IsThisYear "IsThisYear" 1 1 DateTime "IsThisYear(date)" "Whether the date is in this year.";
    IsLastYear "IsLastYear" 1 1 DateTime "IsLastYear(date)" "Whether the date is in last year.";
    IsNextYear "IsNextYear" 1 1 DateTime "IsNextYear(date)" "Whether the date is in next year.";
    IsYearToDate "IsYearToDate" 1 1 DateTime "IsYearToDate(date)" "Whether the date is from the start of this year up to today.";
    IsJanuary "IsJanuary" 1 1 DateTime "IsJanuary(date)" "Whether the date is in January.";
    IsFebruary "IsFebruary" 1 1 DateTime "IsFebruary(date)" "Whether the date is in February.";
    IsMarch "IsMarch" 1 1 DateTime "IsMarch(date)" "Whether the date is in March.";
    IsApril "IsApril" 1 1 DateTime "IsApril(date)" "Whether the date is in April.";
    IsMay "IsMay" 1 1 DateTime "IsMay(date)" "Whether the date is in May.";
    IsJune "IsJune" 1 1 DateTime "IsJune(date)" "Whether the date is in June.";
    IsJuly "IsJuly" 1 1 DateTime "IsJuly(date)" "Whether the date is in July.";
    IsAugust "IsAugust" 1 1 DateTime "IsAugust(date)" "Whether the date is in August.";
    IsSeptember "IsSeptember" 1 1 DateTime "IsSeptember(date)" "Whether the date is in September.";
    IsOctober "IsOctober" 1 1 DateTime "IsOctober(date)" "Whether the date is in October.";
    IsNovember "IsNovember" 1 1 DateTime "IsNovember(date)" "Whether the date is in November.";
    IsDecember "IsDecember" 1 1 DateTime "IsDecember(date)" "Whether the date is in December.";
    // Reporting and row context
    FormatString "FormatString" 1 255 Reporting "FormatString('{0:C2}', value, ...)" "Formats values with .NET-style format strings.";
    Rgb "Rgb" 3 3 Reporting "Rgb(r, g, b)" "A color as #rrggbb text.";
    Argb "Argb" 4 4 Reporting "Argb(a, r, g, b)" "A color as #rrggbbaa text.";
    PageNumber "PageNumber" 0 0 Reporting "PageNumber()" "The page being printed.";
    PageCount "PageCount" 0 0 Reporting "PageCount()" "The number of pages.";
    CurrentRowIndexInGroup "CurrentRowIndexInGroup" 0 0 Reporting "CurrentRowIndexInGroup()" "The row's position in its group, from 0.";
    PrevRowColumnValue "PrevRowColumnValue" 1 1 Reporting "PrevRowColumnValue(value)" "The value on the previous row of the scope.";
    NextRowColumnValue "NextRowColumnValue" 1 1 Reporting "NextRowColumnValue(value)" "The value on the next row of the scope.";
}

impl Function {
    /// The catalog entry.
    pub fn info(self) -> &'static FunctionInfo {
        // The table holds every variant, in declaration order.
        &FUNCTIONS[self as usize].3
    }

    /// The name, as written in expressions.
    pub fn name(self) -> &'static str {
        self.info().name
    }

    /// The fewest and most arguments it takes.
    pub fn arity(self) -> (usize, usize) {
        let e = &FUNCTIONS[self as usize];
        (e.1, e.2)
    }

    /// The function with this name, in any case. `Min` and `Max` are the
    /// two-argument forms; with one argument they are aggregates.
    pub fn from_name(name: &str) -> Option<Self> {
        FUNCTIONS.iter().find(|e| e.3.name.eq_ignore_ascii_case(name)).map(|e| e.0)
    }

    /// Every built-in function.
    pub fn all() -> impl Iterator<Item = Function> {
        FUNCTIONS.iter().map(|e| e.0)
    }
}

/// Aggregate functions: summaries over the rows in scope or a nested collection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[allow(missing_docs, reason = "documented by Aggregate::info")]
pub enum Aggregate {
    Sum,
    Count,
    Avg,
    Min,
    Max,
    Exists,
    Single,
    Join,
    First,
    Last,
    Median,
    Var,
    VarP,
    StdDev,
    StdDevP,
    RunningSum,
    RecordNumber,
    Percentage,
    WeightedAvg,
    DistinctSum,
    DistinctCount,
    DistinctAvg,
    DistinctVar,
    DistinctVarP,
    DistinctStdDev,
    DistinctStdDevP,
}

/// (aggregate, name, alias, fewest arguments, most arguments, signature, description)
const AGGREGATES: &[(Aggregate, &str, &str, usize, usize, &str, &str)] = &[
    (Aggregate::Sum, "Sum", "sumSum", 1, 1, "Sum(value)", "The total."),
    (
        Aggregate::Count,
        "Count",
        "sumCount",
        0,
        1,
        "Count([value])",
        "The number of rows, or of non-null values.",
    ),
    (Aggregate::Avg, "Avg", "sumAvg", 1, 1, "Avg(value)", "The mean."),
    (Aggregate::Min, "Min", "sumMin", 1, 1, "Min(value)", "The smallest value."),
    (Aggregate::Max, "Max", "sumMax", 1, 1, "Max(value)", "The largest value."),
    (
        Aggregate::Exists,
        "Exists",
        "",
        0,
        1,
        "Exists([condition])",
        "Whether any row exists, or satisfies the condition.",
    ),
    (Aggregate::Single, "Single", "", 0, 1, "Single([value])", "The value of the only row; null otherwise."),
    (Aggregate::Join, "Join", "", 1, 2, "Join(value[, separator])", "The values joined as text."),
    (Aggregate::First, "First", "", 1, 1, "First(value)", "The value on the first row."),
    (Aggregate::Last, "Last", "", 1, 1, "Last(value)", "The value on the last row."),
    (Aggregate::Median, "Median", "sumMedian", 1, 1, "Median(value)", "The middle value."),
    (Aggregate::Var, "Var", "sumVar", 1, 1, "Var(value)", "Sample variance."),
    (Aggregate::VarP, "VarP", "sumVarP", 1, 1, "VarP(value)", "Population variance."),
    (Aggregate::StdDev, "StdDev", "sumStdDev", 1, 1, "StdDev(value)", "Sample standard deviation."),
    (Aggregate::StdDevP, "StdDevP", "sumStdDevP", 1, 1, "StdDevP(value)", "Population standard deviation."),
    (
        Aggregate::RunningSum,
        "RunningSum",
        "sumRunningSum",
        1,
        1,
        "RunningSum(value)",
        "The total up to the current row.",
    ),
    (
        Aggregate::RecordNumber,
        "RecordNumber",
        "sumRecordNumber",
        0,
        0,
        "RecordNumber()",
        "The row's number in the scope, from 1.",
    ),
    (
        Aggregate::Percentage,
        "Percentage",
        "sumPercentage",
        1,
        1,
        "Percentage(value)",
        "The row's value as a fraction of the total.",
    ),
    (
        Aggregate::WeightedAvg,
        "WeightedAvg",
        "sumWAvg",
        2,
        2,
        "WeightedAvg(value, weight)",
        "The mean weighted by a second value.",
    ),
    (
        Aggregate::DistinctSum,
        "DistinctSum",
        "sumDSum",
        1,
        1,
        "DistinctSum(value)",
        "The total of the distinct values.",
    ),
    (
        Aggregate::DistinctCount,
        "DistinctCount",
        "sumDCount",
        1,
        1,
        "DistinctCount(value)",
        "The number of distinct values.",
    ),
    (
        Aggregate::DistinctAvg,
        "DistinctAvg",
        "sumDAvg",
        1,
        1,
        "DistinctAvg(value)",
        "The mean of the distinct values.",
    ),
    (
        Aggregate::DistinctVar,
        "DistinctVar",
        "sumDVar",
        1,
        1,
        "DistinctVar(value)",
        "Sample variance of the distinct values.",
    ),
    (
        Aggregate::DistinctVarP,
        "DistinctVarP",
        "sumDVarP",
        1,
        1,
        "DistinctVarP(value)",
        "Population variance of the distinct values.",
    ),
    (
        Aggregate::DistinctStdDev,
        "DistinctStdDev",
        "sumDStdDev",
        1,
        1,
        "DistinctStdDev(value)",
        "Sample standard deviation of the distinct values.",
    ),
    (
        Aggregate::DistinctStdDevP,
        "DistinctStdDevP",
        "sumDStdDevP",
        1,
        1,
        "DistinctStdDevP(value)",
        "Population standard deviation of the distinct values.",
    ),
];

impl Aggregate {
    fn entry(
        self,
    ) -> &'static (Aggregate, &'static str, &'static str, usize, usize, &'static str, &'static str) {
        &AGGREGATES[self as usize]
    }

    /// The name, as written in expressions.
    pub fn name(self) -> &'static str {
        self.entry().1
    }

    /// The fewest and most arguments it takes.
    pub fn arity(self) -> (usize, usize) {
        (self.entry().3, self.entry().4)
    }

    /// The catalog entry.
    pub fn info(self) -> FunctionInfo {
        let e = self.entry();
        FunctionInfo { name: e.1, category: Category::Aggregate, signature: e.5, description: e.6 }
    }

    /// The aggregate with this name or `sum`-prefixed alias, in any case.
    pub fn from_name(name: &str) -> Option<Self> {
        AGGREGATES
            .iter()
            .find(|e| e.1.eq_ignore_ascii_case(name) || (!e.2.is_empty() && e.2.eq_ignore_ascii_case(name)))
            .map(|e| e.0)
    }

    /// Every aggregate.
    pub fn all() -> impl Iterator<Item = Aggregate> {
        AGGREGATES.iter().map(|e| e.0)
    }
}

/// The whole catalog, for expression editors: every function and aggregate.
pub fn catalog() -> Vec<FunctionInfo> {
    Function::all().map(|f| *f.info()).chain(Aggregate::all().map(Aggregate::info)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_match_their_enums() {
        for (i, f) in Function::all().enumerate() {
            assert_eq!(f as usize, i, "{f:?}");
            assert_eq!(Function::from_name(f.name()).map(Function::name), Some(f.name()));
            assert!(f.info().signature.starts_with(f.name()), "{f:?}");
        }
        for (i, a) in Aggregate::all().enumerate() {
            assert_eq!(a as usize, i, "{a:?}");
            assert_eq!(Aggregate::from_name(a.name()), Some(a));
        }
        assert_eq!(Aggregate::from_name("SUMSUM"), Some(Aggregate::Sum));
        assert_eq!(Aggregate::from_name("sumWAvg"), Some(Aggregate::WeightedAvg));
        assert!(catalog().len() > 130);
    }
}
