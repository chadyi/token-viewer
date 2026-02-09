# Token Viewer

A desktop application for visualizing token usage and costs from AI coding tools (Claude Code, Codex CLI, OpenCode).

Built with [Tauri v2](https://tauri.app/) + React 19 + TypeScript.

## Features

- **Local-only** — scans JSONL/JSON logs on your machine, no data leaves your computer
- **Incremental scan** — remembers file offsets, refreshes in milliseconds
- **Accurate cost estimation** — uses LiteLLM pricing with tiered rates (200k+ token threshold)
- **Multi-tool support** — Claude Code, Codex CLI, OpenCode
- **Dashboard** — total requests, tokens, cost at a glance
- **Charts** — token usage by day (area chart), breakdown by tool (pie chart)
- **Flexible date grouping** — Day / Week / Month / Year / All (By Model)
- **Expandable detail rows** — click any date row to see per-model breakdown
- **Cross-platform** — Windows, macOS (Intel + Apple Silicon), Linux

## Download

Pre-built installers are available on the [Releases](../../releases) page:

| Platform | Format |
|----------|--------|
| Windows | `.msi`, `.exe` |
| macOS (Apple Silicon) | `.dmg` |
| macOS (Intel) | `.dmg` |
| Linux | `.deb`, `.AppImage`, `.rpm` |

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) 22+
- [Rust](https://rustup.rs/) stable
- [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)

### Setup

```bash
npm install
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

## How It Works

Token Viewer scans local log files from:

- **Claude Code** — `~/.claude/projects/**/conversations/*.jsonl`
- **Codex CLI** — `~/.codex/conversations/*.jsonl`
- **OpenCode** — `~/.opencode/sessions/*/session.json`

It parses each request's token counts (input, output, cache read, cache write) and estimates cost using model pricing data from [LiteLLM](https://github.com/BerriAI/litellm).

## License

MIT
