//! The LOGSURE banner across the top of the console, drawn when the terminal is tall and wide enough.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::ui::accent;

/// The banner's rows, plus a blank one under it.
pub const HEIGHT: u16 = 7;
/// Below this many rows the console keeps its one-line title, so lists keep their room.
pub const MIN_ROWS: u16 = 32;

/// Block letters, one row each, all rows of a letter the same width.
const LETTERS: [[&str; 6]; 7] = [
    ["██╗     ", "██║     ", "██║     ", "██║     ", "███████╗", "╚══════╝"],
    [" ██████╗ ", "██╔═══██╗", "██║   ██║", "██║   ██║", "╚██████╔╝", " ╚═════╝ "],
    [" ██████╗ ", "██╔════╝ ", "██║  ███╗", "██║   ██║", "╚██████╔╝", " ╚═════╝ "],
    ["███████╗", "██╔════╝", "███████╗", "╚════██║", "███████║", "╚══════╝"],
    ["██╗   ██╗", "██║   ██║", "██║   ██║", "██║   ██║", "╚██████╔╝", " ╚═════╝ "],
    ["██████╗ ", "██╔══██╗", "██████╔╝", "██╔══██╗", "██║  ██║", "╚═╝  ╚═╝"],
    ["███████╗", "██╔════╝", "█████╗  ", "██╔══╝  ", "███████╗", "╚══════╝"],
];

/// The banner as plain text, one string per row.
pub fn rows() -> Vec<String> {
    (0..6).map(|r| LETTERS.iter().map(|letter| letter[r]).collect()).collect()
}

/// How many columns the banner takes.
pub fn width() -> u16 {
    rows()[0].chars().count() as u16
}

/// Whether the banner fits on a screen of this size.
pub fn fits(area: Rect) -> bool {
    area.height >= MIN_ROWS && area.width >= width() + 2
}

/// Draws the banner with the solid blocks in the accent and their shadows dimmed, and a few words beside it when
/// there is room.
pub fn draw(frame: &mut Frame, area: Rect, server: &str, url: &str) {
    let side = [
        String::new(),
        String::new(),
        String::from("analytics · inbox · waitlists"),
        format!("{server}  {url}"),
        format!("v{}  ·  ? help  ·  : command", env!("CARGO_PKG_VERSION")),
        String::new(),
    ];
    let room_for_side = area.width >= width() + 6 + 30;
    let block = Style::new().fg(accent()).add_modifier(Modifier::BOLD);
    let shadow = Style::new().fg(accent()).add_modifier(Modifier::DIM);
    let lines: Vec<Line> = rows()
        .into_iter()
        .zip(side)
        .map(|(row, words)| {
            let mut spans = vec![Span::raw(" ")];
            // Runs of the same kind of character share one span.
            let mut run = String::new();
            let mut solid = None;
            for c in row.chars() {
                let is_solid = c == '█';
                if solid.is_some_and(|s| s != is_solid) {
                    spans
                        .push(Span::styled(std::mem::take(&mut run), if solid == Some(true) { block } else { shadow }));
                }
                solid = Some(is_solid);
                run.push(c);
            }
            spans.push(Span::styled(run, if solid == Some(true) { block } else { shadow }));
            if room_for_side && !words.is_empty() {
                spans.push(Span::raw("    "));
                spans.push(Span::styled(words, Style::new().add_modifier(Modifier::DIM)));
            }
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_is_as_wide_as_the_first() {
        let rows = rows();
        assert_eq!(rows.len(), 6);
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row.chars().count(), rows[0].chars().count(), "row {i}: {row}");
        }
        for letter in LETTERS {
            let w = letter[0].chars().count();
            assert!(letter.iter().all(|r| r.chars().count() == w), "{letter:?}");
        }
    }

    #[test]
    fn fits_only_on_roomy_screens() {
        assert_eq!(width(), 59);
        assert!(fits(Rect::new(0, 0, 80, 40)));
        assert!(!fits(Rect::new(0, 0, 80, 24)), "short terminals keep the one-line title");
        assert!(!fits(Rect::new(0, 0, 50, 40)), "narrow terminals too");
    }
}
