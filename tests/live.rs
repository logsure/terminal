//! Drives the full-screen app against a running server and draws every view into a test terminal.
//!
//! Skipped unless `LOGSURE_URL` and `LOGSURE_TOKEN` point at a server with demo data, e.g.
//! `LOGSURE_URL=http://localhost:8080 LOGSURE_TOKEN=cc_… cargo test --test live -- --nocapture`.
//! It changes data: it resolves a request, adds a note and marks a signup invited.

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use logsure::api::Client;
use logsure::tui::app::{App, Kind, Server};
use logsure::tui::ui;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn server() -> Option<Server> {
    let url = std::env::var("LOGSURE_URL").ok()?;
    let token = std::env::var("LOGSURE_TOKEN").ok()?;
    Some(Server { name: String::from("live"), client: Client::new(&url, &token) })
}

/// Waits until the view on top has loaded and `/me` has arrived.
fn settle(app: &mut App) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        app.pump(Duration::from_millis(50));
        let v = app.top();
        if !v.loading && (v.loaded_at.is_some() || matches!(v.kind, Kind::Servers(_))) && app.me.is_some() {
            // Answers to changes come in after the view's own; give them a moment.
            app.pump(Duration::from_millis(200));
            if !app.top().loading {
                return;
            }
        }
    }
    panic!("the view did not load");
}

fn screen(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(130, 36)).unwrap();
    terminal.draw(|f| ui::draw(f, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let width = buffer.area.width as usize;
    let cells: Vec<&str> = buffer.content().iter().map(|c| c.symbol()).collect();
    cells.chunks(width).map(|row| row.concat().trim_end().to_string()).collect::<Vec<_>>().join("\n")
}

fn key(app: &mut App, code: KeyCode) {
    app.key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn typed(app: &mut App, text: &str) {
    for c in text.chars() {
        key(app, KeyCode::Char(c));
    }
}

fn command(app: &mut App, text: &str) {
    typed(app, ":");
    typed(app, text);
    key(app, KeyCode::Enter);
    settle(app);
}

fn show(title: &str, app: &App) -> String {
    let s = screen(app);
    println!("── {title} ──\n{s}\n");
    s
}

#[test]
fn every_view_against_a_live_server() {
    let Some(server) = server() else {
        eprintln!("skipped: set LOGSURE_URL and LOGSURE_TOKEN");
        return;
    };
    let mut app = App::new(server, String::from("30d"));
    settle(&mut app);
    let first = app.me.as_ref().unwrap().apps.first().expect("the server has apps").clone();

    let s = show(":apps", &app);
    assert!(s.contains("Visitors") && s.contains(&first.name), "apps view");

    key(&mut app, KeyCode::Enter);
    settle(&mut app);
    let s = show("Enter on the first app", &app);
    assert!(s.contains("Top pages") && s.contains("Sources") && s.contains("Countries"));

    command(&mut app, "range 7d");
    let s = show(":range 7d", &app);
    assert!(s.contains("Last 7 days"));

    key(&mut app, KeyCode::Char('e'));
    settle(&mut app);
    let s = show("e: events", &app);
    assert!(s.contains("Conversion"));

    key(&mut app, KeyCode::Enter);
    settle(&mut app);
    let s = show("Enter: one event's properties", &app);
    assert!(s.contains("Property"));

    command(&mut app, "inbox all");
    let s = show(":inbox all", &app);
    assert!(s.contains("Subject"));

    typed(&mut app, "/");
    typed(&mut app, "bug");
    key(&mut app, KeyCode::Enter);
    let s = show("/bug", &app);
    assert!(s.contains("/bug"));
    key(&mut app, KeyCode::Esc);

    key(&mut app, KeyCode::Char('t'));
    settle(&mut app);
    let s = show("t: bugs only", &app);
    assert!(s.contains("Inbox › bug"));

    key(&mut app, KeyCode::Enter);
    settle(&mut app);
    let s = show("Enter: read one", &app);
    assert!(s.contains("Notes ("));

    key(&mut app, KeyCode::Char('n'));
    typed(&mut app, "Looked at this from the terminal.");
    key(&mut app, KeyCode::Enter);
    settle(&mut app);
    let s = show("n: add a note", &app);
    assert!(s.contains("Looked at this from the terminal."), "the note shows after reloading");

    key(&mut app, KeyCode::Char('r'));
    settle(&mut app);
    let s = show("r: resolve", &app);
    assert!(s.contains("resolved"));

    key(&mut app, KeyCode::Char('q'));
    settle(&mut app);
    show("q: back to the inbox", &app);

    command(&mut app, "waitlist waiting");
    let s = show(":waitlist waiting", &app);
    assert!(s.contains("Email") && s.contains("waiting"));
    key(&mut app, KeyCode::Char('i'));
    settle(&mut app);
    let s = show("i: invited", &app);
    assert!(s.contains("marked"));

    key(&mut app, KeyCode::Char('?'));
    let s = show("?: help", &app);
    assert!(s.contains("Help"));
    key(&mut app, KeyCode::Esc);

    command(&mut app, "nope");
    let s = show(":nope", &app);
    assert!(s.contains("unknown command"));

    key(&mut app, KeyCode::Char('q'));
    key(&mut app, KeyCode::Char('q'));
    assert!(app.quit, "q from the first view quits");
}
