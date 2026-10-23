//! A small client for the parts of the console API (`/api/v1`) this app uses.
//!
//! The types follow the schemas in the LogSure contract (`api/openapi.yaml` in logsure/server). Fields the app does
//! not show are left out; unknown fields are ignored, as the contract asks of clients.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Milliseconds since the Unix epoch.
pub type Time = i64;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Range {
    pub key: String,
    pub label: String,
    pub from: Time,
    pub to: Time,
    pub bucket: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct App {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatusCounts {
    pub open: u64,
    pub progress: u64,
    pub resolved: u64,
    pub spam: u64,
    pub archived: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketCounts {
    #[serde(flatten)]
    pub all: StatusCounts,
    #[serde(default)]
    pub by_kind: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Me {
    pub device: Option<DeviceRef>,
    pub user: Option<String>,
    pub name: Option<String>,
    pub apps: Vec<App>,
    pub counts: Option<TicketCounts>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct VisitorsPageviews {
    pub visitors: u64,
    pub pageviews: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewApp {
    pub app_id: String,
    pub visitors: u64,
    pub pageviews: u64,
    pub prev: Option<VisitorsPageviews>,
    pub spark: Vec<u64>,
    pub realtime: u64,
    pub series: Vec<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Overview {
    pub range: Range,
    pub apps: Vec<OverviewApp>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Row {
    pub value: Option<String>,
    pub visitors: u64,
    pub count: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub visitors: u64,
    pub visits: u64,
    pub pageviews: u64,
    pub events: u64,
    pub views_per_visit: Option<f64>,
    pub bounce_rate: Option<f64>,
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Summary {
    #[serde(flatten)]
    pub totals: Totals,
    pub prev: Option<Totals>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SeriesPoint {
    pub t: Time,
    pub visitors: u64,
    pub pageviews: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Goal {
    pub name: String,
    pub visitors: u64,
    pub count: u64,
    pub value: Option<f64>,
    pub conversion: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Live {
    pub visitors: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Dashboard {
    pub app: App,
    pub range: Range,
    pub summary: Summary,
    pub series: Vec<SeriesPoint>,
    pub pages: Vec<Row>,
    pub sources: Vec<Row>,
    pub countries: Vec<Row>,
    pub live: Live,
    pub events: Vec<Goal>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventPoint {
    pub t: Time,
    pub count: u64,
    pub visitors: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventProp {
    pub key: String,
    /// A string, number, boolean or null.
    pub value: Value,
    pub visitors: u64,
    pub count: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventDetail {
    pub name: String,
    pub range: Range,
    pub series: Vec<EventPoint>,
    pub props: Vec<EventProp>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TicketKind {
    Bug,
    Feedback,
    Message,
}

impl TicketKind {
    pub const ALL: [TicketKind; 3] = [TicketKind::Bug, TicketKind::Feedback, TicketKind::Message];

    pub fn as_str(self) -> &'static str {
        match self {
            TicketKind::Bug => "bug",
            TicketKind::Feedback => "feedback",
            TicketKind::Message => "message",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TicketStatus {
    Open,
    Progress,
    Resolved,
    Spam,
    Archived,
}

impl TicketStatus {
    pub const ALL: [TicketStatus; 5] = [
        TicketStatus::Open,
        TicketStatus::Progress,
        TicketStatus::Resolved,
        TicketStatus::Spam,
        TicketStatus::Archived,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            TicketStatus::Open => "open",
            TicketStatus::Progress => "progress",
            TicketStatus::Resolved => "resolved",
            TicketStatus::Spam => "spam",
            TicketStatus::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketSummary {
    pub id: String,
    pub app_id: String,
    pub kind: TicketKind,
    pub status: TicketStatus,
    pub subject: Option<String>,
    pub preview: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub rating: Option<u8>,
    pub category: Option<String>,
    pub created_at: Time,
    pub updated_at: Option<Time>,
    #[serde(default)]
    pub files: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Page<T> {
    pub total: u64,
    pub items: Vec<T>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TicketMeta {
    pub url: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub device: Option<String>,
    pub country: Option<String>,
    pub lang: Option<String>,
    pub host: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Note {
    /// An integer or a string, depending on the server.
    pub id: Value,
    pub ts: Time,
    pub kind: String,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub mime: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticket {
    pub id: String,
    pub app_id: String,
    pub kind: TicketKind,
    pub status: TicketStatus,
    pub subject: Option<String>,
    pub body: String,
    #[serde(default)]
    pub data: serde_json::Map<String, Value>,
    #[serde(default)]
    pub meta: TicketMeta,
    pub email: Option<String>,
    pub name: Option<String>,
    pub rating: Option<u8>,
    pub category: Option<String>,
    pub created_at: Time,
    pub updated_at: Option<Time>,
    pub notes: Vec<Note>,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SignupStatus {
    Waiting,
    Invited,
    Joined,
}

impl SignupStatus {
    pub const ALL: [SignupStatus; 3] = [SignupStatus::Waiting, SignupStatus::Invited, SignupStatus::Joined];

    pub fn as_str(self) -> &'static str {
        match self {
            SignupStatus::Waiting => "waiting",
            SignupStatus::Invited => "invited",
            SignupStatus::Joined => "joined",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Signup {
    pub id: u64,
    pub app_id: String,
    pub list: String,
    pub email: String,
    pub name: Option<String>,
    pub status: SignupStatus,
    pub source: Option<String>,
    pub country: Option<String>,
    pub lang: Option<String>,
    pub created_at: Time,
    pub updated_at: Option<Time>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCounts {
    pub app_id: String,
    pub list: String,
    pub total: u64,
    pub waiting: u64,
    pub invited: u64,
    pub joined: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WaitlistPage {
    pub total: u64,
    pub items: Vec<Signup>,
    pub lists: Vec<ListCounts>,
}

/// An error answer: `{ error, code, field? }`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ApiError {
    pub error: String,
    pub code: String,
    #[serde(default)]
    pub field: Option<String>,
}

#[derive(Debug)]
pub enum Error {
    /// The server answered with an error status.
    Api { status: u16, error: ApiError },
    /// The server could not be reached.
    Transport(String),
    /// The answer was not what the contract describes.
    Decode(String),
}

impl Error {
    pub fn is_unauthorized(&self) -> bool {
        matches!(self, Error::Api { status: 401, .. })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Api { error, .. } => write!(f, "{}", error.error),
            Error::Transport(e) => write!(f, "could not reach the server: {e}"),
            Error::Decode(e) => write!(f, "unexpected answer from the server: {e}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Turns an error answer's body into an [`Error`], falling back to the status when the body is not the contract's.
pub fn parse_error(status: u16, body: &str) -> Error {
    let error = serde_json::from_str::<ApiError>(body).unwrap_or_else(|_| ApiError {
        error: format!("the server answered {status}"),
        code: String::from("unknown"),
        field: None,
    });
    Error::Api { status, error }
}

/// Parses an answer's JSON into one of the types above.
pub fn decode<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| Error::Decode(e.to_string()))
}

#[derive(Clone)]
pub struct Client {
    base: String,
    token: String,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(base: &str, token: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("logsure-terminal/", env!("CARGO_PKG_VERSION")))
            .build();
        Client { base: base.trim_end_matches('/').to_string(), token: token.to_string(), agent }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v1{}", self.base, path)
    }

    fn send(&self, req: ureq::Request, body: Option<Value>) -> Result<Value> {
        let req = req.set("Authorization", &format!("Bearer {}", self.token)).set("Accept", "application/json");
        let res = match body {
            Some(body) => req.send_json(body),
            None => req.call(),
        };
        match res {
            Ok(res) => res.into_json::<Value>().map_err(|e| Error::Decode(e.to_string())),
            Err(ureq::Error::Status(status, res)) => Err(parse_error(status, &res.into_string().unwrap_or_default())),
            Err(ureq::Error::Transport(e)) => Err(Error::Transport(e.to_string())),
        }
    }

    /// GETs a path under `/api/v1`, leaving out empty query values, and answers the JSON as it came.
    pub fn get_value<V: AsRef<str>>(&self, path: &str, query: &[(&str, V)]) -> Result<Value> {
        let mut req = self.agent.get(&self.url(path));
        for (k, v) in query.iter().filter(|(_, v)| !v.as_ref().is_empty()) {
            req = req.query(k, v.as_ref());
        }
        self.send(req, None)
    }

    fn post(&self, path: &str, body: Value) -> Result<()> {
        self.send(self.agent.post(&self.url(path)), Some(body)).map(|_| ())
    }

    pub fn me(&self) -> Result<Me> {
        decode(self.get_value::<&str>("/me", &[])?)
    }

    pub fn overview(&self, range: &str) -> Result<Overview> {
        decode(self.get_value("/overview", &[("range", range)])?)
    }

    pub fn dashboard(&self, app: &str, range: &str) -> Result<Dashboard> {
        decode(self.get_value(&format!("/apps/{}/dashboard", encode(app)), &[("range", range)])?)
    }

    pub fn event(&self, app: &str, name: &str, range: &str) -> Result<EventDetail> {
        decode(self.get_value(&format!("/apps/{}/events/{}", encode(app), encode(name)), &[("range", range)])?)
    }

    pub fn tickets(&self, q: &TicketQuery) -> Result<Page<TicketSummary>> {
        decode(self.get_value("/tickets", &q.pairs())?)
    }

    pub fn ticket(&self, id: &str) -> Result<Ticket> {
        decode(self.get_value::<&str>(&format!("/tickets/{}", encode(id)), &[])?)
    }

    pub fn set_ticket_status(&self, id: &str, status: TicketStatus) -> Result<()> {
        self.post(&format!("/tickets/{}/status", encode(id)), serde_json::json!({ "status": status }))
    }

    pub fn add_note(&self, id: &str, body: &str) -> Result<()> {
        self.post(&format!("/tickets/{}/notes", encode(id)), serde_json::json!({ "body": body }))
    }

    pub fn waitlist(&self, q: &WaitlistQuery) -> Result<WaitlistPage> {
        decode(self.get_value("/waitlist", &q.pairs())?)
    }

    pub fn set_signup_status(&self, id: u64, status: SignupStatus) -> Result<()> {
        self.post(&format!("/waitlist/{id}/status"), serde_json::json!({ "status": status }))
    }
}

/// The filters of `GET /tickets`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TicketQuery {
    pub app: Option<String>,
    pub kind: Option<TicketKind>,
    /// A status, or `active`, `inbox` (the server's default) or `all`.
    pub status: Option<String>,
    pub limit: u32,
}

impl TicketQuery {
    pub fn pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("app", self.app.clone().unwrap_or_default()),
            ("kind", self.kind.map(TicketKind::as_str).unwrap_or_default().to_string()),
            ("status", self.status.clone().unwrap_or_default()),
            ("limit", limit_str(self.limit)),
        ]
    }
}

/// The filters of `GET /waitlist`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WaitlistQuery {
    pub app: Option<String>,
    pub list: Option<String>,
    pub status: Option<SignupStatus>,
    pub limit: u32,
}

impl WaitlistQuery {
    pub fn pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("app", self.app.clone().unwrap_or_default()),
            ("list", self.list.clone().unwrap_or_default()),
            ("status", self.status.map(SignupStatus::as_str).unwrap_or_default().to_string()),
            ("limit", limit_str(self.limit)),
        ]
    }
}

/// The contract allows 1 to 500 rows a page; 0 leaves the server's default.
fn limit_str(limit: u32) -> String {
    if limit == 0 { String::new() } else { limit.clamp(1, 500).to_string() }
}

/// Percent-encodes one path segment.
pub fn encode(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_path_segments() {
        assert_eq!(encode("sign up/now"), "sign%20up%2Fnow");
        assert_eq!(encode("t_0abc"), "t_0abc");
    }

    #[test]
    fn leaves_out_empty_filters() {
        let q = TicketQuery { kind: Some(TicketKind::Bug), limit: 500, ..Default::default() };
        let pairs: Vec<_> = q.pairs().into_iter().filter(|(_, v)| !v.is_empty()).collect();
        assert_eq!(pairs, vec![("kind", "bug".to_string()), ("limit", "500".to_string())]);
        let q = WaitlistQuery { status: Some(SignupStatus::Joined), limit: 9000, ..Default::default() };
        assert_eq!(q.pairs()[3], ("limit", "500".to_string()));
    }
}
