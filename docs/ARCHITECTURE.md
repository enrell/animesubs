# AnimeSubs — Architecture: one product, a local server underneath

Status: proposal + first slice (branch `fusao/subtitle-toolkit`) · 2026-11-08

This note describes how AnimeSubs absorbs the engine of
`psyche-subtitle-toolkit` (and what is worth keeping from
`shinkai-translator`) and becomes a desktop app on top of a local server that
also works on its own, for scripts, agents and media servers. `docs/DESIGN.md`
remains the source of truth for the interface; this document covers what runs
underneath it.

---

## 1. Decision

AnimeSubs stays the product: it is the original project, it has real users,
and its workflow (drop videos, pick a language, get translated subtitles
embedded with the originals backed up) is the one that works best.
The toolkit is the strongest engine; it moves into this repository with its
history. Shinkai is the oldest of the three and only contributes what the
other two lack.

Target shape, borrowed from the author's Anything project (desktop app +
daemon on localhost + standalone CLI):

```
                ┌──────────────────────┐
  people ─────▶ │  AnimeSubs desktop   │  Tauri + Vue (src-tauri/, src/)
                │  (client only)       │
                └──────────┬───────────┘
                           │ HTTP/JSON on 127.0.0.1, bearer token
  scripts,      ┌──────────▼───────────┐
  agents, ────▶ │  animesubs server    │  crates/server  (job queue, progress)
  media server  └──────────┬───────────┘
        │                  │ in-process
        │       ┌──────────▼───────────┐
        └─────▶ │  engine              │  crates/engine  (ex psyche-subtitle-toolkit)
  animesubs-cli └──────────────────────┘
  (standalone or --server)
```

The app installs and starts the server for people who only want the app.
The server and the CLI also run without the app.

## 2. Workspace layout

```
animesubs/
├── Cargo.toml          workspace root (profiles live here now)
├── Cargo.lock          single lockfile (was src-tauri/Cargo.lock)
├── crates/
│   ├── engine/         psyche-subtitle-toolkit, imported with history (git subtree)
│   ├── api/            animesubs-api: wire types, API version, error codes, home-dir files
│   ├── server/         animesubs-server: axum router, auth, job queue, shared operations
│   ├── client/         animesubs-client: typed client + find/start/stop a server
│   └── cli/            animesubs-cli: `serve`, `health`, `tracks`, `translate`, `job(s)`, `stop`
├── src-tauri/          desktop app (Rust side); depends on api + client only
├── src/                desktop app (Vue frontend)
└── docs/
```

Dependency rules:

| Crate | Depends on | Why |
|---|---|---|
| `engine` | — | Pure subtitle/media/provider logic. Keeps its MIT license and its own `LICENSE`. |
| `api` | serde, dirs | The contract. Tiny, no I/O beyond path helpers. |
| `server` | api, engine | Owns jobs. `ops` holds the operations so the CLI can run them in-process. |
| `client` | api, reqwest | No engine: the desktop app links it without the OCR/MNN native stack. |
| `cli` | api, client, server | One headless binary: standalone commands, `serve`, and server-client commands. |
| `src-tauri` | api, client | The app is a client of the server. Existing in-process commands stay until migrated (§6). |

`default-members` excludes `src-tauri`, so `cargo build` / `cargo test` at the
root work on machines without WebKitGTK (CI servers, a headless media
server). `cargo clippy -p animesubs` / `--workspace` include the app.

The CLI binary is `animesubs-cli` (not `animesubs`) because the Tauri binary
already uses that name in the shared `target/` directory.

## 3. How the app installs, starts, finds and stops the server

### Install

The installer ships one extra file: the headless `animesubs-cli` binary as a
Tauri sidecar (`bundle.externalBin`, placed next to the app executable), built
from the same commit as the app, so app and server versions always match.
Headless users install only `animesubs-cli` (release archive or
`cargo install --path crates/cli`).

Not done in the first slice: wiring `externalBin` makes `tauri build`/`cargo
build` of the app require the sidecar to exist, so it lands together with the
release-workflow change that builds it (§8).

### Find

Every server has a **home directory**: `$ANIMESUBS_HOME`, or by default
`<config dir>/com.enrell.animesubs`, which is exactly Tauri's
`app_config_dir()` for this app (the directory that already holds
`secrets.json`). App, CLI and server therefore agree without configuration.

| File | Content |
|---|---|
| `server.json` | `{service, version, api, url, pid, started_at_ms}`, written atomically after bind, removed on clean exit |
| `server-token` | 64 hex chars, created on first start with mode `0600` |
| `server.lock` | exclusive lock held while a server runs: one server per home |
| `logs/server.log` | stdout/stderr of a server spawned by a client |

`animesubs_client::connect(home)` reads `server.json` + token, calls
`GET /v1/health` (must say `service: animesubs`, `api: 1`) and then an
authenticated call. Any failure of the health call means "stale file, no
server".

### Start

`animesubs_client::ensure_server()` is what the app calls:

1. `connect(home)`; if it works, use that server (**borrowed**).
2. Otherwise locate the binary: `$ANIMESUBS_SERVER_BIN`, then `animesubs-cli`
   next to the running executable (bundled sidecar; also where Cargo puts it
   during development), then `PATH`.
3. Spawn `animesubs-cli serve --port 0 --home <home> --exit-on-stdin-eof`
   (no console window on Windows, output appended to `logs/server.log`), poll
   `connect()` every 100 ms for up to 15 s. If the child exits early, re-check
   once: another client may have won the race for the lock.
4. The returned connection **owns** the process.

Port 0 avoids conflicts; `server.json` tells clients the real port. A server
started by hand with `animesubs-cli serve` uses the fixed default port 9650
so scripts and remote tooling have a stable address.

In the slice the app does this once at startup, after the FFmpeg check and
without blocking the UI, and shows the result in the header's engine popover
("Local server: available/missing"). Nothing else depends on it yet.

### Stop

- **Owned** server, app closing normally: `POST /v1/shutdown`, wait up to 5 s,
  then kill (`RunEvent::Exit` in `src-tauri/src/lib.rs`).
- **Owned** server, app crashing or killed: the app holds the write end of
  the server's stdin; the OS closes it, the server sees EOF and shuts down
  cleanly (verified by killing the app with `SIGKILL`).
- **Borrowed** server (started by the user, a script, or another app
  instance): left running.
- Standalone: `animesubs-cli stop`, Ctrl-C or SIGTERM.

Later: when a newer app finds an older server, it asks it to shut down once
its queue is empty and starts the bundled one; an owned server could also
outlive the window while jobs run ("keep translating in the background").

### Lessons taken from Anything

Anything checks a fixed port, spawns the daemon if nothing answers and kills
it on quit, and its CORS is `*` with no token. AnimeSubs keeps the shape and
adds what Anything lacks: discovery file instead of port probing, a token,
no CORS, one-server-per-home lock, and stdin-EOF so a crashed app does not
leave an orphan. Anything's `platform-paths` lesson also applies: when app
and server run on different sides of Windows/WSL, paths must be translated at
the boundary (not needed while the bundled server runs on the same OS).

## 4. API contract (v1)

Base URL from `server.json`. JSON in and out. Every endpoint except
`/v1/health` needs `Authorization: Bearer <token>`. Types live in
`crates/api`; this table is a summary.

| Method & path | Body | Success |
|---|---|---|
| `GET /v1/health` | — | `200 {service, version, api}` |
| `POST /v1/tracks` | `{path}` | `200 {path, tracks: [{id, codec, format, language, name, default, forced}], selected}` |
| `POST /v1/jobs` | `JobSpec` (below) | `202 JobStatus` |
| `GET /v1/jobs` | — | `200 {jobs: [JobStatus]}` (oldest first) |
| `GET /v1/jobs/{id}` | — | `200 JobStatus` |
| `POST /v1/shutdown` | — | `202` |

```jsonc
// POST /v1/jobs
{
  "kind": "translate_subtitle",
  "input": "/media/show/ep01.ass",          // absolute, as seen by the server
  "output": "/media/show/ep01.pt-BR.ass",   // optional; default <stem>.<lang>.<ext>
  "target_language": "pt-BR",
  "provider": { "kind": "openai", "model": "gpt-4o-mini",
                "base_url": "http://localhost:1234", "api_key": "…" },
  "parallel": 2,                              // optional, 1..=16
  "overwrite": false
}
// JobStatus
{
  "id": "job-3f9c…", "kind": "translate_subtitle",
  "state": "queued|running|succeeded|failed",
  "phase": "queued|starting|parsing|translating|writing|done|failed",
  "progress": { "done": 120, "total": 310 },  // translated cues; null until known
  "input": "…", "output": "…", "target_language": "pt-BR",
  "created_at_ms": 0, "started_at_ms": 0, "finished_at_ms": 0,
  "error": { "code": "provider_error", "message": "…" }
}
```

Errors are always `{"error": {"code", "message"}}`. Codes: `unauthorized`
(401), `forbidden_host` / `forbidden_origin` (403), `bad_request` (400/422),
`not_found` (404), `already_exists` (409), `unsupported` (422),
`provider_error` (502), `missing_tool` (503), `engine_error`, `internal`.

Provider kinds are the engine's: `ollama`, `openai` (any OpenAI-compatible
endpoint: OpenAI, LM Studio, llama.cpp, NVIDIA NIM, MiniMax, Gemini's
OpenAI endpoint…), `opencode-zen`, `opencode-go`, `openrouter`, `anthropic`,
`deepl`, `google`, `gemini`. `base_url` is the API origin without `/v1`.
If `api_key` is omitted the server uses `ANIMESUBS_API_KEY`, the provider's
own variable (`OPENAI_API_KEY`, …) or the toolkit's `PSYCHE_SUBTITLE_API_KEY`.

Versioning: additive changes (new fields, endpoints, job kinds, error codes)
stay in v1; clients ignore unknown fields. Breaking changes get `/v2` and
`api: 2`, and the server serves both for at least one release.

Jobs run one at a time (each job already translates chunks concurrently) and
live in memory. Progress counts the cues returned by successful provider
responses, through a `Translator` decorator, so the engine did not have to
change; it can run ahead when the engine retries a rejected chunk and is
clamped to the total.

Planned, in order: `POST /v1/jobs/{id}/cancel`; `GET /v1/events` (SSE stream
of job updates, replacing polling in the app); engine-native progress events;
job kinds `extract_subtitle`, `embed_subtitle`, `translate_video` (§6);
`GET/PUT /v1/settings`; `GET /v1/backups`; persisted jobs under
`<home>/jobs/` with resume after restart.

## 5. Security

Threat model: other users on the machine, web pages in the user's browser
reaching `127.0.0.1` (directly or through DNS rebinding), and accidental
exposure on a network. Not in scope: malware running as the same user (it
could run the CLI anyway).

- **Loopback only.** `ServerConfig` refuses non-loopback addresses.
- **Token.** 32 random bytes from the OS, hex, in `server-token` with `0600`
  (the per-user `%APPDATA%` ACL on Windows); compared in constant time;
  `ANIMESUBS_TOKEN` overrides it for the server and clients.
- **No browser access.** Any request with an `Origin` header is rejected and
  there is no CORS, so pages cannot call the API even if they guessed the
  token. The app calls the server from Rust; the token never reaches the
  webview.
- **DNS rebinding.** `Host` must be `127.0.0.1`, `localhost` or `[::1]` with
  the server's port.
- **Secrets.** Provider keys in job requests are kept in memory only until the
  job ends, never returned in status, never logged (`ProviderConfig`'s
  `Debug` redacts them). The client ignores `HTTP(S)_PROXY` so the token never
  goes through a proxy. Engine errors already drop URLs that may contain keys.
- **Paths.** Absolute paths only; the server acts with the user's permissions
  (same trust as the CLI). Outputs are written next to the destination and
  renamed into place; existing files are not replaced unless `overwrite`.

Remote use (the media server `lain`): run `animesubs-cli serve` on that host
and call it locally there, or put it behind a reverse proxy with TLS and its
own token. A `--listen` option for non-loopback addresses is deliberately not
in v1.

## 6. What each repository contributes

| From | Keep | Notes |
|---|---|---|
| **animesubs** | The product: Vue UI and UX (`DESIGN.md`), queue, provider presets and model listing, translation styles, custom prompt, series context extraction (animedb/TMDB), OP/ED/karaoke/sign filtering, ASS style preservation, encoding detection (chardetng/encoding_rs), ffprobe track detection for every container, ffmpeg/mkvmerge embedding with duplicate removal and default flag, backups and restore, settings and `secrets.json` | Its in-process Rust commands move behind the server one flow at a time |
| **psyche-subtitle-toolkit** → `crates/engine` | ASS/SRT/WebVTT parsers and renderers, inline-tag protection, chunking by provider limits, concurrent chunks, retries with `Retry-After`/backoff/jitter, 9 providers (adds DeepL, Google, Anthropic, OpenCode), validated candidate MKV before replacing, PGS OCR with pinned SHA-256-verified PP-OCRv5 models, resume manifests, file locks | The engine of record; its CLI binary stays during the transition |
| **shinkai-translator** | ffprobe/ffmpeg media backend (non-MKV containers, `media.rs`), best-stream scoring (`score_stream`), language-tag inference, ASS cue classifier with policy and classification report (`ass_classifier.rs`, `domain.rs`), thinking-mode handling for reasoning models, OCR debug-at-timestamp command | Harvest into the engine with their tests; its OCR overlaps the toolkit's, keep the toolkit's; its TUI config editor is not needed |

Duplicates to resolve, engine wins unless noted: three ASS parsers (the
engine's; animesubs' filter rules become an engine classifier, checked
against shinkai's classifier tests); two OCR pipelines (toolkit); two
provider layers (engine providers; animesubs' presets map onto them); two
media backends (mkvmerge for MKV from the engine, ffprobe/ffmpeg for the rest
from shinkai and animesubs).

## 7. Migration path for existing AnimeSubs users

Rule: no release may lose a setting, a key or a backup, and every step can be
shipped and rolled back on its own.

1. **This PR — nothing changes for users.** Same identifier
   (`com.enrell.animesubs`), so the config dir, webview storage and
   `secrets.json` stay put. The app only reports the local server in the
   engine popover; release builds do not bundle it yet, so users see
   "missing" and every flow keeps running in-process.
2. **Bundle the sidecar** (§8). Still no flow depends on it.
3. **Engine parity**, one feature per PR, each with tests ported from
   animesubs: translation styles and custom prompt, OP/ED/karaoke/sign
   filtering, context profile in the prompt, encoding detection, ffprobe
   backend for non-MKV, and an embed that *adds* the translated track,
   keeps the original (`keepOriginalTrack`, default on) and writes the
   backup first. The toolkit's in-place track replacement stays available
   only as an explicit option: as is, it would drop the original track,
   which AnimeSubs users do not expect.
4. **Move flows behind the server**, one at a time, in this order: track
   listing → extraction → translation → embedding. The frontend keeps calling
   the same functions in `src/api/animesubs.ts`; only the Rust side changes,
   with the in-process path as fallback for one release.
5. **Remove** the in-process implementations from `src-tauri`.

Data compatibility:

- **Settings** (`localStorage` keys `animesubs-settings`,
  `animesubs-translation-options`) stay in the webview; the app sends what a
  job needs with the job, as it does with Tauri commands today. When headless
  defaults arrive (`/v1/settings`), the app imports them once, marks the
  import, and never deletes the old keys in the same release.
- **API keys** stay in `<config>/secrets.json`, same format
  (`{provider: key}`), read by the app and passed per job. The server may
  read the same file later for headless jobs, opt-in.
- **Backups** keep `.animesubs_backup/<stem>_<lang>_<timestamp>_track<N>.<ext>`
  next to the video; the engine's embed writes exactly this, and restore keeps
  listing old ones.
- **Resume manifests** of the toolkit (`.psyche-subtitle-toolkit-progress.json`)
  keep being read; a rename to `.animesubs-progress.json` must read both.
- **Toolkit CLI users**: `PSYCHE_SUBTITLE_API_KEY` is still honoured, and the
  engine's own `psyche-subtitle-toolkit` binary keeps building until
  `animesubs-cli` covers MKV translation.

## 8. Build, packaging and CI

- One workspace, one `target/` at the root: `build.yml` paths changed from
  `src-tauri/target/...` to `target/...`. Verify with
  `gh workflow run build.yml --ref <branch>` before the next release (the
  release job only runs on tags).
- CI also runs clippy and tests for the headless crates, and installs
  MKVToolNix so the real MKV round-trip tests run instead of skipping.
- Sidecar (next PR): build `animesubs-cli` per target, copy it to
  `src-tauri/binaries/animesubs-cli-<target-triple>`, add it to
  `bundle.externalBin`.
- Windows: `ocr-rs` links MNN built with the static CRT, so binaries that link
  the engine need `-C target-feature=+crt-static`. The engine's
  `.cargo/config.toml` only applies when building from `crates/engine`, so the
  sidecar build must pass it explicitly (the app does not link the engine and
  keeps the dynamic CRT).
- `ocr-rs` downloads a prebuilt MNN from a moving `dev` release at build time.
  Pin it (version + SHA-256) or vendor it, like the OCR models already are.
- The release profile uses `panic = "abort"`: a panic in a job would take the
  whole server down. Give the sidecar its own profile with unwinding.
- Licensing: the engine stays MIT (its `LICENSE` is kept); everything else
  follows the repository license. The repository's `LICENSE` is AGPL-3.0 while
  the README badge says GPL-3.0; one of them should be corrected.

## 9. What happens to the other repositories (recommendation)

The user decides; recommended:

- **psyche-subtitle-toolkit**: its history now lives in `crates/engine`.
  Make a last commit pointing the README to AnimeSubs and archive the repo
  (keep it private). The crate is on crates.io (0.4.1): either publish future
  versions from `crates/engine` (set `repository` to AnimeSubs) or leave 0.4.1
  as the last one; do not maintain two copies.
- **shinkai-translator**: harvest §6 into the engine with tests, then point
  its README to AnimeSubs and archive it (public, read-only, not deleted, so
  links and the crates.io 0.1.3 page still resolve).

## 10. First slice (this PR)

- `crates/engine`: the toolkit with its 42 commits, via `git subtree`. The
  imported history was filtered to remove links to a private repository, a
  machine-specific path and a stray agent worktree entry; code is unchanged.
- `animesubs-server`: health, tracks, translate-subtitle jobs with polling and
  progress, shutdown; auth as in §5.
- `animesubs-cli`: the same operations standalone (`tracks`, `translate`) and
  through the server (`--server`, `health`, `job`, `jobs`, `stop`, `serve`).
- `animesubs-client` + app: the app starts or connects to the server and shows
  it in the engine popover; owned servers stop with the app.
- Tests: engine suite as imported; API, auth, jobs and failure paths against a
  real server on an ephemeral port; client discovery and lifecycle; CLI
  end-to-end (standalone, `serve` + client commands, the app's start/own/stop
  flow, and parent death). LLMs are mocked with an OpenAI-compatible
  wiremock that "translates" by prefixing lines. MKV tests skip without
  MKVToolNix; OCR model inference is not exercised (no model download).

Translating subtitles *inside videos* through the server is intentionally
not in the slice: doing it with the toolkit's in-place replacement would
break the AnimeSubs contract (keep the original, back it up). It comes with
step 3 of §7.

## 11. Open questions

1. Rename the engine crate to `animesubs-engine`, or keep
   `psyche-subtitle-toolkit` because of the crates.io name?
2. Should an owned server keep running after the window closes while jobs
   are in progress (tray icon), or always stop with the app?
3. Fixed default port 9650 for manual `serve`: fine, or prefer always-random?
4. Which license is correct for the repository, AGPL-3.0 (`LICENSE`) or
   GPL-3.0 (README)?
5. Should headless jobs be able to read `secrets.json`, or must keys always
   come with the request or the environment?
