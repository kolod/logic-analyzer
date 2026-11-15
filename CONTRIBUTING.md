# Contributing

This guide is enough to build the project, name a branch, write a commit message and
run the checks that CI runs. The rules come from the project constitution
([.specify/memory/constitution.md](.specify/memory/constitution.md)); where this guide
and the constitution disagree, the constitution wins.

Supported operating systems: **Windows 10/11** and **Debian 13**. Every change must
work on both.

## 1. Build and test

Install [`rustup`](https://rustup.rs). The development toolchain is pinned in
`rust-toolchain.toml` and installed automatically.

- **Debian 13**: `sudo apt install build-essential git curl`, then install `rustup`.
- **Windows 10/11**: install the Visual Studio Build Tools ("Desktop development with
  C++"), Git, then `rustup`.

```text
cargo build --workspace
cargo test --workspace
```

Run these before you push; CI runs the same commands on Windows and Debian 13 and a
failure on either blocks the merge:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny --all-features check      # install once: cargo install cargo-deny --locked
```

### Minimum supported Rust version (MSRV)

The MSRV is **1.85**, declared as `rust-version` in the workspace `Cargo.toml`. CI also
builds and tests with exactly that toolchain, so the claim is verified. To check
locally (the MSRV toolchain needs the `clippy` component because a test runs it):

```text
rustup toolchain install 1.85 --component clippy
cargo +1.85 test --workspace
```

Raising the MSRV or the pinned toolchain is a deliberate change in its own commit.

## 2. Stability rules (non-negotiable)

The primary goal of the project is a program that does not crash and does not lose
data. Production code therefore MUST NOT panic on external input (device data, files,
user input, scripts).

- Return errors as `la_core::Error` (typed `ErrorKind`, stable `code`, English
  `detail`); do not put user-visible text in errors.
- Do not use `unwrap`, `expect`, direct indexing (`v[i]`), unchecked arithmetic,
  `panic!`, `todo!`, `unimplemented!` or `unreachable!` in production code. The
  workspace lints deny them; use `get`, `checked_*`, `?` and pattern matching.
  Tests may use them.
- `unsafe` is denied. A module that really needs it must opt in with
  `#[allow(unsafe_code)]`, a `// SAFETY:` comment on every block, and a review.
- A genuinely safe exception to a lint is allowed only locally and with a reason:

  ```rust
  #[allow(clippy::indexing_slicing, reason = "index is proven in bounds by the loop bound")]
  ```

- Never log captured signal data, only sizes, counts and identifiers.
- Background work uses `la_diagnostics::spawn_worker`, so a panic is contained, logged
  and reported instead of ending the application. Update shared state transactionally
  (build the new value, then swap it in under the lock) and lock with
  `lock_recover`.

## 3. Language

All project documentation is **English**: specifications, plans, tasks, README, this
guide, code comments, commit messages, and the description of any API contract or
script. Log text is English and is not translated. User interface text is never
hardcoded: it lives in the localization resources in six languages (see section 8 and the
constitution, section "Localization Requirements").

## 4. Branches

All work happens in feature branches named in the Spec Kit format
`NNN-short-name`, for example `001-workspace-ci-foundation`. Merging into `main`
happens only after all checks pass. `main` always builds and passes all tests.

## 5. Commits

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org) and
name the component:

```text
<type>(<scope>): <description>
```

- Types: `feat`, `fix`, `perf`, `refactor`, `test`, `style` (scope required) and
  `docs`, `chore`, `build`, `ci`, `revert` (scope optional).
- Scope is the component, for example `core`, `diagnostics`, `ui`, `drivers/logic16`.
- Description in English, imperative mood, no trailing period.
- Examples: `feat(diagnostics): add size-bounded log writer`,
  `fix(drivers/logic16): handle short USB reads`, `docs: update contributor guide`.
- One commit is one logical change. A change to the host API contract and its
  implementation on all platforms land in `main` together.

Check your commits locally (CI reports non-conforming messages as a non-blocking
warning):

```text
bash scripts/check-commits.sh origin/main..HEAD
```

## 6. Dependencies

Prefer the standard library. A new dependency needs a justification in the plan
(purpose, maintenance status, license). `cargo deny` rejects known vulnerabilities,
unmaintained or yanked crates and licenses that are incompatible with GPL-3.0; its
output names the crate and the reason. The project is licensed under GPL-3.0.

## 7. Working with Spec Kit

Features go through `/speckit-specify`, `/speckit-plan`, `/speckit-tasks` and
`/speckit-implement`; the feature documents live in `specs/NNN-short-name/`. Every plan
passes the Constitution Check before research and again after design. The order of
planned features is in [roadmap.md](roadmap.md).

## 8. Working on the application (`la-host` and `la-app`)

The application is split on purpose. **`la-host`** holds everything the program knows how to do:
commands, state, the frame plan, markers, settings, session files, text. **`la-app`** (iced) only
draws what the host describes and turns input into host commands. **`la-app` contains no domain
logic**: if a rule can be tested without a window, it belongs in `la-host`, with its test.

### Adding a command

1. Add the variant to `Command` (`crates/host/src/command.rs`), a short name in `Command::name`,
   and, if it changes the capture or what is being saved, to `Command::waits_for_operation`.
2. Execute it in `Shell::execute` (`crates/host/src/shell.rs`); validate every argument, return
   `Reject::Ignored` for a command that is not allowed now and never panic.
3. Make it reachable from the keyboard: a row in the shortcut table
   (`crates/host/src/shortcuts.rs`) with a menu path and, where it makes sense, an accelerator that
   does not clash with one the desktops keep. The test `shortcut_coverage` fails until the command
   is in the table or in one of its justified lists.
4. Add a row to the command table of [docs/host-api.md](docs/host-api.md); the test
   `host_api_doc` fails if the table and the enum differ. A change that existing shells cannot
   ignore changes `HOST_API_VERSION` and the version line in the document.
5. Write the test first, in `crates/host/tests/`. The random walk (`random_walk`) will pick the
   command up when you add it to its action list; add it there.

### Adding or changing a user interface string

Every text the user sees is a message in `resources/i18n/<language>/ui.ftl`, in all six languages
(English, German, French, Spanish, Japanese, Ukrainian); the parity test fails if a key is missing
in one, if placeholders differ, or if a key is unused. Keys start with the area they belong to:
`menu-`, `toolbar-`, `status-`, `dialog-`, `settings-`, `panel-`, `notice-`, and a title ends in
`-title`. Put a comment above the message that says where it is shown and what each argument is.
Do not build sentences from pieces: one message per sentence, with arguments for numbers and
names. Texts in menus, buttons, labels and titles also have a length budget in every language
(`crates/host/tests/string_budget.rs`) so that they fit at the minimum window size of 800 by 560.
How to write the messages and a worked example are in [docs/localization.md](docs/localization.md).
User-typed text (channel and marker names) is never translated and is sanitized by the host.

### Building and running the application

`cargo run -p la-app` starts it. No system package is needed to build beyond `build-essential`,
`git` and `curl` on Debian 13 (the CI job installs exactly these). To run it you need a graphics
driver and a desktop session (see [docs/user-guide.md](docs/user-guide.md)). `la-app` needs a newer
Rust compiler than the rest of the workspace; the MSRV job builds everything else.

### Tests that need a window

Pictures of the interface are rendered without a window by the toolkit test support. Set
`LA_SNAPSHOT_DIR` to a folder to get them as PNG files for a person to look at (an existing file is
kept, so delete the old one first). The tests that render take turns because several software
renderers at once crashed the Windows CI runner. `LA_FRAME_STATS=1` makes the running application
log how long building and drawing a frame take.
