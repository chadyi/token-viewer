# Token Viewer — AI Token Usage & Cost Tracker for Claude Code, Codex CLI, and OpenCode

[![Release](https://img.shields.io/github/v/release/chadyi/token-viewer?display_name=tag)](https://github.com/chadyi/token-viewer/releases)
[![Build](https://img.shields.io/github/actions/workflow/status/chadyi/token-viewer/build.yml?label=build)](https://github.com/chadyi/token-viewer/actions/workflows/build.yml)
[![License](https://img.shields.io/github/license/chadyi/token-viewer)](./LICENSE)
[![Stars](https://img.shields.io/github/stars/chadyi/token-viewer?style=social)](https://github.com/chadyi/token-viewer/stargazers)

A **local-first desktop app** to analyze **token usage** and **estimated AI cost** from:
- Claude Code
- Codex CLI
- OpenCode

Built with **Tauri v2 + React 19 + TypeScript**.

> If Token Viewer helps you, please star the repo ⭐

---

English | [简体中文](./README.zh-CN.md)

## Why Token Viewer

Most coding-agent users can see logs, but not a clear answer to:
- How many tokens did I use today/week/month?
- Which model is expensive?
- Is cache helping or just noise?

Token Viewer turns raw local logs into a clean dashboard with trend charts and per-model breakdowns.

## Key Features

- **Local-only**: scans local JSON/JSONL logs, no server required
- **Fast incremental refresh**: remembers file offsets for quick updates
- **Token analytics dashboard**: requests, input/output tokens, totals, cost
- **Cost estimation**: based on LiteLLM pricing tables (supports tiered rates)
- **Date grouping**: Day / Week / Month / Year / All
- **Model drill-down**: click rows to expand per-model usage details
- **Cross-platform desktop**: Windows, macOS (Intel + Apple Silicon), Linux

## Screenshots

### Dashboard
![Dashboard](docs/screenshots/dashboard.png)

### By Date (expandable)
![By Date](docs/screenshots/bydate.png)

### By Model (All)
![By Model](docs/screenshots/bymodel.png)

## Download

Pre-built installers are available in **[Releases](https://github.com/chadyi/token-viewer/releases)**:

| Platform | Format |
| --- | --- |
| Windows | `.msi`, `.exe` |
| macOS (Apple Silicon) | `.dmg` |
| macOS (Intel) | `.dmg` |
| Linux | `.deb`, `.AppImage`, `.rpm` |

## Supported Log Sources

Token Viewer currently scans:

- **Claude Code**
  - `~/.config/claude/projects/**/*.jsonl`
  - `~/.claude/projects/**/*.jsonl`
- **Codex CLI**
  - `~/.codex/sessions/**/*.jsonl`
- **OpenCode**
  - `~/.local/share/opencode/storage/message/**/*.json`

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

## Search Keywords

AI token tracker, token usage dashboard, token cost viewer, Claude Code token usage, Codex CLI token usage, OpenCode token analytics, local AI cost monitor, Tauri desktop analytics.

## License

MIT
