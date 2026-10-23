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

/// Reads the code the console shows when it makes a device token (Settings › Mobile app): `logsure://<host>/cc_…`.
/// Answers the server's address and the token. Local hosts get `http://`, every other host `https://`.
pub fn parse_code(code: &str) -> Option<(String, String)> {
    let rest = code.trim().strip_prefix("logsure://")?;
    let (host, token) = rest.rsplit_once('/')?;
    let host = host.trim_end_matches('/');
    if host.is_empty() || host.contains('/') || !looks_like_token(token) {
        return None;
    }
    let name =
        host.rsplit_once(':').map_or(host, |(h, port)| if port.chars().all(|c| c.is_ascii_digit()) { h } else { host });
    let local = matches!(name, "localhost" | "127.0.0.1" | "[::1]") || name.ends_with(".localhost");
    Some((format!("{}://{host}", if local { "http" } else { "https" }), token.to_string()))
}

/// Device tokens are `cc_` and a random part.
pub fn looks_like_token(token: &str) -> bool {
    token.strip_prefix("cc_").is_some_and(|rest| rest.len() >= 16 && !rest.contains(char::is_whitespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "cc_0123456789abcdefABCDEF_-xyz";

    #[test]
    fn reads_the_consoles_code() {
        let code = format!("logsure://logsure.example.com/{TOKEN}");
        assert_eq!(parse_code(&code), Some((String::from("https://logsure.example.com"), TOKEN.to_string())));
        let local = format!(" logsure://localhost:8080/{TOKEN}\n");
        assert_eq!(parse_code(&local), Some((String::from("http://localhost:8080"), TOKEN.to_string())));
    }

    #[test]
    fn refuses_what_is_not_a_code() {
        assert_eq!(parse_code(TOKEN), None);
        assert_eq!(parse_code("logsure://logsure.example.com/"), None);
        assert_eq!(parse_code("logsure://logsure.example.com/abc"), None);
        assert_eq!(parse_code(&format!("logsure:///{TOKEN}")), None);
        assert_eq!(parse_code(&format!("https://logsure.example.com/{TOKEN}")), None);
    }

    #[test]
    fn tokens() {
        assert!(looks_like_token(TOKEN));
        assert!(!looks_like_token("cc_short"));
        assert!(!looks_like_token("xx_0123456789abcdefABCDEF"));
    }
}
