//! The command line: `logsure` alone opens the full-screen console; a subcommand answers once and exits.

use std::io::{self, BufRead, IsTerminal, Write};

use anyhow::{Context, Result, bail};
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use serde_json::Value;

use crate::api::{Client, SignupStatus, TicketKind, TicketQuery, WaitlistQuery, decode, encode};
use crate::command::TICKET_STATUSES;
use crate::config::{self, Config};
use crate::fmt;
use crate::range::{self, Window as Span};
use crate::secret;
use crate::tui::{self, app::Server};

#[derive(Parser)]
#[command(name = "logsure", version, about = "LogSure in the terminal: analytics, the inbox and the waitlists.")]
pub struct Cli {
    /// The server to use, by name (see `logsure ctx`); default the current one.
    #[arg(long, short = 's', global = true, env = "LOGSURE_SERVER")]
    server: Option<String>,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Sign in with the code the console shows in Settings › Mobile app (`logsure://…`), or an address and a token.
    Login {
        /// The server's address, e.g. https://logsure.example.com, or the console's code. Asked for when left out.
        url: Option<String>,
        /// The device token (`cc_…`) or the console's code. Asked for when left out, so it stays out of the shell's
        /// history.
        #[arg(long)]
        token: Option<String>,
        /// A short name for the server; default its host.
        #[arg(long)]
        name: Option<String>,
    },
    /// Forget a server and its token (default the current one).
    Logout { name: Option<String> },
    /// List the servers, or switch to one.
    Ctx { name: Option<String> },
    /// Every app's visitors, pageviews, change and visitors now.
    Apps(Window),
    /// One app's totals, top pages, sources and countries.
    App {
        id: String,
        #[command(flatten)]
        window: Window,
    },
    /// An app's custom events, or one event's properties.
    Events {
        app: String,
        /// An event's name, to list its properties.
        name: Option<String>,
        #[command(flatten)]
        window: Window,
    },
    /// Feedback, bug reports and messages.
    Inbox {
        #[arg(long, value_parser = ["bug", "feedback", "message"])]
        kind: Option<String>,
        #[arg(long, value_parser = TICKET_STATUSES)]
        status: Option<String>,
        #[arg(long)]
        app: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        #[arg(long)]
        json: bool,
    },
    /// One request in full, with its notes.
    Ticket {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Waitlist signups.
    Waitlist {
        #[arg(long)]
        list: Option<String>,
        #[arg(long, value_parser = ["waiting", "invited", "joined"])]
        status: Option<String>,
        #[arg(long)]
        app: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args)]
struct Window {
    /// The time window: 7d, 30d, 12mo, all, 14d, 3mo, lastmonth, 2026-09, 2026-09-01..2026-09-15, …
    #[arg(long, short = 'r', value_parser = parse_window)]
    range: Option<Span>,
    /// Print the server's JSON answer as it came.
    #[arg(long)]
    json: bool,
}

pub fn main() -> Result<()> {
    let help = Cli::command().before_help(crate::tui::banner::rows().join("\n"));
    let cli = Cli::from_arg_matches(&help.get_matches()).unwrap_or_else(|e| e.exit());
    let config = Config::load()?;
    let server = cli.server.as_deref();
    let default_range =
        config.range.clone().filter(|r| range::parse(r, range::today()).is_ok()).unwrap_or_else(|| String::from("7d"));
    let range = |w: &Window| {
        w.range.clone().unwrap_or_else(|| range::parse(&default_range, range::today()).unwrap_or(Span::Preset("7d")))
    };
    match cli.command {
        None => {
            if !io::stdout().is_terminal() {
                bail!("the full-screen console needs a terminal; use a subcommand (see `logsure --help`)");
            }
            tui::run(connect(&config, server)?, default_range.clone())
        }
        Some(Cmd::Login { url, token, name }) => login(config, url, token, name),
        Some(Cmd::Logout { name }) => logout(config, name.as_deref().or(server)),
        Some(Cmd::Ctx { name }) => ctx(config, name.as_deref()),
        Some(Cmd::Apps(w)) => {
            let c = connect(&config, server)?.client;
            let value = c.get_value("/overview", &range(&w).query())?;
            if w.json {
                return print_json(&value);
            }
            let names = app_names(&c)?;
            let o: crate::api::Overview = decode(value)?;
            println!("{}", o.range.label);
            let rows = o
                .apps
                .iter()
                .map(|a| {
                    vec![
                        names(&a.app_id),
                        a.visitors.to_string(),
                        a.pageviews.to_string(),
                        fmt::change(a.visitors, a.prev.map(|p| p.visitors)),
                        a.realtime.to_string(),
                    ]
                })
                .collect();
            table(&["APP", "VISITORS", "PAGEVIEWS", "CHANGE", "NOW"], &[false, true, true, true, true], rows);
            Ok(())
        }
        Some(Cmd::App { id, window }) => {
            let c = connect(&config, server)?.client;
            let value = c.get_value(&format!("/apps/{}/dashboard", encode(&id)), &range(&window).query())?;
            if window.json {
                return print_json(&value);
            }
            let d: crate::api::Dashboard = decode(value)?;
            let (t, p) = (&d.summary.totals, d.summary.prev.as_ref());
            println!("{} · {}", d.app.name, d.range.label);
            println!(
                "visitors {} {}  visits {}  pageviews {} {}  bounce {}  avg visit {}  now {}",
                t.visitors,
                fmt::change(t.visitors, p.map(|p| p.visitors)),
                t.visits,
                t.pageviews,
                fmt::change(t.pageviews, p.map(|p| p.pageviews)),
                fmt::percent(t.bounce_rate),
                fmt::duration(t.duration),
                d.live.visitors
            );
            println!("{}", fmt::spark(&d.series.iter().map(|s| s.visitors).collect::<Vec<_>>()));
            for (title, rows) in [("PAGE", &d.pages), ("SOURCE", &d.sources), ("COUNTRY", &d.countries)] {
                println!();
                let rows = rows
                    .iter()
                    .take(10)
                    .map(|r| vec![r.value.clone().unwrap_or_else(|| String::from("(none)")), r.visitors.to_string()])
                    .collect();
                table(&[title, "VISITORS"], &[false, true], rows);
            }
            Ok(())
        }
        Some(Cmd::Events { app, name: None, window }) => {
            let c = connect(&config, server)?.client;
            let value = c.get_value(&format!("/apps/{}/dashboard", encode(&app)), &range(&window).query())?;
            if window.json {
                return print_json(&value["events"]);
            }
            let d: crate::api::Dashboard = decode(value)?;
            let rows = d
                .events
                .iter()
                .map(|g| {
                    vec![
                        g.name.clone(),
                        g.visitors.to_string(),
                        g.count.to_string(),
                        fmt::percent(g.conversion),
                        g.value.map_or_else(String::new, |v| format!("{v:.2}")),
                    ]
                })
                .collect();
            table(&["EVENT", "VISITORS", "COUNT", "CONVERSION", "VALUE"], &[false, true, true, true, true], rows);
            Ok(())
        }
        Some(Cmd::Events { app, name: Some(name), window }) => {
            let c = connect(&config, server)?.client;
            let value =
                c.get_value(&format!("/apps/{}/events/{}", encode(&app), encode(&name)), &range(&window).query())?;
            if window.json {
                return print_json(&value);
            }
            let e: crate::api::EventDetail = decode(value)?;
            let rows = e
                .props
                .iter()
                .map(|p| {
                    let value = match &p.value {
                        Value::String(s) => s.clone(),
                        Value::Null => String::from("(none)"),
                        other => other.to_string(),
                    };
                    vec![p.key.clone(), value, p.visitors.to_string(), p.count.to_string()]
                })
                .collect();
            table(&["PROPERTY", "VALUE", "VISITORS", "COUNT"], &[false, false, true, true], rows);
            Ok(())
        }
        Some(Cmd::Inbox { kind, status, app, limit, json }) => {
            let c = connect(&config, server)?.client;
            let q = TicketQuery { app, kind: kind.as_deref().and_then(TicketKind::parse), status, limit };
            let value = c.get_value("/tickets", &q.pairs())?;
            if json {
                return print_json(&value);
            }
            let page: crate::api::Page<crate::api::TicketSummary> = decode(value)?;
            let now = fmt::now_ms();
            let rows = page
                .items
                .iter()
                .map(|t| {
                    vec![
                        t.id.clone(),
                        t.kind.as_str().to_string(),
                        t.status.as_str().to_string(),
                        t.app_id.clone(),
                        fmt::clip(t.subject.as_deref().filter(|s| !s.is_empty()).unwrap_or(&t.preview), 60),
                        fmt::ago(t.created_at, now),
                    ]
                })
                .collect();
            table(&["ID", "KIND", "STATUS", "APP", "SUBJECT", "AGE"], &[false, false, false, false, false, true], rows);
            println!("{} of {}", page.items.len(), page.total);
            Ok(())
        }
        Some(Cmd::Ticket { id, json }) => {
            let c = connect(&config, server)?.client;
            let value = c.get_value::<&str>(&format!("/tickets/{}", encode(&id)), &[])?;
            if json {
                return print_json(&value);
            }
            let t: crate::api::Ticket = decode(value)?;
            println!("{}", t.subject.as_deref().unwrap_or("(no subject)"));
            println!(
                "{} · {} · {} · {} · {}",
                t.kind.as_str(),
                t.status.as_str(),
                t.app_id,
                t.email.as_deref().or(t.name.as_deref()).unwrap_or("anonymous"),
                fmt::datetime(t.created_at)
            );
            println!("\n{}", t.body);
            for (k, v) in &t.data {
                println!("\n{k}: {}", v.as_str().map_or_else(|| v.to_string(), str::to_string));
            }
            for n in &t.notes {
                println!("\n{} {}: {}", fmt::datetime(n.ts), n.kind, n.body);
            }
            Ok(())
        }
        Some(Cmd::Waitlist { list, status, app, limit, json }) => {
            let c = connect(&config, server)?.client;
            let q = WaitlistQuery { app, list, status: status.as_deref().and_then(SignupStatus::parse), limit };
            let value = c.get_value("/waitlist", &q.pairs())?;
            if json {
                return print_json(&value);
            }
            let page: crate::api::WaitlistPage = decode(value)?;
            let now = fmt::now_ms();
            let rows = page
                .items
                .iter()
                .map(|s| {
                    vec![
                        s.id.to_string(),
                        s.email.clone(),
                        s.list.clone(),
                        s.status.as_str().to_string(),
                        s.source.clone().unwrap_or_default(),
                        fmt::ago(s.created_at, now),
                    ]
                })
                .collect();
            table(&["ID", "EMAIL", "LIST", "STATUS", "SOURCE", "AGE"], &[true, false, false, false, false, true], rows);
            println!("{} of {}", page.items.len(), page.total);
            Ok(())
        }
    }
}

/// The server to talk to: `LOGSURE_URL` (with `LOGSURE_TOKEN`) when set, else the named or current one from the
/// configuration, with its token from the keychain (or `LOGSURE_TOKEN`).
fn connect(config: &Config, name: Option<&str>) -> Result<Server> {
    let env_token = std::env::var("LOGSURE_TOKEN").ok().filter(|t| !t.is_empty());
    if let Some(url) = std::env::var("LOGSURE_URL").ok().filter(|u| !u.is_empty()) {
        let url = config::normalize_url(&url)?;
        let token = match env_token {
            Some(t) => t,
            None => secret::load(&url)?,
        };
        return Ok(Server { name: config::default_name(&url), client: Client::new(&url, &token) });
    }
    let (name, server) = config.server(name)?;
    let token = match env_token {
        Some(t) => t,
        None => secret::load(&server.url)?,
    };
    Ok(Server { name, client: Client::new(&server.url, &token) })
}

fn login(mut config: Config, url: Option<String>, token: Option<String>, name: Option<String>) -> Result<()> {
    let read = |prompt: &str| -> Result<String> {
        if io::stdin().is_terminal() {
            return Ok(rpassword::prompt_password(prompt)?);
        }
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        Ok(line)
    };
    // The console's code carries both the address and the token; an address given apart wins.
    let (url, token) = match (url, token) {
        (Some(url), token) if secret::parse_code(&url).is_some() => {
            let (url, code_token) = secret::parse_code(&url).unwrap_or_default();
            (url, token.unwrap_or(code_token))
        }
        (Some(url), Some(token)) => (url, token),
        (Some(url), None) => (url, read("Device token or code (cc_… or logsure://…): ")?),
        (None, token) => {
            let input = match token {
                Some(t) => t,
                None => read("Code from Settings › Mobile app (logsure://…): ")?,
            };
            match secret::parse_code(&input) {
                Some(pair) => pair,
                None => {
                    bail!("that is not the console's code (logsure://…); or run `logsure login <url>` with a token")
                }
            }
        }
    };
    let url = config::normalize_url(&url)?;
    let token = secret::parse_code(&token).map_or(token, |(_, t)| t);
    let token = token.trim().to_string();
    if !secret::looks_like_token(&token) {
        bail!("that is not a device token: they start with cc_ (make one in the console, Settings › Mobile app)");
    }
    let me = Client::new(&url, &token).me().with_context(|| format!("signing in to {url}"))?;
    secret::store(&url, &token)?;
    let name = name.unwrap_or_else(|| config::default_name(&url));
    config.add(&name, &url);
    config.save()?;
    let device = me.device.map(|d| d.name).unwrap_or_default();
    println!("Signed in to {url} as {name:?} (device {device:?}, {} apps).", me.apps.len());
    Ok(())
}

fn logout(mut config: Config, name: Option<&str>) -> Result<()> {
    let (name, server) = config.server(name)?;
    secret::forget(&server.url)?;
    config.remove(&name);
    config.save()?;
    println!("Signed out of {} and forgot {name:?}.", server.url);
    Ok(())
}

fn ctx(mut config: Config, name: Option<&str>) -> Result<()> {
    match name {
        Some(name) => {
            let (name, server) = config.server(Some(name))?;
            config.current = Some(name.clone());
            config.save()?;
            println!("Now on {name} ({}).", server.url);
        }
        None if config.servers.is_empty() => {
            println!("No servers yet: sign in with `logsure login <url> --token cc_…`.")
        }
        None => {
            let current = config.server(None).ok().map(|(n, _)| n);
            let rows = config
                .servers
                .iter()
                .map(|(n, s)| {
                    vec![if Some(n) == current.as_ref() { "*" } else { "" }.to_string(), n.clone(), s.url.clone()]
                })
                .collect();
            table(&["", "NAME", "ADDRESS"], &[false, false, false], rows);
        }
    }
    Ok(())
}

fn parse_window(text: &str) -> Result<Span, String> {
    range::parse(text, range::today())
}

/// Looks the apps' names up once, for tables that only carry IDs.
fn app_names(c: &Client) -> Result<impl Fn(&str) -> String> {
    let apps = c.me()?.apps;
    Ok(move |id: &str| apps.iter().find(|a| a.id == id).map_or_else(|| id.to_string(), |a| a.name.clone()))
}

fn print_json(value: &Value) -> Result<()> {
    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value)?;
    writeln!(out)?;
    Ok(())
}

/// Prints aligned columns separated by two spaces.
fn table(headers: &[&str], right: &[bool], rows: Vec<Vec<String>>) {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in &rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.chars().count());
        }
    }
    let line = |cells: &[String]| {
        let parts: Vec<String> = cells
            .iter()
            .zip(&widths)
            .zip(right)
            .map(|((c, w), r)| if *r { format!("{c:>w$}") } else { format!("{c:<w$}") })
            .collect();
        parts.join("  ").trim_end().to_string()
    };
    println!("{}", line(&headers.iter().map(|h| h.to_string()).collect::<Vec<_>>()));
    for row in rows {
        println!("{}", line(&row));
    }
}
