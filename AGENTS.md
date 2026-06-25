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

## Release Publication Pipeline

Use this pipeline when publishing a fix release:

1. Inspect the worktree and protect unrelated user changes:
```bash
git status --short --branch
```

2. Bump the patch version in all app manifests:
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock` package entry for `animesubs`
- `src-tauri/tauri.conf.json`

3. Run local validation before committing:
```bash
bun run vue-tsc --noEmit
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
git diff --check
```

4. Commit the fix and version bump:
```bash
git add <changed-files>
git commit -m "<clear release/fix message>"
```

5. Create an annotated version tag:
```bash
git tag -a vX.Y.Z -m "vX.Y.Z"
```

6. Push `main` and the tag:
```bash
git push origin main
git push origin vX.Y.Z
```

7. Monitor GitHub Actions. The `Build` workflow runs on `v*` tags and creates the
   GitHub release with assets:
```bash
gh run list --repo enrell/animesubs --limit 10
gh run view <run-id> --repo enrell/animesubs --json status,conclusion,jobs
```

8. If a release runner is stuck because a GitHub-hosted runner label is obsolete,
   cancel the stuck run, update `.github/workflows/build.yml` to a supported
   runner label, commit that CI fix, move the tag to the new commit, and force
   push the tag only if the release was not successfully published yet:
```bash
gh run cancel <run-id> --repo enrell/animesubs
git tag -f -a vX.Y.Z -m "vX.Y.Z"
git push origin main
git push --force origin vX.Y.Z
```

9. After the release exists and all assets are uploaded, write the changelog in
   the release body:
```bash
gh release view vX.Y.Z --repo enrell/animesubs --json url,assets,body
gh release edit vX.Y.Z --repo enrell/animesubs --title "vX.Y.Z" --notes "<changelog>"
```

10. If the release fixes a GitHub issue, comment on and close the issue only
    after the release is published.

## GitHub Issues

- When commenting on or closing an issue, use the same language as the issue
  title/body. If the reporter wrote in English, respond in English; if they wrote
  in Portuguese, respond in Portuguese.
- Reference the published release URL when closing an issue fixed by a release.

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
