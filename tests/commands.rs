//! Parsing the `:` commands.

use logsure::api::{SignupStatus, TicketKind};
use logsure::command::{Command, complete, parse};

#[test]
fn views() {
    assert_eq!(parse(":apps"), Ok(Command::Apps));
    assert_eq!(parse("apps"), Ok(Command::Apps));
    assert_eq!(parse("  :app shop "), Ok(Command::App(String::from("shop"))));
    assert_eq!(parse(":events"), Ok(Command::Events(None)));
    assert_eq!(parse(":events shop"), Ok(Command::Events(Some(String::from("shop")))));
    assert_eq!(parse(":ctx"), Ok(Command::Ctx(None)));
    assert_eq!(parse(":ctx work"), Ok(Command::Ctx(Some(String::from("work")))));
    assert_eq!(parse(":q"), Ok(Command::Quit));
    assert_eq!(parse(":help"), Ok(Command::Help));
}

#[test]
fn inbox_takes_kind_and_status_in_any_order() {
    assert_eq!(parse(":inbox"), Ok(Command::Inbox { kind: None, status: None }));
    let both = Ok(Command::Inbox { kind: Some(TicketKind::Bug), status: Some(String::from("resolved")) });
    assert_eq!(parse(":inbox bug resolved"), both);
    assert_eq!(parse(":inbox resolved bug"), both);
    assert_eq!(parse(":inbox active"), Ok(Command::Inbox { kind: None, status: Some(String::from("active")) }));
    assert!(parse(":inbox bugs").is_err());
    assert!(parse(":inbox bug open extra").is_err());
}

#[test]
fn waitlist_takes_list_and_status() {
    assert_eq!(parse(":waitlist"), Ok(Command::Waitlist { list: None, status: None }));
    assert_eq!(
        parse(":waitlist beta invited"),
        Ok(Command::Waitlist { list: Some(String::from("beta")), status: Some(SignupStatus::Invited) })
    );
    assert_eq!(parse(":waitlist joined"), Ok(Command::Waitlist { list: None, status: Some(SignupStatus::Joined) }));
}

#[test]
fn ranges() {
    for key in ["30m", "1h", "today", "24h", "7d", "30d", "90d", "12mo", "all"] {
        assert_eq!(parse(&format!(":range {key}")), Ok(Command::Range(key.to_string())));
    }
    assert!(parse(":range 8d").unwrap_err().contains("not a range"));
    assert!(parse(":range").is_err());
}

#[test]
fn mistakes() {
    assert!(parse(":").is_err());
    assert!(parse(":app").unwrap_err().contains("needs an app"));
    assert!(parse(":app a b").is_err());
    assert!(parse(":apps x").is_err());
    assert!(parse(":frobnicate").unwrap_err().contains("unknown command"));
}

#[test]
fn completion() {
    assert_eq!(complete("wa"), Some(String::from("waitlist ")));
    assert_eq!(complete("ra"), Some(String::from("range ")));
    assert_eq!(complete("app"), None, "app and apps both fit");
    assert_eq!(complete("range 7"), None);
}
