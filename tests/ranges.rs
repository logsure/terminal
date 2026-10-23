//! Parsing `:range` windows.

use logsure::range::{Window, day_number, day_text, parse};

/// Saturday 2026-10-24.
fn today() -> i64 {
    day_number(2026, 10, 24)
}

fn days(input: &str) -> (String, String, String) {
    match parse(input, today()).unwrap_or_else(|e| panic!("{input}: {e}")) {
        Window::Days { from, to, label } => (day_text(from), day_text(to), label),
        other => panic!("{input}: {other:?}"),
    }
}

fn query(input: &str) -> Vec<(&'static str, String)> {
    parse(input, today()).unwrap().query()
}

#[test]
fn presets_go_to_the_server_as_they_are() {
    for key in ["30m", "1h", "today", "24h", "7d", "30d", "90d", "12mo", "all"] {
        assert_eq!(query(key), vec![("range", key.to_string())]);
    }
    assert_eq!(parse("7D", today()).unwrap(), Window::Preset("7d"));
    assert_eq!(parse("all", today()).unwrap().label(), "All time");
}

#[test]
fn lengths_back_from_today() {
    assert_eq!(days("14d"), ("2026-10-11".into(), "2026-10-24".into(), "Last 14 days".into()));
    assert_eq!(days("1d").2, "Last day");
    assert_eq!(days("2w").0, "2026-10-11");
    assert_eq!(days("3mo").0, "2026-07-25");
    assert_eq!(days("6months").0, "2026-04-25");
    assert_eq!(days("2y"), ("2024-10-25".into(), "2026-10-24".into(), "Last 2 years".into()));
    assert_eq!(query("14d"), vec![("from", "2026-10-11".to_string()), ("to", "2026-10-24".to_string())]);
}

#[test]
fn month_lengths_keep_to_real_days() {
    // One month back from 31 March is the last day of February.
    let march_31 = day_number(2026, 3, 31);
    match parse("1mo", march_31).unwrap() {
        Window::Days { from, .. } => assert_eq!(day_text(from), "2026-03-01"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn named_stretches() {
    assert_eq!(days("yesterday"), ("2026-10-23".into(), "2026-10-23".into(), "Yesterday".into()));
    assert_eq!(days("thisweek").0, "2026-10-19", "weeks start on Monday");
    assert_eq!(days("lastweek"), ("2026-10-12".into(), "2026-10-18".into(), "Last week".into()));
    assert_eq!(days("thismonth"), ("2026-10-01".into(), "2026-10-24".into(), "This month".into()));
    assert_eq!(days("lastmonth"), ("2026-09-01".into(), "2026-09-30".into(), "Last month".into()));
    assert_eq!(days("thisyear").0, "2026-01-01");
    assert_eq!(days("lastyear"), ("2025-01-01".into(), "2025-12-31".into(), "Last year".into()));
}

#[test]
fn lastmonth_in_january() {
    match parse("lastmonth", day_number(2027, 1, 10)).unwrap() {
        Window::Days { from, to, .. } => {
            assert_eq!((day_text(from), day_text(to)), ("2026-12-01".into(), "2026-12-31".into()))
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn dates_and_spans() {
    assert_eq!(days("2026-09-15"), ("2026-09-15".into(), "2026-09-15".into(), "2026-09-15".into()));
    assert_eq!(days("2026-02"), ("2026-02-01".into(), "2026-02-28".into(), "2026-02-01 – 2026-02-28".into()));
    assert_eq!(days("2024-02").1, "2024-02-29", "leap years");
    assert_eq!(days("2025"), ("2025-01-01".into(), "2025-12-31".into(), "2025-01-01 – 2025-12-31".into()));
    assert_eq!(days("2026-09-01..2026-09-15").1, "2026-09-15");
    assert_eq!(days("2026-06..2026-08"), ("2026-06-01".into(), "2026-08-31".into(), "2026-06-01 – 2026-08-31".into()));
    assert_eq!(days("2026-09..").1, "2026-10-24", "an open end runs to today");
    assert_eq!(days("2026-09-15..2026-09-01").0, "2026-09-01", "ends given backwards are swapped");
}

#[test]
fn windows_stop_at_today() {
    assert_eq!(days("2026").1, "2026-10-24");
    assert_eq!(days("2026-10").1, "2026-10-24");
    assert!(parse("2027-01", today()).unwrap_err().contains("after today"));
}

#[test]
fn bounds_for_lists_the_server_does_not_window() {
    let w = parse("lastmonth", today()).unwrap();
    let (start, end) = w.bounds(0);
    assert_eq!(start, day_number(2026, 9, 1) * 86_400_000);
    assert_eq!(end, day_number(2026, 10, 1) * 86_400_000);
    assert_eq!(parse("all", today()).unwrap().bounds(5), (i64::MIN, i64::MAX));
}

#[test]
fn mistakes() {
    for bad in ["", "8x", "0d", "2026-13", "2026-02-30", "26-09-01", "..2026-09-01", "last", "d"] {
        assert!(parse(bad, today()).is_err(), "{bad:?} should be refused");
    }
}
