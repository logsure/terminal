//! Time windows, as typed after `:range` or `--range`.
//!
//! The server's presets (`30m 1h today 24h 7d 30d 90d 12mo all`) go to it as `range=`. Everything else becomes whole
//! UTC days sent as `from` and `to` (both inclusive):
//!
//! - a length back from today: `14d`, `6w`, `3mo`, `2y`
//! - a named stretch: `yesterday`, `thisweek`, `lastweek`, `thismonth`, `lastmonth`, `thisyear`, `lastyear`
//! - a day, a month or a year: `2026-09-15`, `2026-09`, `2026`
//! - a span between two of those: `2026-09-01..2026-09-15`, `2026-06..2026-08`, `2026-09..` (up to today)
//!
//! Relative windows are worked out again on every request, so `14d` keeps meaning the last 14 days.

use crate::fmt;

/// The server's own windows (`RangeKey` in the contract).
pub const PRESETS: [&str; 9] = ["30m", "1h", "today", "24h", "7d", "30d", "90d", "12mo", "all"];

const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Window {
    /// One of [`PRESETS`].
    Preset(&'static str),
    /// Whole UTC days, as days since 1970-01-01, both ends included, with a label.
    Days { from: i64, to: i64, label: String },
}

/// What `:range` accepts, for help and errors.
pub const SYNTAX: &str = "a preset (30m 1h today 24h 7d 30d 90d 12mo all), a length (14d 6w 3mo 2y), \
yesterday, this/last week, month or year (thismonth, lastyear), a date (2026-09-15, 2026-09, 2026) or a span (2026-09-01..2026-09-15)";

/// Parses a window for the day `today` (days since 1970-01-01, UTC).
pub fn parse(input: &str, today: i64) -> Result<Window, String> {
    let text = input.trim().to_lowercase();
    if let Some(p) = PRESETS.iter().find(|p| **p == text) {
        return Ok(Window::Preset(p));
    }
    let bad = || format!("{input:?} is not a window: use {SYNTAX}");
    let (y, m, d) = civil(today);
    let days = |from: i64, to: i64, label: String| Ok(Window::Days { from, to, label });
    match text.as_str() {
        "yesterday" => return days(today - 1, today - 1, String::from("Yesterday")),
        "thisweek" | "week" => return days(today - weekday(today), today, String::from("This week")),
        "lastweek" => {
            let monday = today - weekday(today) - 7;
            return days(monday, monday + 6, String::from("Last week"));
        }
        "thismonth" | "month" => return days(day_number(y, m, 1), today, String::from("This month")),
        "lastmonth" => {
            let (ly, lm) = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
            return days(day_number(ly, lm, 1), day_number(y, m, 1) - 1, String::from("Last month"));
        }
        "thisyear" | "year" => return days(day_number(y, 1, 1), today, String::from("This year")),
        "lastyear" => return days(day_number(y - 1, 1, 1), day_number(y, 1, 1) - 1, String::from("Last year")),
        _ => {}
    }
    if let Some((n, unit)) = length(&text) {
        if n == 0 || n > 10_000 {
            return Err(bad());
        }
        let (from, label) = match unit {
            "d" => (today - (n - 1), plural(n, "day")),
            "w" => (today - (7 * n - 1), plural(n, "week")),
            "mo" => (months_back(y, m, d, n) + 1, plural(n, "month")),
            "y" => (months_back(y, m, d, 12 * n) + 1, plural(n, "year")),
            _ => return Err(bad()),
        };
        return days(from, today, format!("Last {label}"));
    }
    let (from, to) = match text.split_once("..") {
        Some((a, b)) => {
            let (from, _) = date(a).ok_or_else(bad)?;
            let to = if b.is_empty() { today } else { date(b).ok_or_else(bad)?.1 };
            (from.min(to), from.max(to))
        }
        None => date(&text).ok_or_else(bad)?,
    };
    if from > today {
        return Err(format!("{input:?} starts after today"));
    }
    let to = to.min(today);
    let label = if from == to { day_text(from) } else { format!("{} – {}", day_text(from), day_text(to)) };
    days(from, to, label)
}

impl Window {
    /// The query keys that select this window.
    pub fn query(&self) -> Vec<(&'static str, String)> {
        match self {
            Window::Preset(key) => vec![("range", key.to_string())],
            Window::Days { from, to, .. } => vec![("from", day_text(*from)), ("to", day_text(*to))],
        }
    }

    pub fn label(&self) -> String {
        match self {
            Window::Preset(key) => preset_label(key).to_string(),
            Window::Days { label, .. } => label.clone(),
        }
    }

    /// The window in milliseconds, `[start, end)`, for lists the server does not window itself (the inbox and the
    /// waitlist). Follows the server's rules: whole UTC days for day ranges, no bounds for `all`.
    pub fn bounds(&self, now: i64) -> (i64, i64) {
        const MIN: i64 = 60_000;
        let midnight = now.div_euclid(DAY_MS) * DAY_MS;
        let start = match self {
            Window::Preset("30m") => now - 30 * MIN,
            Window::Preset("1h") => now - 60 * MIN,
            Window::Preset("24h") => now - DAY_MS,
            Window::Preset("today") => midnight,
            Window::Preset("7d") => midnight - 6 * DAY_MS,
            Window::Preset("30d") => midnight - 29 * DAY_MS,
            Window::Preset("90d") => midnight - 89 * DAY_MS,
            Window::Preset("12mo") => midnight - 364 * DAY_MS,
            Window::Preset(_) => i64::MIN,
            Window::Days { from, to, .. } => return (from * DAY_MS, (to + 1) * DAY_MS),
        };
        (start, i64::MAX)
    }
}

/// Today, as days since 1970-01-01 (UTC).
pub fn today() -> i64 {
    fmt::now_ms().div_euclid(DAY_MS)
}

fn preset_label(key: &str) -> &'static str {
    match key {
        "30m" => "Last 30 minutes",
        "1h" => "Last hour",
        "today" => "Today",
        "24h" => "Last 24 hours",
        "7d" => "Last 7 days",
        "30d" => "Last 30 days",
        "90d" => "Last 90 days",
        "12mo" => "Last 12 months",
        _ => "All time",
    }
}

fn plural(n: i64, unit: &str) -> String {
    if n == 1 { unit.to_string() } else { format!("{n} {unit}s") }
}

/// `14d` → (14, "d"); also `mo`, `w`, `y`.
fn length(text: &str) -> Option<(i64, &str)> {
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 5 {
        return None;
    }
    let (n, unit) = text.split_at(digits);
    let unit = match unit {
        "d" | "day" | "days" => "d",
        "w" | "wk" | "week" | "weeks" => "w",
        "mo" | "month" | "months" => "mo",
        "y" | "yr" | "year" | "years" => "y",
        _ => return None,
    };
    Some((n.parse().ok()?, unit))
}

/// `2026`, `2026-09` or `2026-09-15` as its first and last day.
fn date(text: &str) -> Option<(i64, i64)> {
    let parts: Vec<&str> = text.trim().split('-').collect();
    let num = |s: &str, len: usize| {
        (s.len() == len && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<i64>().ok()).flatten()
    };
    match parts.as_slice() {
        [y] => {
            let y = num(y, 4)?;
            Some((day_number(y, 1, 1), day_number(y + 1, 1, 1) - 1))
        }
        [y, m] => {
            let (y, m) = (num(y, 4)?, num(m, 2)?);
            if !(1..=12).contains(&m) {
                return None;
            }
            let next = if m == 12 { day_number(y + 1, 1, 1) } else { day_number(y, m as u32 + 1, 1) };
            Some((day_number(y, m as u32, 1), next - 1))
        }
        [y, m, d] => {
            let (y, m, d) = (num(y, 4)?, num(m, 2)?, num(d, 2)?);
            let day = day_number(y, m as u32, d as u32);
            // Rejects 2026-02-30 and the like: the day must come back as written.
            (civil(day) == (y, m as u32, d as u32)).then_some((day, day))
        }
        _ => None,
    }
}

/// The day `n` months before `y-m-d`, keeping the day of the month where it exists.
fn months_back(y: i64, m: u32, d: u32, n: i64) -> i64 {
    let total = y * 12 + (m as i64 - 1) - n;
    let (ty, tm) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
    let next = if tm == 12 { day_number(ty + 1, 1, 1) } else { day_number(ty, tm + 1, 1) };
    (day_number(ty, tm, 1) + d as i64 - 1).min(next - 1)
}

/// 0 for Monday … 6 for Sunday. 1970-01-01 was a Thursday.
fn weekday(day: i64) -> i64 {
    (day + 3).rem_euclid(7)
}

/// `YYYY-MM-DD`.
pub fn day_text(day: i64) -> String {
    let (y, m, d) = civil(day);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A calendar date to days since 1970-01-01 (Howard Hinnant's algorithm).
pub fn day_number(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Days since 1970-01-01 to a calendar date.
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}
