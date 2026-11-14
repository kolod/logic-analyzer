# Research: UI Shell and Signal View

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-11-14

Each section records a decision, why, and what else was considered. Versions and licences below
were read from the crates' manifests on 2026-11-14 (`cargo info`, `cargo metadata` for Windows
and Linux targets); the facts about text support come from the libraries' documentation and sources
through Context7.

## R1. UI toolkit

**Decision**: **iced 0.14** (MIT) with the `wgpu` renderer and the `tiny-skia` software fallback, and
its `canvas` widget for the signal view.

**Measured facts** (probe projects with the candidate and `rfd`, resolved for both target platforms):

| | iced 0.14 | egui/eframe 0.37 | Slint 1.18 |
|---|---|---|---|
| Licence | MIT | MIT OR Apache-2.0 | GPL-3.0-only OR royalty-free OR commercial |
| Packages (Windows / Linux) | 174 / 250 | 200 / 299 | 313 / 434 |
| Highest `rust-version` in the tree | 1.92 (`rfd`) | 1.95 (`ecolor`) | 1.92 |
| Licences outside today's `deny.toml` allow-list | `BSL-1.0` (two Windows-only crates) | `BSL-1.0` (same two) | the Slint licence expression, `BSL-1.0`, `Unlicense` |
| Text shaping, fallback fonts, bidirectional text | cosmic-text: shaping, bidi, system font fallback | **no bidi** (bidi controls are dropped, TODO in epaint), fonts must be installed by hand or via a system-font add-on | own text engine, bidi supported |
| IME (Japanese input) | input-method events (`Opened`, `Preedit`, `Commit`, `Closed`) | supported, historically rough edges | supported |
| Custom drawing | `canvas::Program` with cached geometry; `shader` widget for raw wgpu | `Painter` shapes each frame | no vector canvas; draw into an image or import a wgpu texture |
| Accessibility | none yet | AccessKit | AccessKit |
| Architecture | Elm style: state, messages, `update`, `view` | immediate mode | declarative `.slint` language plus Rust glue |
| Translation | free (we supply strings) | free | built around gettext `@tr`, a second path next to Fluent |

**Rationale**:

- The constitution's hardest UI rules are text rules (Japanese, long German, user text that may be
  right-to-left, Principle VII and FR-025/026). iced's cosmic-text handles shaping, fallback and
  bidirectional text; egui does not handle bidirectional text at all, which would make FR-025's "mixed
  direction user text must not disturb the layout" untestable on the real display.
- The Elm architecture matches FR-033 and the layering of Principle IV: the whole shell is plain data
  (`State`), plain commands (`Message`) and a function from one to the other, so almost everything
  can be tested without a window.
- The signal view needs a custom renderer. iced's canvas takes ready-made geometry, caches it between
  frames and redraws only on demand; the render plan computed by the core is O(W) primitives, which
  is well inside what the tessellator handles. The `shader` widget (raw wgpu) is the reserve if the
  canvas cannot meet SC-002 (see R4).
- Smallest dependency tree and a permissive licence. Slint's GPL option is compatible with a GPL-3.0
  project, but its second language, its gettext-based translations and its lack of a vector canvas
  all add work against Principle V.
- Cost accepted: iced is pre-1.0 and has no accessibility support. Both are recorded as risks (R12)
  and the iced-specific code is confined to the `la-app` crate so a later switch costs only that
  crate.

**Alternatives considered**: egui (rejected for bidi, see above; its AccessKit support is its main
advantage and is the reason this stays an explicit trade-off); Slint (rejected for the second
language and canvas); GTK 4 / Qt bindings (heavy native dependencies, differing text and dialog
behaviour on Windows, not pure Rust); Tauri or a web view (a second runtime and language, and
rendering through the DOM rather than a Rust-indexed canvas); Floem, Xilem, Dioxus-native (younger,
smaller communities).

**Reconsideration triggers**: accessibility becoming a requirement; an iced release that breaks the
canvas API with no migration path; the canvas missing the SC-002 budget even after the shader
fallback.

## R2. Where the logic lives: `la-host` and `la-app`

**Decision**: two new crates.

- `la-host` (UI-free, MSRV 1.85 like the other core crates): everything the shell *does*. It owns the
  shell state, executes commands, runs the device registry and the background operations, turns
  capture snapshots into frame plans, keeps presentation, markers, messages and settings, and maps
  errors to localized messages. It is the implementation of the host API contract
  ([contracts/host-api.md](contracts/host-api.md)).
- `la-app` (binary `logic-analyzer`, iced, higher MSRV): draws a frame plan, turns input into host
  commands, hosts native dialogs. No domain logic.

**Rationale**: Principle IV (UI-free domain logic, a contract between UI and core, headless testing)
and FR-033. Putting the "view model" in `la-host` is what makes SC-003, SC-005, SC-006, SC-007 and
SC-011 automatable on every CI runner without a display. The contract is then a real Rust API plus a
document, not a loose convention.

**Alternatives considered**: putting the logic into the iced application (untestable without a
window, ties the domain behaviour to the toolkit); splitting into more crates (view maths, settings,
messages) now (premature; modules inside `la-host` can be split later if one grows).

## R3. Host API contract: shape and location

**Decision**: the contract is **commands in, state and events out**, versioned `1.0`:

- `Command`: a plain enum (start capture, stop, zoom, pan, place marker, save, change language …).
- `Shell::apply(command) -> Outcome`: executes it synchronously for the instant parts and starts a
  background operation for the slow parts.
- `Shell::poll() -> Changes`: drains events of background operations (capture progress, save/load
  progress, device hot-plug) and reports which parts of the state changed.
- `Shell::state() -> &State` and `Shell::frame_plan(size) -> FramePlan`: read-only queries.
- The contract document lives at `docs/host-api.md` (written during implementation from
  [contracts/host-api.md](contracts/host-api.md), which is its design source), with
  `HOST_API_VERSION` in the crate and a test that fails if the document and the constant disagree.
  The constitution's `TODO(HOST_API_CONTRACT)` is satisfied by this location and rule; the
  constitution text itself is amended only with the user's approval (see Complexity Tracking).

**Rationale**: a synchronous, data-only surface is simple to document, to test and to call from any
future shell. Versioning is by constant plus document so a change cannot land without the doc
(Principle IV: contract changes merge together with the implementation).

**Alternatives considered**: a message-passing server inside the process with channels (more moving
parts, no benefit while there is one UI); a C ABI (needed by no consumer); async API (iced already
wraps blocking work in tasks, and the long operations are already background operations).

## R4. Rendering the signal view

**Decision**: a two-stage pipeline.

1. **Frame plan** (in `la-host`, toolkit-free): for the visible window and the pixel width W, call
   `Snapshot::viewport(channel, start, end, W)` for each visible channel (cost bounded by
   Principle VIII) and fold the per-pixel summaries into a short list of **segments** per channel:
   `Level { from, to, high }`, `Edge { x, rising }` (with the exact sub-pixel x from the edge
   position), `Dense { from, to, count }`, `Gap { from, to }`, `Beyond`. Adjacent equal columns are
   merged, so a quiet channel is one segment however long the capture is. Plus the axis ticks and the
   marker positions. All integer or exact arithmetic with checked operations; `f64` appears only in
   the final screen coordinate of a primitive.
2. **Draw** (in `la-app`): a `canvas::Program` converts segments to strokes and fills. Static layers
   (background, grid, channel labels) and the waveform layer are cached by `canvas::Cache`
   and invalidated by a *version key* `(snapshot version, window, W, presentation version, theme,
   scale)`; cursor, markers and hover are drawn in a cheap overlay layer that does not invalidate the
   waveform. The frame plan is computed only when the key changes.

**Dense activity**: drawn as a filled, hatched column band in the channel colour plus a thin outline
(shape differs from both "no transition" and "one transition", so it does not rely on colour,
FR-013). The count is available in the hover text.

**Budget and fallback**: SC-002 asks for p95 < 16 ms and worst < 50 ms on 100 M samples ×
16 channels. The plan stage costs W×16 viewport columns (a few probes each); the draw stage costs at
most a few thousand primitives per channel. A criterion benchmark (`frame_plan`) with a recorded
baseline measures the first stage in CI; a manual/automated frame-time log in the app measures the
second. If the canvas path misses the budget, the order of fallbacks is: (1) coarser tessellation
(one polyline per channel instead of per segment), (2) the `shader` widget drawing the same
segments from a vertex buffer, (3) computing the frame plan on a latest-wins background worker
while showing the previous frame. None of these changes the contract.

**Alternatives considered**: drawing straight from `viewport` results in the UI (duplicates the
folding in every shell and cannot be unit-tested without a window); pre-rendering an image per frame
(blurry on scale changes, large uploads); reading samples (violates Principle VIII).

## R5. Fonts: Japanese, Cyrillic and long German

**Decision**: bundle a small base font family and rely on system fonts for everything else.

- Embed an OFL-licensed UI font subset covering Latin, Latin Extended, Greek and Cyrillic, and an
  OFL-licensed Japanese font subset limited to the characters that occur in the application's own
  `ja` resources plus common kana and the JIS level 1 kanji (so the UI itself renders on a machine
  without CJK fonts, which a minimal Debian install is). The subsets are produced by a documented
  script and committed with their licence files (SIL OFL 1.1; compatible with GPL-3.0 distribution
  as separate font data, recorded in the third-party notices).
- iced's cosmic-text falls back to system fonts for anything not covered (user-typed CJK, emoji,
  other scripts). Where no font covers a character the view shows the replacement glyph, and the
  application tells the user once per session which package to install (`fonts-noto-cjk` on Debian)
  in the documentation, not in the UI text.
- Layout never assumes widths: controls size from measured text, long strings wrap, and fixed-width
  elements (toolbar buttons) are icon-plus-label with the label allowed to wrap or to move into a
  tooltip. A "text fit" check (R10) runs the German and Japanese resources through every screen.

**Rationale**: Principle VII requires correct Japanese and un-truncated German on both OSes; a
guaranteed font for the UI strings removes the dependence on what the machine has, while system
fallback keeps the download small and covers user text.

**Alternatives considered**: system fonts only (a minimal Debian shows boxes, breaking a constitutional
rule); a complete CJK font bundled (about 16 MB, most of it unused); no CJK bundling but a hard
package dependency (not enforceable for a portable binary).

## R6. Menus, dialogs and shortcuts

**Decision**: an in-window menu bar built from iced widgets (iced has no native menu support), a
toolbar, and native file dialogs through `rfd` (MIT; async, run as iced tasks). Every menu entry,
toolbar button and shortcut is one `Command` (spec Key Entities); the shortcut table is data in
`la-host` (`contracts/shell-layout.md`) so the keyboard map is testable and the menu shows the
accelerator in the user's language.

**Rationale**: one place where actions are defined keeps FR-036 (everything keyboard operable)
checkable: a test walks the table and confirms every command has a reachable binding or menu path.

**Alternatives considered**: native menus through a separate crate (extra platform code and a second
text path, no toolkit integration); no menu bar, toolbar only (hides rarely used commands from
keyboard-only users).

## R7. Saving presentation and markers: the `VIEW` section

**Decision**: a new optional session section `VIEW` carries channel presentation, markers and the
last view window, written by the shell and ignored by older readers (they preserve it, as the format
requires). The file's minor version becomes **1** (documented evolution rule: a new minor adds only
optional sections). Payload layout in [contracts/view-section.md](contracts/view-section.md); it uses
the same canonical varint style as `CAPT` so the file stays deterministic. An invalid or oversize
`VIEW` payload never blocks opening: the capture loads with default presentation and a warning
message (the capture data is more valuable than the cosmetics, Principle I).

**Rationale**: FR-018/020 require the choices to travel with the file; the format already reserves
"view state" as a later optional section and the spec of the feature that adds it defines it.
Settings that belong to the user rather than to the capture (language, theme, memory limit,
geometry, recent files) stay in a separate settings file (R8).

**Alternatives considered**: a sidecar file next to the session (can be lost when the file is
copied, breaking "saved and reopened"); storing presentation in the `META` section (changes a
critical section, would need a major version).

## R8. Settings file

**Decision**: `settings.json` in the per-user configuration directory (`directories` crate: MIT/
Apache-2.0, small), written atomically by `la-host` with the same temp-file/rename discipline as
sessions (via the `la-session` file-system seam), read with serde. Version field; unknown fields
preserved; any read problem (missing, unreadable, not UTF-8, invalid, wrong version, too large)
yields defaults plus one `host.settings_*` message and never prevents startup. Values are validated
and clamped (memory limit, UI scale 75–200 %, recent files ≤ 10, window geometry kept on a visible
monitor).

**Rationale**: human-readable and editable by a user who needs to repair it; serde/serde_json are the
standard tools and are already needed by nothing else, so they are justified here only (Principle V
table in the plan).

**Alternatives considered**: a TOML file (one more parser with no benefit); a hand-written format
(more code to harden); the Windows registry / GSettings (two platform paths, not portable).

## R9. Background work, events and panics

**Decision**:

- Capture, save and load already run on their own threads with progress, cancel and typed
  outcomes (features 003, 004). `la-host` holds the handles; `Shell::poll()` is called by the UI on a
  timer **only while something runs** (50 ms) and at 2 Hz otherwise (device hot-plug), so an idle
  application neither redraws nor burns CPU (FR-015, SC-010).
- `Shell::apply` runs each command inside a panic boundary: state is mutated on a working copy and
  committed only if the command returns, so a panic leaves the previous state intact, is logged with
  the command name, and becomes a `host.internal` message (FR-029). `frame_plan` has its own
  boundary and returns `FramePlan::cannot_draw` on failure so the view shows the "cannot draw"
  state for that region (US6 scenario 5).
- A panic in an application-level iced callback is outside what the host can wrap; `la-app` keeps
  its own code to mapping and drawing only, with no `unwrap`/indexing (the workspace lints apply to
  it as to every crate).
- The unsaved-data guard is a host state machine (`Dirty` / `Running` / `Clean`): the close,
  open, new-capture and quit commands are *deferred* by the host until the user's answer arrives
  (`Outcome::NeedsDecision`), so the rule cannot be bypassed by a UI that forgets to ask (SC-011).

**Alternatives considered**: running the host on its own thread with a channel API (adds latency and
a second state copy; no benefit for one UI); async runtime inside the host (not needed, the
operations are threads).

## R10. Testing without a display

**Decision**:

- **Host (automated, both OSes, no display)**: unit and scenario tests for zoom/pan arithmetic with
  hand-written boundary tables; frame plan versus a brute-force reference built from the transitions
  (including the glitch-at-every-zoom sweep, SC-003); marker maths; presentation and `VIEW` codec
  round-trips and a mutation fuzzer in the style of feature 003; settings damage cases; message
  grouping and the "every error code" sweep (SC-006) using the catalogue of `la-i18n`; the 100,000
  action random walk with scripted device failures (SC-005); save interruptions through the session
  crate's `FaultFs` at 100+ points (SC-007); close/open/new-capture guard scenarios (SC-011);
  frame-plan benchmark with baseline (SC-002, first stage).
- **App (automated where possible)**: a pure `view` smoke test per screen in each language through
  the toolkit's headless test support if the feature spike (task in `tasks.md`) confirms it works
  on both CI runners; this also drives the **text fit check** (SC-008): render each screen at the
  minimum window size with the longest German and Japanese strings and compare measured text bounds
  with container bounds. If headless layout measurement is not feasible, the fallback is a resource
  lint (every UI string must fit a stated width budget per element class, enforced in the parity
  test) plus the manual checklist in the quickstart. The plan says which is used after the spike.
- **Manual (recorded in `verification.md`)**: windowed frame-time and responsiveness measurements,
  IME input of Japanese channel names, high-contrast and scale-factor checks, on Windows and Debian.

**Rationale**: Principle III; only the last category truly needs a window, and the plan keeps it
small and explicit.

## R11. Time, zoom and precision rules

**Decision**:

- Positions are **sample indices** (`u64`) and the time of a position is the exact rational
  `position / sample_rate`; durations are formatted from that rational at a precision that
  never exceeds one sample period (FR-017), through the localizer's duration/frequency
  formatters. The view's window is `(start, length)` in samples with `length ≥ ceil(W / 64)`
  (at most 64 pixels per sample), at most the capture length, with `start + length ≤ length`
  of the capture; all arithmetic is checked or `u128`-widened.
- Zoom by factors 1/2 and 2 per wheel notch (smooth factor from the amount of scroll), anchored on
  the pointer position's sample (kept at the same pixel); pan by pixels; "fit all", "go to start/end",
  "zoom to range".
- Axis: tick step chosen from {1, 2, 5} × 10^k seconds so ticks are at least 80 px apart; labels
  use the same step to decide how many digits to show (no `0.30000000000000004`).
- The timeline is relative to the capture start; the wall-clock start time appears only in the
  capture's information panel, formatted by the localizer with the system UTC offset (feature 005).

## R12. Risks and limits recorded

- **Accessibility**: iced offers no screen-reader support in 0.14. Keyboard operation, scale factor
  and an interface-scale setting are provided; the gap is documented, and the reconsideration trigger
  in R1 applies.
- **GPU availability**: machines (and CI runners) without a usable GPU fall back to the tiny-skia
  software renderer; frame times there are not part of the budget.
- **MSRV**: the UI dependency tree needs Rust 1.92 or newer; the toolchain pin is already 1.100.0.
  `la-app` declares its own `rust-version`; the 1.85 job covers every other crate (R13).
- **Linux build dependencies**: the Debian jobs run in a bare `debian:trixie` container. Which
  system packages (if any) the iced/winit/wgpu/rfd tree needs is settled by the first CI run of the
  setup phase; the answer is recorded in `CONTRIBUTING.md` and in the CI job.
- **Pre-1.0 toolkit**: iced breaks its API between minor releases; the version is pinned in
  `Cargo.lock`, and upgrades are their own change.

## R13. CI, licences and dependencies

**Decision**:

- New dependencies and why: `iced` (the toolkit, R1), `rfd` (native dialogs, R6), `serde` +
  `serde_json` (settings, R8), `directories` (per-user paths, R8). All permissive; licences of the
  full tree checked above. `deny.toml` gains `BSL-1.0` (Boost; permissive, used by two Windows
  clipboard crates); nothing else needs adding for iced. The other core crates gain no new
  dependency.
- `rust-version`: workspace stays `1.85` for the core crates; `crates/app` sets its own (1.92 at
  plan time, re-measured when the dependency set is final). The MSRV job builds and tests
  `--workspace --exclude la-app`; the normal jobs build everything.
- Two checks are added to the CI contract: the host API document/version agreement test (in
  `cargo test`) and the frame-plan benchmark baseline (informational, like the earlier ones).
- Resources: a new `ui.ftl` per language (and `host.*` error codes in `errors.ftl`); the parity
  check of feature 005 already scans every `.ftl` and every `.rs` of the workspace, so the new
  strings are covered. The embedded-resource list in `la-i18n` gains `ui.ftl`.

### R10 addendum: spike results (tasks T006 and T007, 2026-11-14)

**Debian build packages (T006)**: the bare `debian:trixie` container with only `build-essential`,
`ca-certificates`, `curl` and `git` (the existing "Debian prerequisites" step) builds, lints and
tests `la-app` with iced 0.14 and `rfd` 0.18 without any extra system package; the run-time
libraries (X11, Wayland, Vulkan, `xdg-desktop-portal`) are loaded dynamically and are only needed
when a window is opened. Nothing is added to the CI prerequisites.

**Headless layout measurement (T007)**: `iced_test` 0.14 (`Simulator`, MIT, MSRV 1.88) builds a view
in a window of a chosen size and, through a custom selector, enumerates every widget with its layout
bounds, visible bounds and text content. Measured on Windows and in the Debian CI container with the
same numbers (a German sentence in a 200 px container wraps to two lines: 200 × 41.6). Findings:

- Text is wrapped by word or glyph, so overflow shows up as **height growth**, not as extra width.
- A parent with a fixed height or `clip(true)` shrinks the text's reported bounds to the allotted
  space; the clipped part cannot be detected from the bounds. The layout rules therefore forbid fixed
  heights for anything containing text (text containers size from content), which the check enforces
  by walking the tree: any `Container` candidate whose bounds are smaller than the union of its text
  children is reported.
- Overlap (two text widgets with intersecting bounds), elements outside the window, and elements
  clipped by a scrollable or window edge (`visible_bounds` smaller than `bounds`) are detected
  directly.

**Decision**: option (a). The text-fit check of task T077 renders every screen and dialog through
`iced_test::Simulator` at 800 × 560 in each of the six languages and reports (1) a widget outside the
window, (2) overlapping text widgets, (3) a clipped text (`visible_bounds` ≠ `bounds`), (4) a
container smaller than its text. Missing glyphs are checked separately and without a window: every
character of every message and of the bundled sample user texts must have a glyph in the bundled
fonts or be covered by the declared system fallback list (task T080). The resource-length lint of
option (b) is not needed. `iced_test` becomes a dev-dependency of `la-app`.
