# LogSure terminal

A terminal console for LogSure: browse your apps' analytics, the inbox and the waitlists from the keyboard. Written
in Rust, talking to a [LogSure server](https://github.com/logsure/server) through its console API (`/api/v1`).

- **Full screen:** run `logsure` and move between views with `:` commands and a few keys.
- **One shot:** `logsure apps --json` and friends answer once and exit, for scripts.
- **Several servers:** sign in to as many as you like and switch with `:ctx`.

## Install

You need Rust 1.88 or newer.

```bash
cargo install --git https://github.com/logsure/terminal
```

Or from a checkout: `cargo build --release`, then copy `target/release/logsure` onto your `PATH`. The binary is
`logsure`; the server's is `logsure-server`, so both can be installed side by side.

## Sign in

Make a device token in the console (Settings › Mobile app), then:

```bash
logsure login https://logsure.example.com
```

The token (`cc_…`) is asked for, so it stays out of your shell's history; `--token cc_…` passes it directly, and
it can be piped in too. It is checked against the server and kept in the OS keychain (Keychain on macOS, Credential
Manager on Windows, the Secret Service on Linux), never in a file. `--name work` gives the server a short name;
the default is its host. Signing in through the browser (OAuth) comes later.

The servers live in `~/.config/logsure/config.toml` (or `$XDG_CONFIG_HOME/logsure/config.toml`):

```toml
current = "work"
range = "7d"        # the window views open with

[servers.work]
url = "https://logsure.example.com"

[servers.home]
url = "http://localhost:8080"
```

`logsure ctx` lists them, `logsure ctx home` switches, `logsure logout [name]` forgets one and its token.

For headless use, `LOGSURE_URL` and `LOGSURE_TOKEN` point at a server without the configuration or the keychain.
`LOGSURE_SERVER` (or `--server`) picks a server by name for one run, and `LOGSURE_CONFIG` moves the configuration
file.

## The full-screen console

| Command | View |
|---|---|
| `:apps` | Every app: visitors, pageviews, change against the previous period, visitors now, and a trend |
| `:app <id>` | One app: totals, visitors over time, top pages, sources and countries |
| `:events [app]` | Custom events with visitors, counts, conversion and value; Enter shows one event's properties |
| `:inbox [kind] [status]` | Feedback, bug reports and messages, e.g. `:inbox bug open` |
| `:waitlist [list] [status]` | Signups, with every list's counts, e.g. `:waitlist beta waiting` |
| `:range <window>` | `30m` `1h` `today` `24h` `7d` `30d` `90d` `12mo` `all`, for every view |
| `:ctx [name]` | List the servers, or switch to one |
| `:refresh` | Ask the server again; views also refresh every 30 seconds |
| `:q` | Quit |

The inbox and the waitlist are not windowed by the server, so `:range` shows the requests and signups that arrived
within the window there (`:range all` shows everything).

| Key | Does |
|---|---|
| `↑` `↓` `PgUp` `PgDn` `Home` `End` | Move (scroll, when reading a request) |
| `Enter` | Open the selected row |
| `/` | Filter the current list as you type; `Enter` keeps it, `Esc` clears it |
| `?` | Help |
| `q` | Back; quits from the first view |
| `Tab` | Complete a command |
| `Ctrl-R` | Refresh |
| `Ctrl-C` | Quit |
| `e` | In an app: its events |
| `r` | In the inbox or a request: resolve |
| `n` | In the inbox or a request: add a note (`Enter` saves, `Esc` cancels) |
| `t` / `s` | In the inbox: next kind / next status |
| `i` / `j` / `w` | In the waitlist: mark invited / joined / waiting (sends no email) |
| `l` / `s` | In the waitlist: next list / next status |

Times are shown in UTC, as the server keeps them.

## One-shot subcommands

```bash
logsure apps --range 30d                  # every app
logsure app shop                          # one app's totals, pages, sources, countries
logsure events shop                       # its custom events
logsure events shop signup                # one event's properties
logsure inbox --kind bug --status active  # requests
logsure ticket t_0abc…                    # one request with its notes
logsure waitlist --list beta --status waiting
```

Each takes `--json` to print the server's answer as it came, and `--server <name>` to use another server.
`logsure --help` and `logsure <command> --help` list every option.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The API client is tested against answers copied from a real server (`tests/fixtures`). `tests/live.rs` drives
every view against a running server with demo data and is skipped unless `LOGSURE_URL` and `LOGSURE_TOKEN` are set;
it changes data (resolves a request, adds a note, invites a signup), so point it at a throwaway server only.

## License

[Mozilla Public License 2.0](LICENSE).
