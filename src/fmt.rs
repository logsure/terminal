//! Numbers, times and changes, written for narrow columns.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// `950`, `9.5k`, `12k`, `1.2M`.
pub fn count(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=9_999 => trim(format!("{:.1}k", n as f64 / 1e3)),
        10_000..=999_999 => format!("{}k", (n as f64 / 1e3).round() as u64),
        _ => trim(format!("{:.1}M", n as f64 / 1e6)),
    }
}

fn trim(s: String) -> String {
    s.replace(".0k", "k").replace(".0M", "M")
}

/// The change from `prev` to `now`: `+12%`, `-3%`, `new` or empty when there is nothing to compare.
pub fn change(now: u64, prev: Option<u64>) -> String {
    match prev {
        None => String::new(),
        Some(0) if now == 0 => String::from("0%"),
        Some(0) => String::from("new"),
        Some(p) => {
            let pct = (now as f64 - p as f64) / p as f64 * 100.0;
            format!("{}{:.0}%", if pct > 0.0 { "+" } else { "" }, pct)
        }
    }
}

/// A share, 0 to 1, as a percentage.
pub fn percent(share: Option<f64>) -> String {
    match share {
        Some(s) if s < 0.1 => format!("{:.1}%", s * 100.0),
        Some(s) => format!("{:.0}%", s * 100.0),
        None => String::from("–"),
    }
}

/// Milliseconds as `45s`, `2m 18s`, `1h 5m`.
pub fn duration(ms: Option<f64>) -> String {
    let Some(ms) = ms else { return String::from("–") };
    let s = (ms / 1000.0).round() as u64;
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m {}s", s / 60, s % 60),
        _ => format!("{}h {}m", s / 3600, (s % 3600) / 60),
    }
}

/// How long ago, from `now`: `now`, `5m`, `3h`, `2d`, `5w`, `1y`.
pub fn ago(at: i64, now: i64) -> String {
    let s = ((now - at).max(0) / 1000) as u64;
    match s {
        0..=59 => String::from("now"),
        60..=3599 => format!("{}m", s / 60),
        3600..=86_399 => format!("{}h", s / 3600),
        86_400..=1_209_599 => format!("{}d", s / 86_400),
        1_209_600..=31_535_999 => format!("{}w", s / 604_800),
        _ => format!("{}y", s / 31_536_000),
    }
}

/// A UTC date and time, `2026-10-24 14:05`.
pub fn datetime(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = crate::range::civil(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rest / 3600, (rest % 3600) / 60)
}

/// One line of block characters for a series.
pub fn spark(values: &[u64]) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = values.iter().copied().max().unwrap_or(0);
    values
        .iter()
        .map(|&v| if max == 0 { BARS[0] } else { BARS[((v as f64 / max as f64) * 7.0).round() as usize] })
        .collect()
}

/// Shortens text to `width` characters, ending with `…` when cut.
pub fn clip(text: &str, width: usize) -> String {
    let one_line: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    if one_line.chars().count() <= width {
        return one_line;
    }
    let mut out: String = one_line.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts() {
        assert_eq!(count(950), "950");
        assert_eq!(count(1000), "1k");
        assert_eq!(count(9_540), "9.5k");
        assert_eq!(count(23_471), "23k");
        assert_eq!(count(1_250_000), "1.2M");
    }

    #[test]
    fn changes() {
        assert_eq!(change(2283, Some(2197)), "+4%");
        assert_eq!(change(549, Some(560)), "-2%");
        assert_eq!(change(5, Some(0)), "new");
        assert_eq!(change(5, None), "");
    }

    #[test]
    fn times() {
        assert_eq!(duration(Some(138_916.7)), "2m 19s");
        assert_eq!(ago(0, 90 * 60_000), "1h");
        assert_eq!(datetime(1_792_195_200_000), "2026-10-17 00:00");
    }

    #[test]
    fn sparks_and_clips() {
        assert_eq!(spark(&[0, 5, 10]), "▁▅█");
        assert_eq!(spark(&[0, 0]), "▁▁");
        assert_eq!(clip("abcdef", 4), "abc…");
        assert_eq!(clip("a\nb", 4), "a b");
    }
}
