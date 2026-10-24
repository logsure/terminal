# LogSure terminal

A terminal console for LogSure: browse your apps' analytics, the inbox and the waitlists from the keyboard. Written
in Rust, talking to a [LogSure server](https://github.com/logsure/server) through its console API (`/api/v1`).

- **Full screen:** run `logsure` and move between views with `:` commands and a few keys.
- **One shot:** `logsure apps --json` and friends answer once and exit, for scripts.
- **Several servers:** sign in to as many as you like and switch with `:ctx`.

## Install

With [Homebrew](https://brew.sh) (macOS and Linux), from this repository's own tap:

```bash
brew tap logsure/terminal https://github.com/logsure/terminal
brew trust logsure/terminal
brew install logsure
```

Recent Homebrew versions load formulas from other taps only once you trust them, hence the second line. It builds from source, so the first install takes a few minutes. `brew upgrade logsure` picks up new releases;
`brew install --HEAD logsure` builds the latest `main`.

With Cargo (Rust 1.88 or newer):

```bash
cargo install --git https://github.com/logsure/terminal
```

Or from a checkout: `cargo build --release`, then copy `target/release/logsure` onto your `PATH`. The binary is
`logsure`; the server's is `logsure-server`, so both can be installed side by side.

## Sign in

In the web console, open Settings › Mobile app (two-factor sign-in has to be on), give the device a name such as
"Terminal" and choose **Make a code**. The page shows a QR code and, under "No camera?", a code like
`logsure://logsure.example.com/cc_…`. It is shown once, so copy it before leaving the page. Then:

```bash
logsure login
```

and paste the code when asked. The code holds the server's address and the device token, so nothing else is
needed. `logsure login https://logsure.example.com` with just the token (`cc_…`) works too, and either can be piped
in or passed with `--token`, though typing it on the command line leaves it in your shell's history.

The token is checked against the server and kept in the OS keychain (Keychain on macOS, Credential Manager on
Windows, the Secret Service on Linux), never in a file. `--name work` gives the server a short name; the default is
its host. To sign a device out, remove it under Settings › Mobile app, or run `logsure logout`. Signing in through
the browser (OAuth) comes later.

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
| `:range <window>` | The time window for every view (see below) |
| `:ctx [name]` | List the servers, or switch to one |
| `:refresh` | Ask the server again; views also refresh every 30 seconds |
| `:q` | Quit |

`:range` (and `--range` on the subcommands) takes any window:

| Window | Examples |
|---|---|
| The server's presets | `30m` `1h` `today` `24h` `7d` `30d` `90d` `12mo` `all` |
| A length back from today | `14d` `6w` `3mo` `2y` (also `2 weeks`, `6 months`) |
| A named stretch | `yesterday` `thisweek` `lastweek` `thismonth` `lastmonth` `thisyear` `lastyear` |
| A day, a month or a year | `2026-09-15` `2026-09` `2025` |
| A span | `2026-09-01..2026-09-15` `2026-06..2026-08` `2026-09..` (up to today) |

Days are UTC, as the server keeps them; weeks start on Monday. A relative window such as `14d` keeps meaning the
last 14 days while the console stays open. `range = "lastmonth"` in the configuration sets the window views open
with.

The inbox and the waitlist are not windowed by the server, so `:range` shows the requests and signups that arrived
within the window there (`:range all` shows everything).

| Key | Does |
|---|---|
| `↑` `↓` `PgUp` `PgDn` `Home` `End` | Move (scroll, when reading a request) |
| `Enter` | Open the selected row |
| `/` | Filter the current list as you type; `Enter` keeps it, `Esc` clears it |
| `?` | Help |
| `Esc` | Back, like `q`, but never quits. When help, the `:` line, a filter or a note is open, it closes that first |
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
logsure apps --range lastmonth            # any window :range takes
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

## Releasing

Bump `version` in `Cargo.toml`, tag the commit (`git tag -a v0.2.0 -m v0.2.0`) and push the tag, then point
`Formula/logsure.rb` at it: its `tag` and its `revision` (`git rev-list -n1 v0.2.0`).

## License

[Mozilla Public License 2.0](LICENSE).
