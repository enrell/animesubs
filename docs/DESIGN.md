# AnimeSubs — UI/UX System Design

Version 1.0 · 2026-07-28

This document is the source of truth for the AnimeSubs interface redesign. It
defines the product philosophy, the information architecture, the queue state
machine, the visual design system (themes, tokens, typography, artwork), and
the copy guidelines. Implementation details follow each decision.

---

## 1. Product philosophy

> **A production line the user can trust — not a control panel for a machine.**

The old interface exposed the machinery: technical jargon ("translation
protocol", "signal controls", "packets"), all technical details always visible,
and a monochrome brutalist skin where every element competed at the same
visual volume. The redesign inverts this: the user's goal (translated
subtitles, embedded, originals safe) drives every layout decision, and
technical depth is available on demand instead of being mandatory reading.

### 1.1 Audience

Two audiences share one interface:

| | Everyday user | Power user |
|---|---|---|
| Profile | Internet-literate anime fan. Has used ChatGPT, knows what an API key is, understands that tokens cost money | Fansubber / tinkerer. Wants logs, per-file control, prompt iteration |
| Wants | Zero decisions beyond target language. Confidence before spending | Visibility and override granularity |
| Fears | Wasting API money, corrupting original files | Not knowing *why* something failed |

The difference between them is **decision burden, not vocabulary**. We never
hide technical information; we change its default visibility. There is no
"simple mode / advanced mode" toggle — dual modes fragment the product and
betray both audiences.

### 1.2 Design principles

1. **One interface, three depths.** Surface (always visible) → one gesture away
   (expand, hover, popover) → the console (log drawer). Every UI decision is
   answered by: *what is the default visibility of this element?*
2. **Exactly one obvious action at any moment.** The footer bar computes a
   single primary action from the queue state.
3. **Honest progress.** Progress bars are driven by translated line counts,
   never by time guesses. Phases without measurable progress show a label, not
   a fake percentage.
4. **Failures are first-class.** A failed episode is more important than a
   succeeded one. Retry is granular (per episode), and retry only appears when
   retrying makes sense.
5. **Safety is communicated, not just implemented.** Automatic backup exists
   today as a technical feature; the redesign surfaces it as a guarantee
   ("originals untouched").
6. **Human language everywhere.** Technical terms may annotate, never lead.

---

## 2. Information architecture

The app is a **queue**, matching the mental model the codebase already has
(`selectedFiles`). Reference: HandBrake — deeply technical underneath,
mainstream on the surface, because the queue is the center and presets hide
complexity.

```
┌──────────────────────────────────────────────────────────┐
│ HEADER  brand · engine chip · theme · language · settings │
├──────────────────────────────────────────────────────────┤
│                                                          │
│   EMPTY STATE (no files)            or      QUEUE LIST   │
│   - drop target + guided setup      │  ┌ side panel ───┐ │
│   - onboarding steps                │  │ target lang   │ │
│                                     │  │ track picker  │ │
│                                     │  │ embed toggle  │ │
│                                     │  │ context       │ │
│                                     │  └───────────────┘ │
├──────────────────────────────────────────────────────────┤
│ LOG DRAWER (collapsible, activity console)               │
├──────────────────────────────────────────────────────────┤
│ FOOTER  aggregate counts · estimate · ONE primary action │
└──────────────────────────────────────────────────────────┘
```

### 2.1 Surface (depth 0)

- Queue rows in collapsed form (see §3).
- Side panel: target language, subtitle track (human label), embed toggle,
  context toggle. Nothing else.
- Footer aggregate with the single primary action.

### 2.2 One gesture away (depth 1)

- **Row expansion**: any queue row expands into the technical inspector —
  tracks with codec/index, backups, extract/restore actions. This replaces the
  old always-visible track list, which was the main source of visual density.
- **Engine chip popover**: the header shows one compact chip
  ("Engine: Gemini ✓"). Clicking reveals endpoint, model, FFmpeg version.
- **Advanced options collapse** in the side panel (custom prompt, context
  extraction).

### 2.3 The console (depth 2)

- **Log drawer**: collapsible bottom panel with timestamped activity entries —
  backend status messages, per-file errors, validation failures. Everyday
  users never open it; power users live in it. The same failure event renders
  as a human sentence on the row and as the raw error in the drawer.

---

## 3. Queue item state machine

Six persistent states plus one transient. New features must not add states;
if a feature seems to need an eighth state, question the feature.

```
                 add file
                    │
                    ▼
              ┌───────────┐
              │  PROBING  │  transient — reading metadata via ffprobe
              └─────┬─────┘
        ┌───────────┼────────────────┐
        ▼           ▼                ▼
  ┌──────────┐ ┌─────────┐   ┌──────────────┐
  │  READY   │ │ NO-SUBS │   │ FAILED(probe)│  unreadable/corrupt file
  └────┬─────┘ └─────────┘   └──────────────┘
       │ "Translate" pressed
       ▼
  ┌──────────┐   its turn    ┌──────────────┐
  │  QUEUED  │ ────────────▶ │ TRANSLATING  │
  └────┬─────┘               └──┬────┬────┬─┘
       │ cancel                 │    │    │
       ▼                success │    │    │ cancel
   back to READY                ▼    │    ▼
                              DONE   │  CANCELLED
                                     ▼
                                  FAILED

  FAILED / CANCELLED / DONE ── retry / re-translate ──▶ QUEUED
  any state except TRANSLATING ── remove ──▶ leaves queue
```

### 3.1 State presentation

Every collapsed row has the same skeleton:
`status icon · episode name · contextual summary · actions`.

| State | Icon/tone | Secondary text | Primary action | Secondary actions |
|---|---|---|---|---|
| `PROBING` | spinner, neutral | "Reading file…" | — | remove |
| `READY` | dot, success tone | "English subtitle (default)" + muted `track 2 · subrip` | — (waits for batch) | expand, remove |
| `QUEUED` | position number, muted | "In queue — position 3" | cancel → READY | — |
| `TRANSLATING` | inline progress bar, accent | phase + "342/508 lines" | cancel | — |
| `DONE` | check, success | "EN → pt-BR · model · output folder" | open folder | re-translate (menu), remove |
| `FAILED` | cross, error tone | human error message | **retry** | view log, remove |
| `CANCELLED` | hollow circle, muted | "Cancelled by you" | retry | remove |
| `NO-SUBS` | warning triangle, amber | "No embedded subtitles" | — | expand (why), remove |

Rules:

- **At most two visible actions per row**, plus an overflow menu. The correct
  action appears in the correct state.
- **Row expansion is state-independent.** Depth is always available; the state
  only changes the surface.
- Semantic tones only: success (green), work-in-progress (theme accent),
  error (red), attention (amber), neutral (muted). The accent never collides
  with red, so destructive and primary actions are never confused.

### 3.2 Phases inside TRANSLATING

The backend pipeline already runs in phases; the row mirrors them:

1. **Extracting subtitle** — label only, no percentage (fast, unpredictable).
2. **Translating** — real percentage from `lines_translated / total_lines`
   (backend `translation-progress` events). This is the only honest bar.
3. **Writing/embedding** — label only.

### 3.3 Error taxonomy → row action

The `FAILED` state's offered action depends on the error category:

| Category | Example | Row message | Offered action |
|---|---|---|---|
| Network / rate limit | timeout, 429 | "The API didn't respond in time" | Retry |
| Authentication | 401, bad key | "API key was rejected" | **Open settings** (not retry) |
| Model | unknown model | "Model not found on provider" | Open settings |
| Embed / ffmpeg | write failure | "Translated, but failed to embed" | Retry + view log |
| File | probe failure | "Couldn't read this file" | Remove (no retry) |

Rule: retry is only offered when retrying can plausibly succeed. Configuration
errors offer the fix path instead.

### 3.4 Done-with-warnings

If a file completes with a subset of lines left untranslated (fallback to
source language), the row is `DONE` with an amber badge:
"Completed · 12 lines kept in the original language". A 97% translated
episode is watchable; failing the whole episode over 2% of lines punishes the
user for a partial problem.

### 3.5 Structural rules

- **Sequential by default.** One `TRANSLATING` at a time: predictable queue
  positions, gentle on rate limits. Parallelism is a future advanced setting
  and does not change the state model.
- **Config snapshot at enqueue.** When a file enters `QUEUED`, it captures the
  config it will use (languages, model, prompt). The `DONE` row displays the
  snapshot, so mid-batch settings changes never create ambiguity.
  **Exception:** retry uses the *current* config — the most common reason to
  retry is having fixed the config.
- **NO-SUBS is excluded, not blocking.** The primary button counts only
  `READY` files ("Translate 8 episodes" even with 10 in queue).
- **Config blockers are not row states.** Missing model / API key / FFmpeg
  disables the primary button with a specific, clickable reason next to it.
- **Dedup by path.** Adding the same file twice is a silent no-op that
  highlights the existing row.
- **Sample runs are transient.** The sample flow borrows the first `READY`
  row ("Generating sample…") and returns it to `READY` afterward. It never
  enters the state machine.

### 3.6 Footer aggregate

The footer is a pure function of row states — it holds no state of its own:

```
8 ready · 1 translating (Ep 03, 47%) · 2 failed     [Retry 2 failures]
```

Primary button priority:

1. Failures exist, nothing running → **"Retry N failures"**
2. Ready files exist, nothing running → "Translate N episodes"
3. Batch running → "Cancel batch" (secondary style)
4. Nothing ready → disabled with specific reason ("Add episodes" /
   "Configure a provider →")

---

## 4. Visual design system

### 4.1 Anti-goals

Explicitly banned, because they read as generic template output:

- No indigo/violet/blue palettes. No purple-blue gradient meshes.
- No Tailwind tutorial aesthetics (`bg-gray-900`, indigo buttons, blurred
  gradient blobs, `ring-1 ring-white/10` cards).
- No glassmorphism, no neon cyberpunk, no generic "AI startup" look.
- No monochrome brutalism either (the old skin): pure black on pure white,
  2px borders everywhere, `border-radius: 0`, all-caps everything.

### 4.2 Creative direction: "evening at the festival"

Anime-inspired without using any copyrighted artwork. The mood is a warm
summer festival (*matsuri*) at dusk: paper, ink, lantern light, vermillion
torii gates, sakura petals. All backgrounds are **original hand-crafted SVG
illustrations** shipped with the app — legally clean, resolution-independent,
and a few KB each.

The accent hue family is **warm**: vermillion red, lantern gold, sakura pink,
matcha green. Cool hues appear only inside illustrations (night skies), never
as UI chrome.

### 4.3 Theme system

Four named themes, two dark and two light, defined as token sets. Theme is
user-selectable and persisted; default follows `prefers-color-scheme`.

| Theme | Mode | Mood | Accent |
|---|---|---|---|
| **Kitsune** (default dark) | dark | Warm charcoal night, ember light | Vermillion `#E4573D` |
| **Matsuri** | dark | Festival night, lantern glow | Lantern gold `#E39A2D` |
| **Hanami** (default light) | light | Washi paper, sakura petals | Sakura `#D6406B` |
| **Matcha** | light | Tea house, bamboo | Matcha `#5A7A2E` |

Each theme defines the full token set below (no theme may leave a token
undefined), plus an SVG background illustration and its overlay opacity.

### 4.4 Design tokens

```css
--bg-base        /* app background (under illustration) */
--bg-surface     /* cards, panels — translucent, illustration bleeds through */
--bg-elevated    /* popovers, drawers, modals */
--bg-inset       /* inputs, inset areas */

--text-primary
--text-secondary
--text-muted

--border-subtle  /* hairlines between rows */
--border-strong  /* panel edges */

--accent         /* primary action, active states, WIP progress */
--accent-hover
--accent-contrast /* text/icons on top of accent */

--success
--warning
--danger

--focus-ring
--radius-sm / --radius-md / --radius-lg
--shadow-pop
```

Surfaces are slightly translucent (`color-mix` over `--bg-base`) so the
theme illustration reads through panels — the artwork is atmosphere, not a
framed picture.

### 4.5 Background illustrations

One SVG scene per theme, rendered behind the app at low opacity with a
themed overlay gradient for legibility:

- **Kitsune**: night sky with stars, a vermillion torii silhouette low in the
  corner, warm moon glow.
- **Matsuri**: hanging paper lantern glows (soft warm bokeh), pennant flags
  silhouette along the top.
- **Hanami**: a sakura branch entering from the top corner, scattered
  falling petals over washi-paper texture.
- **Matcha**: bamboo stalk silhouettes, soft dappled light.

Rules for artwork: silhouettes and flat shapes only (cel-shading logic), no
raster images, no recognizable characters or franchise imagery, contrast kept
low enough that text over it always passes §4.8.

### 4.6 Typography

- **UI text**: system stack (`Inter`, `Segoe UI`, system-ui). 14px base.
- **Display**: same family at 700–800 weight for the brand wordmark — no
  decorative fonts.
- **Mono** (`ui-monospace`): only where mono means something — file paths,
  track annotations, log drawer, counters.
- Sentence case everywhere. All-caps is banned outside the wordmark;
  sustained capitals hurt reading speed.

### 4.7 Copy guidelines

Banned vocabulary (developer jargon leaking into UI): *signal, packet,
protocol, sync, runtime, initiate, execute, payload, entity*.

| Old | New |
|---|---|
| Translation protocol | Translation |
| Initiate translation | Translate |
| N packets attached | N episodes loaded |
| Sync 47% | Translating — 342/508 lines |
| Awaiting packet response | Waiting for the API… |
| Advanced signal controls | Advanced options |
| Route through mkvmerge | (moved to settings; the app decides) |

Error messages are written as cause + next step
("API key was rejected — check it in Settings"), never as raw exceptions.

### 4.8 Accessibility

- Contrast: text ≥ 4.5:1, large text ≥ 3:1 against the *effective* background
  (illustration included).
- Focus: visible 2px focus ring (`--focus-ring`) on every interactive element.
- State is never color-only: every status has an icon + text.
- Minimum 11px for annotations; 14px for reading text.

---

## 5. Component inventory

| Component | Purpose | Notes |
|---|---|---|
| `AppHeader` | brand, engine chip (popover), theme picker, language, settings | drag region for the frameless window |
| `EmptyState` | drop target + guided setup when queue is empty | doubles as onboarding: surfaces missing provider/FFmpeg as steps |
| `QueueRow` | one episode: state icon, summary, actions, expandable inspector | implements §3 |
| `SidePanel` | target language, track picker, embed toggle, context, advanced | only depth-0 controls visible by default |
| `LogDrawer` | timestamped activity console | collapsible; badge count of errors |
| `FooterBar` | aggregate counts + estimate + single primary action | pure function of queue state (§3.6) |
| `SettingsModal` | provider/connection, translation, output | gains "Test connection" |
| `ContextPanel` | story context profile | framed as "Series context — consistent names & terms" |

---

## 6. Interaction details

- **Drag & drop** anywhere on the window adds files; the whole queue area is
  the drop target, highlighted while dragging.
- **Retry failures**: the footer action re-runs the job with only failed
  paths, using current settings (see §3.5).
- **Per-file retry** from the row menu runs a single-file job immediately when
  idle.
- **Cancel** requests stop after the current file completes (the backend has
  no mid-file cancellation; the UI says "will stop after the current
  episode").
- **Keyboard**: `Enter` starts the batch when idle, `Delete` removes the
  focused row, `Ctrl/Cmd+,` opens settings, `Ctrl/Cmd+L` toggles the log
  drawer.

---

## 7. Implementation notes (current codebase)

- Per-row state is derived in a composable (`useQueueStates`) from:
  `file.loading` → PROBING; `file.error` → FAILED(probe);
  `videoInfo.subtitle_tracks` → READY / NO-SUBS; job `currentFileIndex` →
  QUEUED/TRANSLATING; `TranslationJobResult.outputs` (by `videoPath`) → DONE;
  batch files missing from outputs → FAILED with the reason matched from
  `failures[]` strings ("Error in {filename}: {reason}").
- Progress events already exist (`translation-job-progress`,
  `translation-progress`); the row's inline bar reuses the per-file chunk
  ratio.
- Retry granularity needs no backend change: `start_translation_job` accepts
  any subset of paths.
- The activity log is a frontend store fed by the same events plus validation
  results; the drawer renders it newest-first with level filtering.

---

## 8. Explicitly out of scope (for now)

- Queue persistence across sessions (state model is serializable; adding it
  later is a storage task, not a redesign).
- Mid-file pause/resume with partial-translation caching.
- Sample translation flow (side-by-side preview before the batch) — designed
  conceptually, requires a new backend command for partial translation.
- Manual queue reordering (drag to reorder) — cosmetic over the state model.
- Translation presets (prompt + context + format bundles).
