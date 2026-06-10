# AGENTS.md

Project instructions for AI coding agents working on AnimeSubs.

## Project Overview

AnimeSubs is a Tauri desktop application for extracting, translating, and embedding subtitles into anime videos. It uses a Vue.js frontend and Rust backend.

## Tech Stack

- **Frontend**: Vue 3 + TypeScript + Naive UI + Vite
- **Backend**: Rust + Tauri 2
- **Package Manager**: Bun
- **Build Tool**: Tauri CLI

## Project Structure

```
animesubs/
├── src/                    # Vue frontend
│   ├── components/         # Vue components
│   ├── composables/        # Vue composables
│   ├── config/             # Configuration files
│   └── api/                # API layer
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── commands/       # Tauri commands
│   │   ├── providers/      # LLM provider integrations
│   │   ├── models.rs       # Data models
│   │   └── utils.rs        # Utility functions
│   └── Cargo.toml
└── public/                 # Static assets
```

## Common Commands

```bash
# Development
bun run tauri dev

# Build for production
bun run tauri build

# Frontend typecheck
bun run vue-tsc --noEmit

# Rust linter
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

# Run tests
cargo test --manifest-path src-tauri/Cargo.toml

# Quality assurance
rustquty qa --project-dir src-tauri
```

## Code Style

### Rust
- Follow standard Rust conventions
- All clippy warnings are errors (`-D warnings`)
- Max line length: 100 characters
- Use `LazyLock` for static regex patterns
- Use `.clamp()` instead of `.max().min()`
- Use `.push('c')` instead of `.push_str("c")` for single characters

### TypeScript/Vue
- Use Vue 3 Composition API
- Naive UI component library
- Strict TypeScript (no `any`)

## LLM Providers

The app supports multiple LLM providers via OpenAI-compatible API:

| Provider | Auth Required |
|----------|---------------|
| OpenAI | Yes |
| Gemini | Yes |
| Ollama | No |
| LM Studio | No |
| llama.cpp | No |
| OpenRouter | Yes |
| NVIDIA NIM | Yes |
| MiniMax | Yes |

## Testing

- Rust tests use `tokio::test` for async tests
- Tests mock HTTP servers using `TcpListener`
- Each provider has dedicated test coverage

## Quality Gate

PRs must pass `rustquty qa` which checks:

- Code metrics against baseline
- Long lines (>100 chars)
- Clippy warnings
- Test coverage

To update baseline after improvements:
```bash
rustquty update-baseline --project-dir src-tauri
```

## Dependencies

### Rust (Cargo.toml)
- `tauri` - Desktop framework
- `serde` / `serde_json` - Serialization
- `reqwest` - HTTP client
- `regex` - Regular expressions
- `tokio` - Async runtime

### Frontend (package.json)
- `vue` - UI framework
- `naive-ui` - Component library
- `@tauri-apps/api` - Tauri API bindings

## Branch Strategy

- `main` - Stable releases
- Feature branches - New features
- All PRs require CI passing (clippy, tests, rustquty)
