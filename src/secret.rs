//! Tokens live in the OS keychain (Keychain on macOS, Credential Manager on Windows, the Secret Service on Linux),
//! one entry per server address. `LOGSURE_TOKEN` overrides the keychain, for headless use.

use anyhow::{Context, Result, anyhow};

const SERVICE: &str = "logsure";

fn entry(url: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, url).context("opening the keychain")
}

pub fn store(url: &str, token: &str) -> Result<()> {
    entry(url)?.set_password(token).context("saving the token in the keychain")
}

pub fn load(url: &str) -> Result<String> {
    if let Some(token) = std::env::var("LOGSURE_TOKEN").ok().filter(|t| !t.is_empty()) {
        return Ok(token);
    }
    match entry(url)?.get_password() {
        Ok(token) => Ok(token),
        Err(keyring::Error::NoEntry) => Err(anyhow!("not signed in to {url}: run `logsure login {url} --token cc_…`")),
        Err(e) => Err(e).context("reading the token from the keychain"),
    }
}

pub fn forget(url: &str) -> Result<()> {
    match entry(url)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).context("removing the token from the keychain"),
    }
}

/// Device tokens are `cc_` and a random part.
pub fn looks_like_token(token: &str) -> bool {
    token.strip_prefix("cc_").is_some_and(|rest| rest.len() >= 16 && !rest.contains(char::is_whitespace))
}
