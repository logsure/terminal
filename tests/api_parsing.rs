//! The client's types against answers copied from a real server (`tests/fixtures`, app names replaced).

use logsure::api::{
    self, Dashboard, EventDetail, Me, Overview, Page, SignupStatus, Ticket, TicketKind, TicketStatus, TicketSummary,
    WaitlistPage,
};
use serde_json::Value;

fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn parse<T: serde::de::DeserializeOwned>(name: &str) -> T {
    api::decode(fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn me() {
    let me: Me = parse("me");
    assert_eq!(me.device.unwrap().id, "d_term0001");
    assert_eq!(me.apps.len(), 3);
    assert_eq!(me.apps[2].id, "shop");
    assert_eq!(me.apps[2].domains, ["shop.example.com", "www.shop.example.com"]);
    let counts = me.counts.unwrap();
    assert_eq!(counts.all.total, 40);
    assert_eq!(
        counts.by_kind.values().sum::<u64>(),
        counts.all.open + counts.all.progress,
        "by kind counts the active ones"
    );
}

#[test]
fn overview() {
    let o: Overview = parse("overview");
    assert_eq!(o.range.key, "7d");
    assert_eq!(o.range.bucket, "day");
    let shop = o.apps.iter().find(|a| a.app_id == "shop").unwrap();
    assert_eq!((shop.visitors, shop.pageviews, shop.realtime), (2283, 6181, 1));
    assert_eq!(shop.prev.unwrap().visitors, 2197);
    assert_eq!(shop.spark.len(), 30);
    assert_eq!(shop.series.len(), 7);
}

#[test]
fn dashboard() {
    let d: Dashboard = parse("dashboard");
    assert_eq!(d.app.name, "Shop");
    assert_eq!(d.range.key, "30d");
    assert_eq!(d.summary.totals.visitors, 8705);
    assert!(d.summary.prev.is_none(), "no previous period before the first event");
    assert!((d.summary.totals.bounce_rate.unwrap() - 0.357).abs() < 0.001);
    assert_eq!(d.series.len(), 30);
    assert_eq!(d.pages[0].value.as_deref(), Some("/"));
    assert!(d.sources.iter().any(|r| r.value.is_none()), "direct visits have no value");
    assert_eq!(d.events.len(), 3);
    assert_eq!(d.events[1].name, "signup");
    assert_eq!(d.events[1].value, Some(889.11));
    assert!(d.events[0].value.is_none());
    assert_eq!(d.live.visitors, 1);
}

#[test]
fn event() {
    let e: EventDetail = parse("event");
    assert_eq!(e.name, "download");
    assert!(!e.series.is_empty());
    let mac = e.props.iter().find(|p| p.value == "mac").unwrap();
    assert_eq!((mac.key.as_str(), mac.visitors, mac.count), ("platform", 533, 562));
}

#[test]
fn tickets() {
    let page: Page<TicketSummary> = parse("tickets");
    assert_eq!(page.total, 37);
    let kinds: Vec<TicketKind> = page.items.iter().map(|t| t.kind).collect();
    assert_eq!(kinds, [TicketKind::Bug, TicketKind::Feedback, TicketKind::Feedback, TicketKind::Message]);
    let feedback = &page.items[1];
    assert_eq!(feedback.rating, Some(4));
    assert_eq!(feedback.status, TicketStatus::Open);
    assert!(page.items.iter().any(|t| t.email.is_none()), "anonymous requests have no email");
}

#[test]
fn ticket() {
    let t: Ticket = parse("ticket");
    assert_eq!(t.kind, TicketKind::Bug);
    assert_eq!(t.status, TicketStatus::Progress);
    assert_eq!(t.meta.host.as_deref(), Some("shop.example.com"));
    let keys: Vec<&str> = t.data.keys().map(String::as_str).collect();
    assert_eq!(keys, ["steps", "expected", "version"], "the server's order is kept");
    assert_eq!(t.notes.len(), 3);
    assert_eq!(t.notes[1].kind, "status");
    assert_eq!(t.attachments[0].mime, "image/png");
}

#[test]
fn waitlist() {
    let w: WaitlistPage = parse("waitlist");
    assert_eq!(w.total, 150);
    assert_eq!(w.items.len(), 4);
    assert_eq!(w.items[0].status, SignupStatus::Waiting);
    let beta = w.lists.iter().find(|l| l.list == "beta").unwrap();
    assert_eq!((beta.waiting, beta.invited, beta.joined), (62, 20, 10));
}

#[test]
fn errors() {
    let body = fixture("error").to_string();
    let e = api::parse_error(401, &body);
    assert!(e.is_unauthorized());
    match e {
        api::Error::Api { error, .. } => assert_eq!(error.code, "unauthorized"),
        other => panic!("{other:?}"),
    }
    let e = api::parse_error(502, "<html>Bad gateway</html>");
    assert_eq!(e.to_string(), "the server answered 502");
}

#[test]
fn unknown_fields_are_ignored() {
    let mut value = fixture("overview");
    value["somethingNew"] = Value::from(1);
    value["apps"][0]["alsoNew"] = Value::from("x");
    assert!(api::decode::<Overview>(value).is_ok());
}
