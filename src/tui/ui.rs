//! Drawing: a title line, the view, and a line for commands, filters and messages. Flat, with one accent colour.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Sparkline, Table, TableState, Wrap};

use super::app::{App, Kind, Mode, View};
use crate::api::{Dashboard, Row as ApiRow, Ticket};
use crate::fmt;

/// LogSure's merlot, #a3172f, or the nearest of the 256 colours when the terminal has no true colour.
pub fn accent() -> Color {
    let truecolor = std::env::var("COLORTERM").is_ok_and(|v| v == "truecolor" || v == "24bit");
    if truecolor { Color::Rgb(0xa3, 0x17, 0x2f) } else { Color::Indexed(124) }
}

fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

pub fn draw(frame: &mut Frame, app: &App) {
    let [title, body, bottom] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
    draw_title(frame, title, app);
    let view = app.top();
    match &view.kind {
        Kind::App { data, .. } => match data {
            Some(d) => draw_dashboard(frame, body, d, &view.filter),
            None => draw_empty(frame, body, view),
        },
        Kind::Ticket { data, .. } => match data {
            Some(t) => draw_ticket(frame, body, app, t, view.scroll),
            None => draw_empty(frame, body, view),
        },
        Kind::Event { data, .. } => {
            let spark: Option<Vec<u64>> = data.as_ref().map(|e| e.series.iter().map(|p| p.count).collect());
            match spark {
                Some(series) if !series.is_empty() => {
                    let [top, rest] = Layout::vertical([Constraint::Length(5), Constraint::Min(1)]).areas(body);
                    let total: u64 = series.iter().sum();
                    draw_spark(frame, top, &format!("Events  {}", fmt::count(total)), &series);
                    draw_list(frame, rest, app);
                }
                _ => draw_list(frame, body, app),
            }
        }
        Kind::Waitlist { data: Some(w), .. } => {
            let summary: Vec<Span> = w
                .lists
                .iter()
                .flat_map(|l| {
                    [
                        Span::styled(format!("{} › {}", app.app_name(&l.app_id), l.list), bold()),
                        Span::raw(format!("  {} waiting · {} invited · {} joined    ", l.waiting, l.invited, l.joined)),
                    ]
                })
                .collect();
            let [top, rest] = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(body);
            frame.render_widget(Paragraph::new(Line::from(summary)).wrap(Wrap { trim: true }), pad(top));
            draw_list(frame, rest, app);
        }
        _ => draw_list(frame, body, app),
    }
    draw_bottom(frame, bottom, app);
    if app.help {
        draw_help(frame, frame.area());
    }
}

/// One column of padding on each side.
fn pad(area: Rect) -> Rect {
    Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area }
}

fn draw_title(frame: &mut Frame, area: Rect, app: &App) {
    let view = app.top();
    let mut spans = vec![
        Span::styled(" LogSure ", Style::new().bg(accent()).fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(app.server.name.clone(), dim()),
        Span::raw("  "),
        Span::styled(view.title(app), bold()),
    ];
    let mut right = Vec::new();
    if view.uses_range() {
        right.push(app.window().label());
    }
    if view.loading {
        right.push(String::from("loading…"));
    } else if let Some(at) = view.loaded_at {
        right.push(format!("updated {}", fmt::ago(at, fmt::now_ms())));
    }
    let right = format!("{} ", right.join("  ·  "));
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let gap = (area.width as usize).saturating_sub(used + right.chars().count());
    spans.push(Span::raw(" ".repeat(gap)));
    spans.push(Span::styled(right, dim()));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_bottom(frame: &mut Frame, area: Rect, app: &App) {
    let line = match &app.mode {
        Mode::Command(text) => {
            Line::from(vec![Span::styled(":", Style::new().fg(accent())), Span::raw(format!("{text}▏"))])
        }
        Mode::Filter(text) => {
            Line::from(vec![Span::styled("/", Style::new().fg(accent())), Span::raw(format!("{text}▏"))])
        }
        Mode::Note { text, .. } => {
            Line::from(vec![Span::styled("note: ", Style::new().fg(accent())), Span::raw(format!("{text}▏"))])
        }
        Mode::Normal => match &app.status {
            Some((text, true)) => {
                Line::from(Span::styled(format!(" {text}"), Style::new().fg(accent()).add_modifier(Modifier::BOLD)))
            }
            Some((text, false)) => Line::from(format!(" {text}")),
            None => {
                let mut hints = String::from(" ");
                if !app.top().filter.is_empty() {
                    hints.push_str(&format!("/{}  (Esc clears)   ", app.top().filter));
                }
                hints.push_str(hints_for(app.top()));
                Line::from(Span::styled(hints, dim()))
            }
        },
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn hints_for(view: &View) -> &'static str {
    match view.kind {
        Kind::Apps(_) => "Enter open  : command  / filter  ? help  q quit",
        Kind::App { .. } => "e events  / filter  : command  ? help  q back",
        Kind::Events { .. } => "Enter properties  / filter  ? help  q back",
        Kind::Event { .. } => "/ filter  ? help  q back",
        Kind::Inbox { .. } => "Enter read  r resolve  n note  t kind  s status  / filter  q back",
        Kind::Ticket { .. } => "r resolve  n note  ↑↓ scroll  q back",
        Kind::Waitlist { .. } => "i invited  j joined  w waiting  l list  s status  / filter  q back",
        Kind::Servers(_) => "Enter switch  q back",
    }
}

fn draw_empty(frame: &mut Frame, area: Rect, view: &View) {
    let text = match &view.error {
        Some(e) => Line::from(Span::styled(e.clone(), Style::new().fg(accent()))),
        None if view.loading => Line::from(Span::styled("loading…", dim())),
        None => Line::from(Span::styled("nothing here", dim())),
    };
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), pad(area.inner(Margin::new(0, 1))));
}

/// The columns of each list: header, width and whether the numbers are right-aligned.
fn columns(kind: &Kind) -> Vec<(&'static str, Constraint, bool)> {
    use Constraint::{Fill, Length};
    match kind {
        Kind::Apps(_) => vec![
            ("App", Fill(2), false),
            ("Visitors", Length(9), true),
            ("Pageviews", Length(10), true),
            ("Change", Length(7), true),
            ("Now", Length(5), true),
            ("", Length(1), false),
            ("Trend", Fill(2), false),
        ],
        Kind::Events { .. } => vec![
            ("Event", Fill(3), false),
            ("Visitors", Length(9), true),
            ("Count", Length(8), true),
            ("Conversion", Length(11), true),
            ("Value", Length(10), true),
        ],
        Kind::Event { .. } => {
            vec![
                ("Property", Fill(1), false),
                ("Value", Fill(2), false),
                ("Visitors", Length(9), true),
                ("Count", Length(8), true),
            ]
        }
        Kind::Inbox { .. } => vec![
            ("Kind", Length(8), false),
            ("Status", Length(9), false),
            ("App", Length(14), false),
            ("Subject", Fill(3), false),
            ("From", Fill(1), false),
            ("Age", Length(4), true),
        ],
        Kind::Waitlist { .. } => vec![
            ("Email", Fill(3), false),
            ("Name", Fill(2), false),
            ("List", Fill(1), false),
            ("Status", Length(8), false),
            ("Source", Fill(1), false),
            ("Country", Length(7), false),
            ("Age", Length(4), true),
        ],
        Kind::Servers(_) => vec![("", Length(1), false), ("Name", Fill(1), false), ("Address", Fill(3), false)],
        Kind::App { .. } | Kind::Ticket { .. } => Vec::new(),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, app: &App) {
    let view = app.top();
    let rows = app.rows().unwrap_or_default();
    if rows.is_empty() {
        if !view.filter.is_empty() && view.error.is_none() && !view.loading {
            let text = Span::styled(format!("nothing matches /{}", view.filter), dim());
            frame.render_widget(Paragraph::new(text), pad(area.inner(Margin::new(0, 1))));
            return;
        }
        return draw_empty(frame, area, view);
    }
    let cols = columns(&view.kind);
    // The Apps view has a spacer column; its cells are pushed one to the right.
    let cells_for = |cells: &[String]| -> Vec<String> {
        if matches!(view.kind, Kind::Apps(_)) {
            let mut c = cells.to_vec();
            c.insert(5, String::new());
            c
        } else {
            cells.to_vec()
        }
    };
    let align = |text: String, right: bool| {
        Cell::from(Text::from(Line::from(text).alignment(if right { Alignment::Right } else { Alignment::Left })))
    };
    let header = Row::new(cols.iter().map(|(h, _, right)| align(h.to_string(), *right)))
        .style(bold().add_modifier(Modifier::UNDERLINED));
    let table_rows = rows
        .iter()
        .map(|r| Row::new(cells_for(&r.cells).into_iter().zip(&cols).map(|(c, (_, _, right))| align(c, *right))));
    let table = Table::new(table_rows, cols.iter().map(|(_, w, _)| *w))
        .header(header)
        .column_spacing(2)
        .row_highlight_style(Style::new().bg(accent()).fg(Color::White));
    let mut state = TableState::default().with_selected(Some(view.selected.min(rows.len() - 1)));
    let mut footer = format!("{} of {}", rows.len(), app_total(app));
    if view.error.is_some() {
        footer = format!("{footer}  ·  {}", view.error.clone().unwrap_or_default());
    }
    let [table_area, foot] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    frame.render_stateful_widget(table, pad(table_area), &mut state);
    frame.render_widget(Paragraph::new(Span::styled(footer, dim())).alignment(Alignment::Right), pad(foot));
}

/// How many rows the list has before the filter (and, for the inbox and waitlist, how many the server has).
fn app_total(app: &App) -> String {
    match &app.top().kind {
        Kind::Inbox { data: Some(p), .. } => format!("{} in the inbox", p.total),
        Kind::Waitlist { data: Some(w), .. } => format!("{} signups", w.total),
        Kind::Apps(Some(o)) => o.apps.len().to_string(),
        Kind::Events { data: Some(d), .. } => d.events.len().to_string(),
        Kind::Event { data: Some(e), .. } => e.props.len().to_string(),
        Kind::Servers(s) => s.len().to_string(),
        _ => String::from("0"),
    }
}

fn draw_spark(frame: &mut Frame, area: Rect, label: &str, series: &[u64]) {
    let block =
        Block::new().borders(Borders::BOTTOM).border_style(dim()).title(Span::styled(label.to_string(), bold()));
    let spark = Sparkline::default().block(block).data(series).style(Style::new().fg(accent()));
    frame.render_widget(spark, pad(area));
}

fn draw_dashboard(frame: &mut Frame, area: Rect, d: &Dashboard, filter: &str) {
    let t = &d.summary.totals;
    let p = d.summary.prev.as_ref();
    let metric = |label: &str, value: String, change: String| {
        vec![
            Span::styled(format!("{label} "), dim()),
            Span::styled(value, bold()),
            Span::raw(if change.is_empty() { String::from("   ") } else { format!(" {change}   ") }),
        ]
    };
    let mut line = Vec::new();
    line.extend(metric("Visitors", fmt::count(t.visitors), fmt::change(t.visitors, p.map(|p| p.visitors))));
    line.extend(metric("Visits", fmt::count(t.visits), fmt::change(t.visits, p.map(|p| p.visits))));
    line.extend(metric("Pageviews", fmt::count(t.pageviews), fmt::change(t.pageviews, p.map(|p| p.pageviews))));
    line.extend(metric(
        "Views/visit",
        t.views_per_visit.map_or(String::from("–"), |v| format!("{v:.1}")),
        String::new(),
    ));
    line.extend(metric("Bounce", fmt::percent(t.bounce_rate), String::new()));
    line.extend(metric("Avg visit", fmt::duration(t.duration), String::new()));
    line.extend(metric("Now", d.live.visitors.to_string(), String::new()));
    line.extend(metric("Events", d.events.len().to_string(), String::new()));

    let [totals, spark, tables] =
        Layout::vertical([Constraint::Length(2), Constraint::Length(6), Constraint::Min(3)]).areas(area);
    frame.render_widget(Paragraph::new(Line::from(line)).wrap(Wrap { trim: true }), pad(totals));
    let series: Vec<u64> = d.series.iter().map(|s| s.visitors).collect();
    draw_spark(frame, spark, &format!("Visitors per {}", d.range.bucket), &series);

    let direction = if area.width >= 100 { Direction::Horizontal } else { Direction::Vertical };
    let parts = Layout::default().direction(direction).constraints([Constraint::Ratio(1, 3); 3]).split(tables);
    for (i, (title, rows)) in
        [("Top pages", &d.pages), ("Sources", &d.sources), ("Countries", &d.countries)].into_iter().enumerate()
    {
        draw_rows(frame, parts[i], title, rows, filter);
    }
}

fn draw_rows(frame: &mut Frame, area: Rect, title: &str, rows: &[ApiRow], filter: &str) {
    let needle = filter.to_lowercase();
    let label = |r: &ApiRow| r.value.clone().unwrap_or_else(|| String::from("(none)"));
    let rows: Vec<Row> = rows
        .iter()
        .filter(|r| needle.is_empty() || label(r).to_lowercase().contains(&needle))
        .map(|r| {
            Row::new([
                Cell::from(label(r)),
                Cell::from(Text::from(Line::from(fmt::count(r.visitors)).alignment(Alignment::Right))),
            ])
        })
        .collect();
    let header = Row::new([
        Cell::from(title.to_string()),
        Cell::from(Text::from(Line::from("Visitors").alignment(Alignment::Right))),
    ])
    .style(bold().add_modifier(Modifier::UNDERLINED));
    let table = Table::new(rows, [Constraint::Fill(1), Constraint::Length(9)]).header(header).column_spacing(1);
    frame.render_widget(table, pad(area));
}

fn draw_ticket(frame: &mut Frame, area: Rect, app: &App, t: &Ticket, scroll: u16) {
    let mut lines: Vec<Line> = Vec::new();
    let subject = t.subject.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| fmt::clip(&t.body, 80));
    lines.push(Line::from(Span::styled(subject, bold())));
    let from = match (&t.name, &t.email) {
        (Some(n), Some(e)) => format!("{n} <{e}>"),
        (Some(n), None) => n.clone(),
        (None, Some(e)) => e.clone(),
        (None, None) => String::from("anonymous"),
    };
    lines.push(Line::from(vec![
        Span::styled(t.kind.as_str(), Style::new().fg(accent())),
        Span::styled(
            format!(
                "  ·  {}  ·  {}  ·  {}  ·  {}",
                t.status.as_str(),
                app.app_name(&t.app_id),
                from,
                fmt::datetime(t.created_at)
            ),
            dim(),
        ),
    ]));
    let mut extra = Vec::new();
    if let Some(r) = t.rating {
        extra.push(format!("{}{}", "★".repeat(r as usize), "☆".repeat(5usize.saturating_sub(r as usize))));
    }
    if let Some(c) = &t.category {
        extra.push(format!("category: {c}"));
    }
    if !extra.is_empty() {
        lines.push(Line::from(Span::styled(extra.join("  ·  "), dim())));
    }
    lines.push(Line::default());
    lines.extend(t.body.lines().map(|l| Line::from(l.to_string())));
    for (key, value) in &t.data {
        let text = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(heading(key), bold())));
        lines.extend(text.lines().map(|l| Line::from(l.to_string())));
    }
    let m = &t.meta;
    let facts: Vec<String> = [
        ("page", &m.url),
        ("browser", &m.browser),
        ("os", &m.os),
        ("device", &m.device),
        ("country", &m.country),
        ("language", &m.lang),
    ]
    .into_iter()
    .filter_map(|(k, v)| v.as_ref().map(|v| format!("{k} {v}")))
    .collect();
    if !facts.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(facts.join("  ·  "), dim())));
    }
    if !t.attachments.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled("Attachments", bold())));
        for a in &t.attachments {
            lines.push(Line::from(format!("{}  {}  {} bytes", a.name, a.mime, a.size)));
        }
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(format!("Notes ({})", t.notes.len()), bold())));
    for n in &t.notes {
        let who = if n.kind == "note" { "note" } else { n.kind.as_str() };
        lines.push(Line::from(vec![
            Span::styled(format!("{}  {who}  ", fmt::datetime(n.ts)), dim()),
            Span::raw(n.body.clone()),
        ]));
    }
    let para = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((scroll, 0));
    frame.render_widget(para, pad(area.inner(Margin::new(0, 1))));
}

/// `expected` → `Expected`.
fn heading(key: &str) -> String {
    let mut c = key.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

pub const HELP: &[(&str, &str)] = &[
    (":apps", "every app: visitors, pageviews, change, visitors now"),
    (":app <id>", "one app: totals, visitors over time, pages, sources, countries"),
    (":events [app]", "custom events with counts and conversion"),
    (":inbox [kind] [status]", "feedback, bug reports and messages"),
    (":waitlist [list] [status]", "waitlist signups"),
    (":range <window>", "7d 30d 12mo all · 14d 6w 3mo 2y · yesterday lastmonth thisyear"),
    ("", "2026-09 · 2026-09-15 · 2026-09-01..2026-09-15 · 2026-06.. — for every view"),
    (":ctx [name]", "list the servers, or switch to one"),
    (":refresh  Ctrl-R", "ask the server again (views refresh every 30 s)"),
    (":q  Ctrl-C", "quit"),
    ("", ""),
    ("↑ ↓ PgUp PgDn", "move"),
    ("Enter", "open the selected row"),
    ("/", "filter the current list; Esc clears it"),
    ("q", "back; quits from the first view"),
    ("Tab", "complete a command"),
    ("", ""),
    ("e", "app: its events"),
    ("r  n", "inbox: resolve, add a note"),
    ("t  s", "inbox: next kind, next status"),
    ("i  j  w", "waitlist: invited, joined, waiting"),
    ("l  s", "waitlist: next list, next status"),
];

fn draw_help(frame: &mut Frame, area: Rect) {
    let width = 96.min(area.width.saturating_sub(4));
    let height = (HELP.len() as u16 + 4).min(area.height.saturating_sub(2));
    let rect = Rect { x: area.x + (area.width - width) / 2, y: area.y + (area.height - height) / 2, width, height };
    let lines: Vec<Line> = HELP
        .iter()
        .map(|(k, v)| {
            Line::from(vec![
                Span::styled(format!("  {k:<28}"), Style::new().fg(accent()).add_modifier(Modifier::BOLD)),
                Span::raw(*v),
            ])
        })
        .collect();
    let block = Block::bordered()
        .border_style(Style::new().fg(accent()))
        .title(Span::styled(" Help — any key closes ", bold()));
    frame.render_widget(Clear, rect);
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}
