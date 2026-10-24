//! The full-screen app's state: a stack of views, the input line, and the answers arriving from the server.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::api::{
    self, Client, Dashboard, EventDetail, Me, Overview, Page, SignupStatus, Ticket, TicketKind, TicketQuery,
    TicketStatus, TicketSummary, WaitlistPage, WaitlistQuery,
};
use crate::command::{self, Command, TICKET_STATUSES};
use crate::config::Config;
use crate::fmt;
use crate::range::{self, Window};
use crate::secret;

/// How often the view on screen asks the server again.
pub const REFRESH_MS: i64 = 30_000;
/// How many rows PgUp and PgDn move.
const PAGE: usize = 10;

pub struct Server {
    pub name: String,
    pub client: Client,
}

pub enum Payload {
    Overview(Overview),
    Dashboard(Box<Dashboard>),
    Event(EventDetail),
    Tickets(Page<TicketSummary>),
    Ticket(Box<Ticket>),
    Waitlist(WaitlistPage),
}

pub enum Msg {
    /// `/me`, for the server generation it was asked for.
    Me(u64, Result<Me, String>),
    Loaded {
        view: u64,
        result: Result<Payload, String>,
    },
    /// A change (status, note) finished; the text is what to tell the user.
    Done {
        view: u64,
        result: Result<String, String>,
    },
}

pub enum Kind {
    Apps(Option<Overview>),
    App { id: String, data: Option<Box<Dashboard>> },
    Events { app: String, data: Option<Box<Dashboard>> },
    Event { app: String, name: String, data: Option<EventDetail> },
    Inbox { query: TicketQuery, data: Option<Page<TicketSummary>> },
    Ticket { id: String, data: Option<Box<Ticket>> },
    Waitlist { query: WaitlistQuery, data: Option<WaitlistPage> },
    Servers(Vec<ServerRow>),
}

pub struct ServerRow {
    pub name: String,
    pub url: String,
    pub current: bool,
}

pub struct View {
    pub id: u64,
    pub kind: Kind,
    pub selected: usize,
    pub scroll: u16,
    pub filter: String,
    pub loading: bool,
    pub error: Option<String>,
    pub loaded_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Command(String),
    Filter(String),
    Note { ticket: String, text: String },
}

/// One row of a list view: the key Enter and the view's keys act on, the cells shown, and extra text `/` searches.
pub struct ListRow {
    pub key: String,
    pub cells: Vec<String>,
    pub search: String,
}

pub struct App {
    pub server: Server,
    pub me: Option<Me>,
    pub range: String,
    pub stack: Vec<View>,
    pub mode: Mode,
    pub help: bool,
    /// A message for the bottom line, and whether it is an error.
    pub status: Option<(String, bool)>,
    pub quit: bool,
    next_id: u64,
    generation: u64,
    last_app: Option<String>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl App {
    pub fn new(server: Server, range: String) -> App {
        let (tx, rx) = channel();
        let mut app = App {
            server,
            me: None,
            range,
            stack: Vec::new(),
            mode: Mode::Normal,
            help: false,
            status: None,
            quit: false,
            next_id: 0,
            generation: 0,
            last_app: None,
            tx,
            rx,
        };
        app.load_me();
        app.push(Kind::Apps(None));
        app
    }

    /// The window, worked out for today: `14d` moves along as days pass.
    pub fn window(&self) -> Window {
        range::parse(&self.range, range::today()).unwrap_or(Window::Preset("7d"))
    }

    pub fn top(&self) -> &View {
        self.stack.last().expect("there is always a view")
    }

    fn top_mut(&mut self) -> &mut View {
        self.stack.last_mut().expect("there is always a view")
    }

    pub fn app_name(&self, id: &str) -> String {
        self.me
            .as_ref()
            .and_then(|me| me.apps.iter().find(|a| a.id == id))
            .map_or_else(|| id.to_string(), |a| a.name.clone())
    }

    fn say(&mut self, text: impl Into<String>) {
        self.status = Some((text.into(), false));
    }

    fn fail(&mut self, text: impl Into<String>) {
        self.status = Some((text.into(), true));
    }

    // --- Talking to the server -------------------------------------------------------------------------------

    fn spawn(&self, job: impl FnOnce(&Client) -> Msg + Send + 'static) {
        let client = self.server.client.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let _ = tx.send(job(&client));
        });
    }

    fn load_me(&mut self) {
        self.generation += 1;
        let generation = self.generation;
        self.spawn(move |c| Msg::Me(generation, c.me().map_err(|e| e.to_string())));
    }

    /// Asks the server again for the view on top.
    pub fn load(&mut self) {
        let range = self.window();
        let view = self.top_mut();
        let id = view.id;
        type Job = Box<dyn FnOnce(&Client) -> api::Result<Payload> + Send>;
        let job: Job = match &view.kind {
            Kind::Apps(_) => Box::new(move |c| c.overview(&range).map(Payload::Overview)),
            Kind::App { id: app, .. } | Kind::Events { app, .. } => {
                let app = app.clone();
                Box::new(move |c| c.dashboard(&app, &range).map(|d| Payload::Dashboard(Box::new(d))))
            }
            Kind::Event { app, name, .. } => {
                let (app, name) = (app.clone(), name.clone());
                Box::new(move |c| c.event(&app, &name, &range).map(Payload::Event))
            }
            Kind::Inbox { query, .. } => {
                let query = query.clone();
                Box::new(move |c| c.tickets(&query).map(Payload::Tickets))
            }
            Kind::Ticket { id: ticket, .. } => {
                let ticket = ticket.clone();
                Box::new(move |c| c.ticket(&ticket).map(|t| Payload::Ticket(Box::new(t))))
            }
            Kind::Waitlist { query, .. } => {
                let query = query.clone();
                Box::new(move |c| c.waitlist(&query).map(Payload::Waitlist))
            }
            Kind::Servers(_) => return,
        };
        view.loading = true;
        self.spawn(move |c| Msg::Loaded { view: id, result: job(c).map_err(|e| e.to_string()) });
    }

    /// Takes in every answer that has arrived, waiting up to `wait` for the first. Answers whether anything came.
    pub fn pump(&mut self, wait: Duration) -> bool {
        let mut got = false;
        let first = if wait.is_zero() { self.rx.try_recv().ok() } else { self.rx.recv_timeout(wait).ok() };
        let mut next = first;
        while let Some(msg) = next {
            self.apply(msg);
            got = true;
            next = self.rx.try_recv().ok();
        }
        got
    }

    fn apply(&mut self, msg: Msg) {
        match msg {
            Msg::Me(generation, result) if generation == self.generation => match result {
                Ok(me) => self.me = Some(me),
                Err(e) => self.fail(e),
            },
            Msg::Me(..) => {}
            Msg::Loaded { view, result } => {
                let Some(v) = self.stack.iter_mut().find(|v| v.id == view) else { return };
                v.loading = false;
                v.loaded_at = Some(fmt::now_ms());
                match result {
                    Ok(payload) => {
                        v.error = None;
                        match (&mut v.kind, payload) {
                            (Kind::Apps(data), Payload::Overview(o)) => *data = Some(o),
                            (Kind::App { data, .. }, Payload::Dashboard(d))
                            | (Kind::Events { data, .. }, Payload::Dashboard(d)) => *data = Some(d),
                            (Kind::Event { data, .. }, Payload::Event(e)) => *data = Some(e),
                            (Kind::Inbox { data, .. }, Payload::Tickets(p)) => *data = Some(p),
                            (Kind::Ticket { data, .. }, Payload::Ticket(t)) => *data = Some(t),
                            (Kind::Waitlist { data, .. }, Payload::Waitlist(w)) => *data = Some(w),
                            _ => {}
                        }
                    }
                    Err(e) => v.error = Some(e),
                }
            }
            Msg::Done { view, result } => {
                match result {
                    Ok(text) => self.say(text),
                    Err(e) => self.fail(e),
                }
                if self.top().id == view {
                    self.load();
                }
            }
        }
    }

    /// Refreshes the view on top when its data is older than [`REFRESH_MS`].
    pub fn tick(&mut self, now: i64) {
        let v = self.top();
        if !v.loading && v.loaded_at.is_some_and(|at| now - at >= REFRESH_MS) {
            self.load();
        }
    }

    // --- Views ----------------------------------------------------------------------------------------------

    fn push(&mut self, kind: Kind) {
        self.next_id += 1;
        self.stack.push(View {
            id: self.next_id,
            kind,
            selected: 0,
            scroll: 0,
            filter: String::new(),
            loading: false,
            error: None,
            loaded_at: None,
        });
        self.load();
    }

    /// Opens a view from a command. A view of the same kind already on the stack is brought back instead of
    /// stacking another, so `q` always leads somewhere new.
    fn open(&mut self, kind: Kind) {
        let same = |a: &Kind, b: &Kind| std::mem::discriminant(a) == std::mem::discriminant(b);
        if let Some(i) = self.stack.iter().position(|v| same(&v.kind, &kind)) {
            self.stack.truncate(i);
        }
        if self.stack.is_empty() && !matches!(kind, Kind::Apps(_)) {
            self.push(Kind::Apps(None));
        }
        self.push(kind);
    }

    fn back(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
            self.load();
        } else {
            self.quit = true;
        }
    }

    fn current_app(&self) -> Option<String> {
        self.stack
            .iter()
            .rev()
            .find_map(|v| match &v.kind {
                Kind::App { id, .. } | Kind::Events { app: id, .. } | Kind::Event { app: id, .. } => Some(id.clone()),
                _ => None,
            })
            .or_else(|| self.last_app.clone())
            .or_else(|| self.me.as_ref().and_then(|m| m.apps.first()).map(|a| a.id.clone()))
    }

    pub fn run(&mut self, cmd: Command) {
        match cmd {
            Command::Apps => self.open(Kind::Apps(None)),
            Command::App(id) => {
                if let Some(me) = &self.me
                    && !me.apps.iter().any(|a| a.id == id)
                {
                    let ids: Vec<&str> = me.apps.iter().map(|a| a.id.as_str()).collect();
                    return self.fail(format!("no app {id:?}; the apps are {}", ids.join(", ")));
                }
                self.last_app = Some(id.clone());
                self.open(Kind::App { id, data: None });
            }
            Command::Events(app) => match app.or_else(|| self.current_app()) {
                Some(app) => {
                    self.last_app = Some(app.clone());
                    self.open(Kind::Events { app, data: None });
                }
                None => self.fail(":events needs an app, e.g. :events shop"),
            },
            Command::Inbox { kind, status } => {
                self.open(Kind::Inbox { query: TicketQuery { kind, status, limit: 500, app: None }, data: None })
            }
            Command::Waitlist { list, status } => {
                self.open(Kind::Waitlist { query: WaitlistQuery { list, status, limit: 500, app: None }, data: None })
            }
            Command::Range(key) => {
                self.range = key;
                self.say(format!("range: {}", self.window().label()));
                self.load();
            }
            Command::Ctx(None) => match Config::load() {
                Ok(config) => {
                    let rows = config
                        .servers
                        .iter()
                        .map(|(name, s)| ServerRow {
                            name: name.clone(),
                            url: s.url.clone(),
                            current: *name == self.server.name,
                        })
                        .collect();
                    self.open(Kind::Servers(rows));
                }
                Err(e) => self.fail(e.to_string()),
            },
            Command::Ctx(Some(name)) => self.switch(&name),
            Command::Refresh => self.load(),
            Command::Help => self.help = true,
            Command::Quit => self.quit = true,
        }
    }

    /// Switches to another server from the configuration and makes it the current one.
    fn switch(&mut self, name: &str) {
        let result = (|| {
            let mut config = Config::load()?;
            let (name, server) = config.server(Some(name))?;
            let token = secret::load(&server.url)?;
            config.current = Some(name.clone());
            config.save()?;
            anyhow::Ok(Server { name, client: Client::new(&server.url, &token) })
        })();
        match result {
            Ok(server) => {
                self.say(format!("now on {} ({})", server.name, server.client.base()));
                self.server = server;
                self.me = None;
                self.last_app = None;
                self.stack.clear();
                self.load_me();
                self.push(Kind::Apps(None));
            }
            Err(e) => self.fail(e.to_string()),
        }
    }

    // --- Lists ----------------------------------------------------------------------------------------------

    /// The rows of the view on top after the filter, or `None` when it is not a list.
    pub fn rows(&self) -> Option<Vec<ListRow>> {
        let v = self.top();
        let rows = self.all_rows(v)?;
        let needle = v.filter.to_lowercase();
        Some(
            rows.into_iter()
                .filter(|r| {
                    needle.is_empty()
                        || r.cells.iter().any(|c| c.to_lowercase().contains(&needle))
                        || r.search.to_lowercase().contains(&needle)
                })
                .collect(),
        )
    }

    fn all_rows(&self, v: &View) -> Option<Vec<ListRow>> {
        let now = fmt::now_ms();
        let (since, until) = self.window().bounds(now);
        let within = |at: i64| at >= since && at < until;
        let row = |key: String, cells: Vec<String>, search: String| ListRow { key, cells, search };
        Some(match &v.kind {
            Kind::Apps(data) => data
                .iter()
                .flat_map(|o| &o.apps)
                .map(|a| {
                    row(
                        a.app_id.clone(),
                        vec![
                            self.app_name(&a.app_id),
                            fmt::count(a.visitors),
                            fmt::count(a.pageviews),
                            fmt::change(a.visitors, a.prev.map(|p| p.visitors)),
                            a.realtime.to_string(),
                            fmt::spark(&a.series),
                        ],
                        a.app_id.clone(),
                    )
                })
                .collect(),
            Kind::Events { data, .. } => data
                .iter()
                .flat_map(|d| &d.events)
                .map(|g| {
                    row(
                        g.name.clone(),
                        vec![
                            g.name.clone(),
                            fmt::count(g.visitors),
                            fmt::count(g.count),
                            fmt::percent(g.conversion),
                            g.value.map_or_else(String::new, |v| format!("{v:.2}")),
                        ],
                        String::new(),
                    )
                })
                .collect(),
            Kind::Event { data, .. } => data
                .iter()
                .flat_map(|e| &e.props)
                .map(|p| {
                    let value = match &p.value {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Null => String::from("(none)"),
                        other => other.to_string(),
                    };
                    row(
                        format!("{}={value}", p.key),
                        vec![p.key.clone(), value, fmt::count(p.visitors), fmt::count(p.count)],
                        String::new(),
                    )
                })
                .collect(),
            Kind::Inbox { data, .. } => data
                .iter()
                .flat_map(|p| &p.items)
                .filter(|t| within(t.created_at))
                .map(|t| {
                    let subject = t.subject.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| t.preview.clone());
                    let from = t.name.clone().or_else(|| t.email.clone()).unwrap_or_default();
                    row(
                        t.id.clone(),
                        vec![
                            t.kind.as_str().to_string(),
                            t.status.as_str().to_string(),
                            self.app_name(&t.app_id),
                            subject,
                            from,
                            fmt::ago(t.created_at, now),
                        ],
                        format!(
                            "{} {} {}",
                            t.preview,
                            t.email.as_deref().unwrap_or(""),
                            t.category.as_deref().unwrap_or("")
                        ),
                    )
                })
                .collect(),
            Kind::Waitlist { data, .. } => data
                .iter()
                .flat_map(|w| &w.items)
                .filter(|s| within(s.created_at))
                .map(|s| {
                    row(
                        s.id.to_string(),
                        vec![
                            s.email.clone(),
                            s.name.clone().unwrap_or_default(),
                            s.list.clone(),
                            s.status.as_str().to_string(),
                            s.source.clone().unwrap_or_default(),
                            s.country.clone().unwrap_or_default(),
                            fmt::ago(s.created_at, now),
                        ],
                        self.app_name(&s.app_id),
                    )
                })
                .collect(),
            Kind::Servers(rows) => rows
                .iter()
                .map(|s| {
                    row(
                        s.name.clone(),
                        vec![if s.current { "*" } else { "" }.to_string(), s.name.clone(), s.url.clone()],
                        String::new(),
                    )
                })
                .collect(),
            Kind::App { .. } | Kind::Ticket { .. } => return None,
        })
    }

    fn selected_key(&self) -> Option<String> {
        let rows = self.rows()?;
        let i = self.top().selected.min(rows.len().checked_sub(1)?);
        Some(rows[i].key.clone())
    }

    fn move_by(&mut self, delta: isize) {
        let len = self.rows().map(|r| r.len());
        let v = self.top_mut();
        match len {
            Some(len) => {
                let max = len.saturating_sub(1) as isize;
                v.selected = (v.selected.min(max as usize) as isize + delta).clamp(0, max) as usize;
            }
            None => v.scroll = (v.scroll as isize + delta).clamp(0, u16::MAX as isize) as u16,
        }
    }

    // --- Keys -----------------------------------------------------------------------------------------------

    pub fn key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => self.quit = true,
                KeyCode::Char('r') => self.load(),
                _ => {}
            }
            return;
        }
        if self.help {
            self.help = false;
            return;
        }
        match self.mode.clone() {
            Mode::Normal => self.normal_key(key.code),
            Mode::Command(text) => self.command_key(key.code, text),
            Mode::Filter(text) => self.filter_key(key.code, text),
            Mode::Note { ticket, text } => self.note_key(key.code, ticket, text),
        }
    }

    fn normal_key(&mut self, code: KeyCode) {
        self.status = None;
        match code {
            KeyCode::Char(':') => self.mode = Mode::Command(String::new()),
            KeyCode::Char('/') => self.mode = Mode::Filter(self.top().filter.clone()),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('q') => self.back(),
            // Esc first clears a filter, then goes back like q, but never quits.
            KeyCode::Esc => {
                let v = self.top_mut();
                if !v.filter.is_empty() {
                    v.filter.clear();
                    v.selected = 0;
                } else if self.stack.len() > 1 {
                    self.back();
                }
            }
            KeyCode::Up => self.move_by(-1),
            KeyCode::Down => self.move_by(1),
            KeyCode::PageUp => self.move_by(-(PAGE as isize)),
            KeyCode::PageDown => self.move_by(PAGE as isize),
            KeyCode::Home => self.move_by(isize::MIN / 2),
            KeyCode::End => self.move_by(isize::MAX / 2),
            KeyCode::Enter => self.enter(),
            KeyCode::Char(c) => self.view_key(c),
            _ => {}
        }
    }

    fn enter(&mut self) {
        let Some(key) = self.selected_key() else { return };
        match &self.top().kind {
            Kind::Apps(_) => self.run(Command::App(key)),
            Kind::Events { app, .. } => {
                let app = app.clone();
                self.push(Kind::Event { app, name: key, data: None });
            }
            Kind::Inbox { .. } => self.push(Kind::Ticket { id: key, data: None }),
            Kind::Servers(_) => self.switch(&key),
            _ => {}
        }
    }

    /// The keys that belong to one view.
    fn view_key(&mut self, c: char) {
        let selected = self.selected_key();
        match (&mut self.top_mut().kind, c) {
            (Kind::App { id, .. }, 'e') => {
                let app = id.clone();
                self.run(Command::Events(Some(app)));
            }
            (Kind::Inbox { query, .. }, 't') => {
                query.kind = match query.kind {
                    None => Some(TicketKind::Bug),
                    Some(TicketKind::Bug) => Some(TicketKind::Feedback),
                    Some(TicketKind::Feedback) => Some(TicketKind::Message),
                    Some(TicketKind::Message) => None,
                };
                self.reset_and_load();
            }
            (Kind::Inbox { query, .. }, 's') => {
                let at = TICKET_STATUSES.iter().position(|s| Some(*s) == query.status.as_deref()).unwrap_or(0);
                query.status = Some(TICKET_STATUSES[(at + 1) % TICKET_STATUSES.len()].to_string());
                self.reset_and_load();
            }
            (Kind::Inbox { .. }, 'r') | (Kind::Ticket { .. }, 'r') => {
                if let Some(id) = self.ticket_id(selected) {
                    self.resolve(id);
                }
            }
            (Kind::Inbox { .. }, 'n') | (Kind::Ticket { .. }, 'n') => {
                if let Some(ticket) = self.ticket_id(selected) {
                    self.mode = Mode::Note { ticket, text: String::new() };
                }
            }
            (Kind::Waitlist { query, data }, 'l') => {
                let lists: Vec<String> = data.iter().flat_map(|d| &d.lists).map(|l| l.list.clone()).collect();
                let at = lists.iter().position(|l| Some(l) == query.list.as_ref());
                query.list = match at {
                    None => lists.first().cloned(),
                    Some(i) => lists.get(i + 1).cloned(),
                };
                self.reset_and_load();
            }
            (Kind::Waitlist { query, .. }, 's') => {
                query.status = match query.status {
                    None => Some(SignupStatus::Waiting),
                    Some(SignupStatus::Waiting) => Some(SignupStatus::Invited),
                    Some(SignupStatus::Invited) => Some(SignupStatus::Joined),
                    Some(SignupStatus::Joined) => None,
                };
                self.reset_and_load();
            }
            (Kind::Waitlist { .. }, c @ ('i' | 'j' | 'w')) => {
                let status = match c {
                    'i' => SignupStatus::Invited,
                    'j' => SignupStatus::Joined,
                    _ => SignupStatus::Waiting,
                };
                if let Some(id) = selected.and_then(|k| k.parse::<u64>().ok()) {
                    self.set_signup(id, status);
                }
            }
            _ => {}
        }
    }

    fn reset_and_load(&mut self) {
        let v = self.top_mut();
        v.selected = 0;
        v.data_clear();
        self.load();
    }

    fn ticket_id(&self, selected: Option<String>) -> Option<String> {
        match &self.top().kind {
            Kind::Ticket { id, .. } => Some(id.clone()),
            _ => selected,
        }
    }

    fn resolve(&mut self, id: String) {
        let view = self.top().id;
        self.spawn(move |c| Msg::Done {
            view,
            result: c
                .set_ticket_status(&id, TicketStatus::Resolved)
                .map(|_| format!("resolved {id}"))
                .map_err(|e| e.to_string()),
        });
    }

    fn set_signup(&mut self, id: u64, status: SignupStatus) {
        let view = self.top().id;
        self.spawn(move |c| Msg::Done {
            view,
            result: c
                .set_signup_status(id, status)
                .map(|_| format!("marked {id} {}", status.as_str()))
                .map_err(|e| e.to_string()),
        });
    }

    fn command_key(&mut self, code: KeyCode, mut text: String) {
        match code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                match command::parse(&text) {
                    Ok(cmd) => self.run(cmd),
                    Err(e) => self.fail(e),
                }
            }
            KeyCode::Tab => {
                if let Some(done) = command::complete(&text) {
                    self.mode = Mode::Command(done);
                }
            }
            KeyCode::Backspace if text.is_empty() => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                text.pop();
                self.mode = Mode::Command(text);
            }
            KeyCode::Char(c) => {
                text.push(c);
                self.mode = Mode::Command(text);
            }
            _ => {}
        }
    }

    fn filter_key(&mut self, code: KeyCode, mut text: String) {
        match code {
            KeyCode::Esc => {
                text.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                text.pop();
                self.mode = Mode::Filter(text.clone());
            }
            KeyCode::Char(c) => {
                text.push(c);
                self.mode = Mode::Filter(text.clone());
            }
            _ => return,
        }
        let v = self.top_mut();
        v.filter = text;
        v.selected = 0;
    }

    fn note_key(&mut self, code: KeyCode, ticket: String, mut text: String) {
        match code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter if text.trim().is_empty() => self.mode = Mode::Normal,
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                let view = self.top().id;
                let body = text.trim().to_string();
                self.spawn(move |c| Msg::Done {
                    view,
                    result: c.add_note(&ticket, &body).map(|_| String::from("note added")).map_err(|e| e.to_string()),
                });
            }
            KeyCode::Backspace => {
                text.pop();
                self.mode = Mode::Note { ticket, text };
            }
            KeyCode::Char(c) => {
                text.push(c);
                self.mode = Mode::Note { ticket, text };
            }
            _ => {}
        }
    }
}

impl View {
    /// Drops the data when the filters changed, so the view does not show answers to the old ones.
    fn data_clear(&mut self) {
        match &mut self.kind {
            Kind::Inbox { data, .. } => *data = None,
            Kind::Waitlist { data, .. } => *data = None,
            _ => {}
        }
    }

    pub fn title(&self, app: &App) -> String {
        match &self.kind {
            Kind::Apps(_) => String::from("Apps"),
            Kind::App { id, .. } => app.app_name(id),
            Kind::Events { app: id, .. } => format!("{} › Events", app.app_name(id)),
            Kind::Event { app: id, name, .. } => format!("{} › Events › {name}", app.app_name(id)),
            Kind::Inbox { query, .. } => {
                let kind = query.kind.map_or("all kinds", TicketKind::as_str);
                format!("Inbox › {kind} › {}", query.status.as_deref().unwrap_or("inbox"))
            }
            Kind::Ticket { id, .. } => format!("Inbox › {id}"),
            Kind::Waitlist { query, .. } => {
                let list = query.list.as_deref().unwrap_or("all lists");
                format!("Waitlist › {list} › {}", query.status.map_or("every status", SignupStatus::as_str))
            }
            Kind::Servers(_) => String::from("Servers"),
        }
    }

    /// Whether the time window applies to this view.
    pub fn uses_range(&self) -> bool {
        !matches!(self.kind, Kind::Ticket { .. } | Kind::Servers(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An app whose server is never reached: answers fail and are ignored by these tests.
    fn app() -> App {
        App::new(
            Server { name: String::from("test"), client: Client::new("http://127.0.0.1:9", "cc_test") },
            String::from("7d"),
        )
    }

    fn press(app: &mut App, code: KeyCode) {
        app.key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn depth(app: &App) -> usize {
        app.stack.len()
    }

    #[test]
    fn esc_goes_back_like_q() {
        let mut app = app();
        app.push(Kind::Inbox { query: TicketQuery::default(), data: None });
        app.push(Kind::Ticket { id: String::from("t_1"), data: None });
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.top().kind, Kind::Inbox { .. }), "from a request back to its list");
        press(&mut app, KeyCode::Esc);
        assert!(matches!(app.top().kind, Kind::Apps(_)));
        assert!(!app.quit);
    }

    #[test]
    fn esc_never_quits_from_the_first_view() {
        let mut app = app();
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Esc);
        assert_eq!(depth(&app), 1);
        assert!(!app.quit, "Esc on the first view does nothing");
        press(&mut app, KeyCode::Char('q'));
        assert!(app.quit, "q still quits from there");
    }

    #[test]
    fn esc_closes_help_first() {
        let mut app = app();
        app.push(Kind::Inbox { query: TicketQuery::default(), data: None });
        press(&mut app, KeyCode::Char('?'));
        assert!(app.help);
        press(&mut app, KeyCode::Esc);
        assert!(!app.help);
        assert_eq!(depth(&app), 2, "closing help does not also go back");
    }

    #[test]
    fn esc_closes_the_command_line_first() {
        let mut app = app();
        app.push(Kind::Inbox { query: TicketQuery::default(), data: None });
        press(&mut app, KeyCode::Char(':'));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(depth(&app), 2);
    }

    #[test]
    fn esc_closes_a_filter_then_goes_back() {
        let mut app = app();
        app.push(Kind::Inbox { query: TicketQuery::default(), data: None });
        // Typing a filter: Esc drops it and stays.
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.top().filter.is_empty());
        assert_eq!(depth(&app), 2);
        // A kept filter: the first Esc clears it, the second goes back.
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.top().filter, "x");
        press(&mut app, KeyCode::Esc);
        assert!(app.top().filter.is_empty());
        assert_eq!(depth(&app), 2);
        press(&mut app, KeyCode::Esc);
        assert_eq!(depth(&app), 1);
    }

    #[test]
    fn esc_closes_the_note_box_first() {
        let mut app = app();
        app.push(Kind::Inbox { query: TicketQuery::default(), data: None });
        app.push(Kind::Ticket { id: String::from("t_1"), data: None });
        press(&mut app, KeyCode::Char('n'));
        assert!(matches!(app.mode, Mode::Note { .. }));
        press(&mut app, KeyCode::Char('h'));
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, Mode::Normal);
        assert!(matches!(app.top().kind, Kind::Ticket { .. }), "the request stays open");
    }
}
