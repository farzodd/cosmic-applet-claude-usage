# Claude Usage — COSMIC panel applet

A panel applet for the [COSMIC](https://system76.com/cosmic) desktop (Pop!_OS 24.04) that shows your Claude plan usage limits at a glance: the current **session** (5-hour window) and the current **week**.

The panel shows the Claude mark followed by the two readings. Each reading can be a small square chart, the percentage, or both. Click the applet for details: a bar per limit, when each resets, and model-specific weekly limits if your plan has them.

## Features

- **Chart / Value / Both** display modes, switchable from the popup.
- **Pace marker**: a tick on each chart showing how far through the window you are. If the fill is above the tick, you're using the limit faster than an even pace.
- Optional **S / W** labels.
- Refreshes every minute. If the API rate-limits it, it backs off for 15 minutes.
- Works on horizontal and vertical panels.

## How it gets the data

It calls the same usage endpoint that Claude Code's `/usage` command uses, authenticated with the OAuth login stored by the [Claude Code CLI](https://docs.claude.com/en/docs/claude-code) in `~/.claude/.credentials.json` (or `$CLAUDE_CONFIG_DIR/.credentials.json`).

You log in once. When the short-lived access token expires, the applet refreshes it and writes the new token back to the same file, so the CLI stays logged in too.

> This endpoint is undocumented and may change without notice. This project is not affiliated with or endorsed by Anthropic.

## Install

Requirements: Pop!_OS 24.04 / COSMIC, a Rust toolchain ([rustup](https://rustup.rs)), and `libxkbcommon-dev`.

```bash
sudo apt install libxkbcommon-dev
```

Log in with the Claude Code CLI (one time):

```bash
curl -fsSL https://claude.ai/install.sh | bash
```

Then run `claude` and type `/login`.

Build and install for the current user (no sudo; installs to `~/.local`):

```bash
./install.sh
```

Then go to **Settings → Desktop → Panel → Configure panel applets → Add applet → Claude Usage**.

To update after pulling changes, run `./install.sh` again, then restart the panel so it picks up the new binary:

```bash
pkill -x cosmic-panel
```

`cosmic-session` restarts the panel right away.

To uninstall:

```bash
./install.sh uninstall
```

Then remove the applet from the panel in Settings.

## Build notes

The first build compiles libcosmic and several hundred dependencies. `install.sh` limits parallelism to 4 jobs by default. Override it with `JOBS`:

```bash
JOBS=8 ./install.sh
```

A build like this loads the CPU and memory hard for a few minutes. If `rustc`, LLVM or `gcc` crash with segfaults, "illegal instruction" or "general protection fault" errors in different places on each run, suspect an unstable CPU overclock or RAM XMP/EXPO profile before suspecting the toolchain.

## Configuration

Settings live in `~/.config/cosmic/io.github.farzodd.CosmicAppletClaudeUsage/v1/`, one file per key:

| Key | Default | Meaning |
| --- | --- | --- |
| `display` | `Both` | `Chart`, `Value` or `Both` |
| `show_labels` | `true` | Show S / W labels |
| `show_pace` | `true` | Draw the pace marker |
| `refresh_secs` | `60` | Refresh interval in seconds (minimum 60) |

The applet picks up changes immediately.

## License

GPL-3.0-only. See [LICENSE](LICENSE).

The Claude name and logo are trademarks of Anthropic.
