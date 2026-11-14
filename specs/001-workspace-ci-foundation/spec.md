# Feature Specification: Workspace and CI Foundation

**Feature Branch**: `001-workspace-ci-foundation`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "Workspace and CI foundation (roadmap item 001): a Rust workspace with a documented minimum supported Rust version (MSRV); CI on Windows 10/11 and Debian 13 running format, lint, test and dependency-audit checks, blocking merges to main on failure; size-bounded structured logging to a local log file; a shared typed error type and the rule that production code never panics on external input, enforced automatically; worker-thread panics caught at the thread boundary, logged and reported without losing user data; English-only documentation and Conventional Commits conventions documented in a contributor guide. No UI, no devices, no signal storage in this feature."

## User Scenarios & Testing *(mandatory)*

Here the "users" are the project's contributors and maintainers, and, indirectly,
the end users who depend on a program that never crashes and never loses data.

### User Story 1 - Merge gate that keeps `main` working (Priority: P1)

A contributor opens a change. Automated checks run on both supported operating
systems (Windows 10/11 and Debian 13) and verify formatting, lint cleanliness,
tests and the safety of third-party dependencies. A change that fails any check
cannot be merged into `main`.

**Why this priority**: the constitution requires that every commit on `main` builds
and passes all tests. Without an enforced gate, every later feature can silently
break that rule.

**Independent Test**: open a change that introduces a formatting error, a lint
warning, a failing test, or a dependency with a known vulnerability, and confirm that
each one is reported as a failed check on the affected operating system(s) and that
merging is blocked. Then open a clean change and confirm that all checks pass.

**Acceptance Scenarios**:

1. **Given** a clean change, **When** it is opened for merge, **Then** all checks
   complete on both Windows and Debian and the change is mergeable.
2. **Given** a change with a formatting violation, a lint warning, or a failing
   test, **When** checks run, **Then** the corresponding check fails and merging into
   `main` is blocked.
3. **Given** a change that adds a dependency with a known vulnerability or a
   disallowed license, **When** checks run, **Then** the dependency check fails and
   merging is blocked.
4. **Given** a change that works on one operating system but fails on the other,
   **When** checks run, **Then** the failure is reported against that operating system
   and merging is blocked.

---

### User Story 2 - Workspace a contributor can build in minutes (Priority: P1)

A new contributor clones the repository on Windows 10/11 or Debian 13, follows the
contributor guide, and builds and tests the whole workspace successfully. The
guide states the minimum supported Rust version, and the build refuses to proceed
(with a clear message) on a toolchain older than that version.

**Why this priority**: all later features (storage, devices, decoders, UI) are added
as parts of this workspace; it must exist, build everywhere, and be easy to extend.

**Independent Test**: on a clean machine of each supported operating system, follow
only the contributor guide and run the documented build and test commands; both
succeed with no undocumented steps.

**Acceptance Scenarios**:

1. **Given** a clean supported machine with the minimum supported toolchain,
   **When** the contributor follows the guide, **Then** the workspace builds and the
   tests pass without extra manual steps.
2. **Given** a toolchain older than the documented minimum, **When** the contributor
   builds, **Then** the build fails with a message that states the required version.
3. **Given** the documented minimum toolchain, **When** CI runs, **Then** the
   workspace is also built and tested against that minimum version, so the stated
   minimum is verified, not just claimed.

---

### User Story 3 - Typed errors and no panics on external input (Priority: P1)

Production code reports every failure as a typed error value that carries enough
information to tell the user what failed, and never panics because of external data
(device data, files, user input, scripts). Constructs that can panic on such data
(`unwrap`, `expect`, unchecked indexing and unchecked arithmetic) are forbidden in
production code and the prohibition is enforced automatically, while remaining
allowed in tests.

**Why this priority**: stability and data integrity are the project's primary goal
(Principle I). Establishing the error model and the enforcement before any feature
code exists is far cheaper than retrofitting it.

**Independent Test**: add a sample use of a forbidden construct to production code and
confirm the automatic checks reject it; add the same construct in test code and
confirm it is accepted. Feed deliberately malformed input to the sample code paths
and confirm they return a typed error instead of panicking.

**Acceptance Scenarios**:

1. **Given** production code that uses `unwrap`, `expect`, unchecked indexing or
   unchecked arithmetic, **When** the lint check runs, **Then** it fails.
2. **Given** the same constructs in test code, **When** the lint check runs, **Then**
   it passes.
3. **Given** an operation that receives malformed external input, **When** it fails,
   **Then** it returns a typed error that identifies the failure category and carries
   a human-readable description, with the underlying cause preserved for the log.
4. **Given** any error reaching the user, **When** it is displayed, **Then** it is
   separable into a user-facing message and technical detail kept for the log
   (Principle VI).

---

### User Story 4 - Worker panics are contained, logged and reported (Priority: P2)

When a background worker thread panics anyway (a bug), the panic is caught at the
thread boundary. It is recorded in the log with its location and message, reported
to the rest of the application as a failure of that worker, and never terminates the
whole application or discards user data that already exists.

**Why this priority**: even with strict rules a bug can slip through; containment is
the safety net required by Principles I and VI. It ranks below the prevention
stories because it handles the exception, not the rule.

**Independent Test**: run a worker that deliberately panics while other state (a
simulated in-memory session) exists; confirm the application keeps running, the
session data is intact, the failure is reported, and the log contains the panic
details.

**Acceptance Scenarios**:

1. **Given** a worker that panics, **When** the panic occurs, **Then** the panic is
   caught, the application continues, and the failure is reported as that worker's
   result.
2. **Given** the panic, **When** the log is inspected, **Then** it contains the panic
   message, the source location and the identity of the worker.
3. **Given** data held by the application at the time of the panic, **When** the
   worker fails, **Then** that data remains accessible and unmodified by the failure
   (no poisoned or half-updated shared state).
4. **Given** several workers, **When** one panics, **Then** the others keep running.

---

### User Story 5 - Structured, size-bounded local log (Priority: P2)

The application writes structured diagnostics to a log file on the local machine.
The log has a bounded total size: when the limit is reached the oldest entries are
discarded, so the log never fills the disk. Each entry carries a timestamp, a severity
level, the component that produced it, and a message with structured fields. Nothing
is sent over the network.

**Why this priority**: observability (Principle VI) is needed for every later
feature (device events, capture lifecycle, decoder faults) and for diagnosing the
crashes that story 4 contains.

**Independent Test**: write far more log data than the configured limit and confirm
that the total on-disk size stays within the limit, that the most recent entries are
preserved, and that every retained entry is complete and readable.

**Acceptance Scenarios**:

1. **Given** a running application, **When** events are logged, **Then** each entry
   appears in the log file with timestamp, severity, component and structured fields.
2. **Given** log output exceeding the configured limit, **When** logging continues,
   **Then** total log size on disk stays within the limit and the newest entries are
   retained.
3. **Given** an unwritable or missing log location, **When** the application starts,
   **Then** it still starts, reports the problem once to the user, and falls back
   gracefully (it never refuses to run, and never crashes, because logging failed).
4. **Given** an abrupt process termination during logging, **When** the log is next
   read, **Then** at most the last partially written entry is lost or malformed, and
   earlier entries remain readable.

---

### User Story 6 - Documented contribution rules (Priority: P3)

A contributor guide, written in English, states: how to build and test, the minimum
supported Rust version, the English-only documentation rule (documentation, code
comments, commit messages), the Conventional Commits format with a component scope,
the feature-branch naming (`NNN-short-name`) and the rules for merging into `main`.

**Why this priority**: these conventions are already in the constitution; the guide
makes them discoverable to contributors and is cheap, but it does not block
engineering work.

**Independent Test**: a contributor who has not read the constitution can, using only
the guide, name a branch, write a conforming commit message, and run the required
checks locally.

**Acceptance Scenarios**:

1. **Given** the guide, **When** a contributor prepares a change, **Then** they can
   determine the branch name, commit message format and local check commands from it
   alone.
2. **Given** a commit message that does not follow the Conventional Commits format,
   **When** checks run, **Then** it is reported (see Assumptions for enforcement
   level).

---

### Edge Cases

- A dependency becomes vulnerable after it was merged: the periodic or next dependency
  check fails even if the contributor changed nothing.
- CI runner images change (new toolchain defaults): the workspace is pinned so CI
  results stay reproducible; a toolchain change is a deliberate commit.
- A forbidden construct is genuinely safe (for example, a proven-in-bounds index):
  an explicit, documented exception mechanism exists, requires a written
  justification at the usage site, and is visible in review.
- The log file is on a read-only or full disk: logging degrades gracefully without
  crashing the application.
- The log limit is configured to an unreasonably small value: a sane minimum is
  enforced.
- Two application instances write to the same log location: neither corrupts the log
  of the other.
- A panic occurs while the panic reporter itself is running, or while the log is
  being written: the application still does not terminate silently and does not
  recurse.
- A panic occurs on the main thread: it is still logged before the process ends, and
  no data that was already saved is damaged.
- Windows and Debian differ in path and line-ending conventions: tests and the log
  location behave correctly on both.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The repository MUST contain a multi-crate workspace that builds and
  passes its tests on Windows 10/11 and Debian 13, to which later features can add
  crates without restructuring.
- **FR-002**: The workspace MUST declare a minimum supported Rust version, MUST
  document it in the contributor guide, and MUST reject builds on older toolchains
  with a message that states the required version.
- **FR-003**: CI MUST run on both Windows and Debian for every change and every push
  to `main`, and MUST verify: formatting, lint cleanliness with warnings treated as
  errors, the full test suite, and a dependency check covering known vulnerabilities
  and licenses.
- **FR-004**: CI MUST also build and test with the documented minimum Rust version.
- **FR-005**: A failing CI check on any supported operating system MUST block merging
  into `main`; a change MUST NOT be mergeable until all checks pass.
- **FR-006**: The toolchain and dependency versions used by CI MUST be pinned and
  reproducible; changing them MUST be an explicit, reviewed change.
- **FR-007**: A dependency check failure MUST name the offending dependency and the
  reason (vulnerability or license).
- **FR-008**: The workspace MUST provide a shared typed error model that classifies
  failures by category, carries a human-readable description, preserves the
  underlying cause, and distinguishes the user-facing message from technical detail
  for the log.
- **FR-009**: Production code MUST NOT panic on external input. The following MUST be
  rejected automatically in production code and allowed in test code: `unwrap`,
  `expect`, unchecked indexing, unchecked arithmetic on external data, and explicit
  `panic`-family macros.
- **FR-010**: Any exception to FR-009 MUST be explicit at the usage site and MUST
  carry a written justification.
- **FR-011**: Use of `unsafe` MUST be rejected automatically unless the block carries
  a justification comment, in line with the constitution.
- **FR-012**: A panic in a worker thread MUST be caught at the thread boundary, MUST
  NOT terminate the application, and MUST be reported to the caller as a failure of
  that worker.
- **FR-013**: Every caught panic MUST be logged with its message, source location and
  the identity of the worker; panics on the main thread MUST also be logged before
  the process ends.
- **FR-014**: A worker panic MUST NOT corrupt or discard data owned by the
  application; shared state MUST remain usable after a panic.
- **FR-015**: The application MUST write structured log entries to a local file, each
  with a timestamp, severity level, producing component, message and structured
  fields.
- **FR-016**: The log MUST have a bounded total size on disk; when the bound is
  reached the oldest entries MUST be discarded first. The bound MUST be configurable
  and have a safe default and a safe minimum.
- **FR-017**: Failure to open or write the log MUST NOT prevent the application from
  starting or running; it MUST be reported once and degrade gracefully.
- **FR-018**: Logging MUST NOT transmit any data over the network.
- **FR-019**: Log entries MUST NOT contain captured signal data payloads (only
  metadata such as sizes and counts), to keep logs small and private.
- **FR-020**: A contributor guide, in English, MUST document: build and test
  commands for both operating systems, the minimum supported Rust version, the
  English-only documentation rule, Conventional Commits with a component scope,
  feature-branch naming, and the merge rules for `main`.
- **FR-021**: The repository MUST contain automated tests demonstrating FR-008
  through FR-017, including at least one deliberately malformed input and one
  deliberately panicking worker.
- **FR-022**: This feature MUST NOT introduce any UI, device access, signal storage
  or file format.

### Key Entities

- **Workspace**: the set of crates that make up the application; the unit that CI
  builds and tests.
- **Error**: a typed failure with a category, a user-facing message, technical detail
  and a preserved cause.
- **Log entry**: one structured diagnostic record (timestamp, severity, component,
  message, fields).
- **Log store**: the bounded on-disk collection of log entries.
- **Worker failure report**: the outcome delivered to the application when a worker
  panics (worker identity, panic message, location).
- **CI check**: one automated verification (format, lint, test, dependency audit,
  minimum-version build) executed per operating system.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of changes merged into `main` have passed every CI check on both
  operating systems; a deliberately broken change in each category (format, lint,
  test, dependency, minimum version) is blocked.
- **SC-002**: A new contributor can build and test the whole workspace on either
  supported operating system within 30 minutes using only the contributor guide.
- **SC-003**: The full CI run for a typical change completes in under 15 minutes
  (excluding cold-cache dependency downloads).
- **SC-004**: In the test suite, 100% of deliberately malformed inputs produce a typed
  error and 0 cause a panic.
- **SC-005**: A deliberately panicking worker never terminates the application and
  never alters other session data, across at least 1,000 repeated runs.
- **SC-006**: After writing at least 10 times the configured log limit, total log
  size on disk never exceeds the limit by more than one entry, and the newest entries
  are always present.
- **SC-007**: An unwritable log location never prevents application start-up (verified
  on both operating systems).
- **SC-008**: Adding a forbidden construct to production code is rejected by the
  automatic checks in 100% of tested cases.

## Assumptions

- Contributors and maintainers are the primary users; end-user-visible behavior
  (UI, devices, capture) belongs to later roadmap items.
- Continuous integration is hosted on the project's existing public GitHub
  repository, which supports required status checks on `main`; enforcing them is part
  of the repository settings and is performed by the repository owner.
- The specific minimum Rust version, the dependency-audit tool(s), the logging
  library and the log default/minimum sizes are decided in the implementation plan.
- Conventional Commits enforcement starts as a documented convention with an
  automated, non-blocking report; making it blocking can be decided later.
- Both supported CPU architectures are assumed to be 64-bit x86; other architectures
  are out of scope for this feature.
- Windows 10/11 and Debian 13 are exercised in CI on hosted runners (or a container
  for Debian 13); real-hardware testing is out of scope until driver features.
- Release packaging, signing and publishing are out of scope for this feature
  (roadmap item 014+).
- Localization of log text is out of scope: log entries are technical, written in
  English, and are not translated (user-facing error messages are localized later,
  under roadmap item 005).
