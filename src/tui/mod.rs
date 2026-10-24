//! The full-screen app.

pub mod app;
pub mod banner;
pub mod ui;

use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};

use crate::fmt;
use app::{App, Server};

/// Runs the full-screen app until the user quits.
pub fn run(server: Server, range: String) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, App::new(server, range));
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, mut app: App) -> Result<()> {
    let mut last_draw = Instant::now() - Duration::from_secs(1);
    let mut dirty = true;
    while !app.quit {
        // Redraw on changes, and once a second for the "updated … ago" clock.
        if dirty || last_draw.elapsed() >= Duration::from_secs(1) {
            terminal.draw(|f| ui::draw(f, &app))?;
            last_draw = Instant::now();
            dirty = false;
        }
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => app.key(key),
                _ => {}
            }
            dirty = true;
        }
        dirty |= app.pump(Duration::ZERO);
        app.tick(fmt::now_ms());
    }
    Ok(())
}
