//! The `:` commands typed in the full-screen app.

use crate::api::{SignupStatus, TicketKind};
use crate::range;

/// The `status` values `GET /tickets` takes, the server's default (`inbox`) first.
pub const TICKET_STATUSES: [&str; 8] = ["inbox", "active", "all", "open", "progress", "resolved", "spam", "archived"];

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Apps,
    App(String),
    /// Custom events of one app, or of the app last opened.
    Events(Option<String>),
    Inbox {
        kind: Option<TicketKind>,
        status: Option<String>,
    },
    Waitlist {
        list: Option<String>,
        status: Option<SignupStatus>,
    },
    Range(String),
    /// Lists the servers, or switches to one.
    Ctx(Option<String>),
    Refresh,
    Help,
    Quit,
}

pub fn parse(input: &str) -> Result<Command, String> {
    let input = input.trim().trim_start_matches(':').trim();
    let mut words = input.split_whitespace();
    let Some(name) = words.next() else {
        return Err(String::from("type a command, e.g. :apps (? for help)"));
    };
    let args: Vec<&str> = words.collect();
    let at_most = |n: usize| {
        if args.len() > n {
            Err(format!(":{name} takes at most {n} argument{}", if n == 1 { "" } else { "s" }))
        } else {
            Ok(())
        }
    };
    match name {
        "apps" => at_most(0).map(|_| Command::Apps),
        "app" => match args.as_slice() {
            [id] => Ok(Command::App(id.to_string())),
            [] => Err(String::from(":app needs an app ID, e.g. :app shop")),
            _ => Err(String::from(":app takes one app ID")),
        },
        "events" | "ev" => at_most(1).map(|_| Command::Events(args.first().map(|s| s.to_string()))),
        "inbox" => {
            at_most(2)?;
            let (mut kind, mut status) = (None, None);
            for a in &args {
                if let Some(k) = TicketKind::parse(a) {
                    kind = Some(k);
                } else if TICKET_STATUSES.contains(a) {
                    status = Some(a.to_string());
                } else {
                    return Err(format!(
                        "{a:?} is not a kind (bug, feedback, message) or a status ({})",
                        TICKET_STATUSES.join(", ")
                    ));
                }
            }
            Ok(Command::Inbox { kind, status })
        }
        "waitlist" | "wl" => {
            at_most(2)?;
            let (mut list, mut status) = (None, None);
            for a in &args {
                match SignupStatus::parse(a) {
                    Some(s) => status = Some(s),
                    None => list = Some(a.to_string()),
                }
            }
            Ok(Command::Waitlist { list, status })
        }
        "range" if args.is_empty() => Err(format!(":range needs a window: {}", range::SYNTAX)),
        // Words are joined, so `:range last month` and `:range 2026-09-01 .. 2026-09-15` work too.
        "range" => {
            let spec = args.concat();
            range::parse(&spec, range::today()).map(|_| Command::Range(spec))
        }
        "ctx" => at_most(1).map(|_| Command::Ctx(args.first().map(|s| s.to_string()))),
        "refresh" | "r" => at_most(0).map(|_| Command::Refresh),
        "help" | "h" | "?" => Ok(Command::Help),
        "q" | "quit" | "q!" => Ok(Command::Quit),
        other => Err(format!("unknown command :{other} (? for help)")),
    }
}

/// Every command, for completion and help.
pub const NAMES: [&str; 10] = ["apps", "app", "events", "inbox", "waitlist", "range", "ctx", "refresh", "help", "quit"];

/// Completes the command name being typed, when exactly one fits.
pub fn complete(input: &str) -> Option<String> {
    let typed = input.trim_start_matches(':');
    if typed.contains(' ') || typed.is_empty() {
        return None;
    }
    let mut fits = NAMES.iter().filter(|n| n.starts_with(typed));
    match (fits.next(), fits.next()) {
        (Some(one), None) => Some(format!("{one} ")),
        _ => None,
    }
}
