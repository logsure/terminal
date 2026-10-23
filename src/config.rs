//! `~/.config/logsure/config.toml`: the servers this app knows, by name, and the current one.
//!
//! ```toml
//! current = "work"
//!
//! [servers.work]
//! url = "https://logsure.example.com"
//! ```
//!
//! Tokens are not kept here but in the OS keychain (see `secret`).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Config {
    /// The name of the server views and subcommands use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    /// The time window views open with, e.g. `7d`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<String>,
    #[serde(default)]
    pub servers: BTreeMap<String, Server>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Server {
    pub url: String,
}

/// `$LOGSURE_CONFIG`, else `$XDG_CONFIG_HOME/logsure/config.toml`, else `~/.config/logsure/config.toml`.
pub fn path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("LOGSURE_CONFIG") {
        return Ok(PathBuf::from(p));
    }
    let base = match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => {
            let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
            PathBuf::from(home.ok_or_else(|| anyhow!("no home folder to keep the configuration in"))?).join(".config")
        }
    };
    Ok(base.join("logsure").join("config.toml"))
}

impl Config {
    pub fn load() -> Result<Config> {
        let path = path()?;
        match fs::read_to_string(&path) {
            Ok(text) => Config::parse(&text).with_context(|| format!("reading {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    pub fn parse(text: &str) -> Result<Config> {
        Ok(toml::from_str(text)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = path()?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        fs::write(&path, toml::to_string_pretty(self)?).with_context(|| format!("writing {}", path.display()))
    }

    /// The named server, or the current one.
    pub fn server(&self, name: Option<&str>) -> Result<(String, Server)> {
        let name = match name.or(self.current.as_deref()) {
            Some(n) => n,
            None if self.servers.len() == 1 => self.servers.keys().next().map(String::as_str).unwrap_or_default(),
            None => bail!("no server yet: sign in with `logsure login <url> --token cc_…`"),
        };
        let server = self.servers.get(name).ok_or_else(|| anyhow!("no server named {name:?} (see `logsure ctx`)"))?;
        Ok((name.to_string(), server.clone()))
    }

    /// Adds or updates a server and makes it the current one.
    pub fn add(&mut self, name: &str, url: &str) {
        self.servers.insert(name.to_string(), Server { url: url.to_string() });
        self.current = Some(name.to_string());
    }

    /// Removes a server; when it was the current one, another (if any) takes its place.
    pub fn remove(&mut self, name: &str) -> Option<Server> {
        let gone = self.servers.remove(name);
        if self.current.as_deref() == Some(name) {
            self.current = self.servers.keys().next().cloned();
        }
        gone
    }
}

/// The name a server gets when none is given: its host, without the port's colon.
pub fn default_name(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let name: String = host.chars().map(|c| if c == ':' { '-' } else { c }).collect();
    if name.is_empty() { String::from("default") } else { name }
}

/// Checks and tidies a server address: `http(s)://host[:port]`, without a trailing slash.
pub fn normalize_url(url: &str) -> Result<String> {
    let url = url.trim();
    let url = if url.contains("://") { url.to_string() } else { format!("https://{url}") };
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        bail!("the server address must start with https:// or http://");
    }
    if url.split_once("://").is_some_and(|(_, rest)| rest.trim_matches('/').is_empty()) {
        bail!("the server address has no host");
    }
    Ok(url.trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_servers_and_current() {
        let c = Config::parse(
            "current = \"work\"\nrange = \"30d\"\n[servers.work]\nurl = \"https://a.example.com\"\n[servers.home]\nurl = \"http://localhost:8080\"\n",
        )
        .unwrap();
        assert_eq!(c.server(None).unwrap().1.url, "https://a.example.com");
        assert_eq!(c.server(Some("home")).unwrap().1.url, "http://localhost:8080");
        assert!(c.server(Some("nope")).is_err());
        assert_eq!(c.range.as_deref(), Some("30d"));
    }

    #[test]
    fn round_trips() {
        let mut c = Config::default();
        c.add("a", "https://a.example.com");
        c.add("b", "https://b.example.com");
        assert_eq!(c.current.as_deref(), Some("b"));
        let again = Config::parse(&toml::to_string_pretty(&c).unwrap()).unwrap();
        assert_eq!(again, c);
        c.remove("b");
        assert_eq!(c.current.as_deref(), Some("a"));
    }

    #[test]
    fn names_and_urls() {
        assert_eq!(default_name("https://logsure.example.com/"), "logsure.example.com");
        assert_eq!(default_name("http://localhost:8080"), "localhost-8080");
        assert_eq!(normalize_url("logsure.example.com/").unwrap(), "https://logsure.example.com");
        assert_eq!(normalize_url("http://localhost:8080/").unwrap(), "http://localhost:8080");
        assert!(normalize_url("ftp://x").is_err());
        assert!(normalize_url("https://").is_err());
    }
}
