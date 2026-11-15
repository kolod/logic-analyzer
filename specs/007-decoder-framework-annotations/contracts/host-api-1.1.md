# Contract: Host API additions (version 1.0 → 1.1)

**Status**: the design of the additions to [`docs/host-api.md`](../../../docs/host-api.md). When this
feature is implemented the additions are merged into that document (its version line becomes `1.1`,
`la_host::HOST_API_VERSION` follows, and the existing `host_api_doc` test keeps the table and the code in
agreement). Everything in 1.0 keeps its meaning: these are additions that a shell can ignore, so the
version change is minor.

The decoder-side contract is in [decoder-api.md](decoder-api.md); this document is only what a user
interface sees.

## New state (in `State`)

| Field | Meaning |
|-------|---------|
| `decoders.catalogue` | the registered decoders as plain descriptors: id, localized name and description, signals (role, localized label, required), options (id, localized label, type with its range or choices and default), input kind, whether a generator exists. Built in the current language and rebuilt when the language changes |
| `decoders.instances` | per instance in creation order: id, name, decoder id, source, channel assignments, option values, `lane_visible`, state (`Waiting`, `Running`, `Finished`, `Cancelled`, `Failed`), progress (covered position and fraction), flags (`incomplete`, `outdated`, `loaded`), the last fault (stable code and localized message) and the counts of annotations and refused records |
| `decoders.selection` | the selected annotation: instance id, annotation id, and its details (kind, formatted text, start, end, length in time formatted for the language, error flag) |
| `decoders.save_results` | whether the results are saved with the session |
| `decoders.limits` | the instance, stack-depth and memory limits in force (for the setup form and the messages) |

`Versions` gets a counter `decoders` (instances, their state and the setups), next to `capture`,
`window`, `presentation`, `markers`, `theme` and `notices`; results growth during a pass bumps it, so the
shell redraws lanes while a decoder works. `Changes` gets the flag `DECODERS`.

## New commands

| Group | Command | Effect |
|-------|---------|--------|
| Decoders | `AddDecoder { decoder }` | creates an instance of the registered decoder with default options and no channels yet, selects it for configuration; ignored when the instance limit is reached (`decoder.too_many_instances`) |
| | `ConfigureDecoder { id, setup }` | replaces channels, options, name, lane visibility and generator options of an instance in one step; validated as a whole (`decoder.invalid_setup` names the setting); a valid change that alters results starts a new pass |
| | `SetDecoderChannel { id, role, channel }` | one channel assignment (or none); same validation and effect |
| | `SetDecoderOption { id, option, value }` | one option value; same validation and effect |
| | `RenameDecoder { id, name }` | sanitized, at most 64 characters |
| | `StackDecoder { id, source }` | makes `source` (an instance or the capture) the input of `id`; refused with `decoder.stack_incompatible`, `decoder.stack_cycle` or `decoder.stack_too_deep` |
| | `ShowDecoderLane { id, visible }` | shows or hides the lane and its overlays |
| | `RemoveDecoder(id)` | removes the instance and its results; guarded when instances are stacked on it (`NeedsDecision(Guarded::RemoveDecoder)`, answered with the existing `Decide`) |
| | `RunDecoder(id)` | starts a new pass of an instance that is idle, cancelled, failed or outdated |
| | `CancelDecoder(id)` | stops a running pass within one second, keeping its results marked incomplete |
| Annotations | `SelectAnnotation { instance, annotation }` | selects it and fills `decoders.selection` |
| | `GoToAnnotation(AnnotationTarget)` | `Selected`, `Next`, `Previous` (of the selected instance, by start); moves the window so that the annotation is shown whole, centred; costs `O(log n)` |
| | `ClearAnnotationSelection` | |
| Settings | `SetSaveDecoderResults(bool)` | whether the results are saved with the session |
| | `SetDecoderMemoryLimit(bytes)` | validated (16 MiB to 4 GiB); applies to passes that start afterwards, never to a running one |

Commands that change decoders while a save or load runs wait like the other content-changing commands
(`waits_for_operation`). `StartCapture`, `NewCapture` and `OpenSession` cancel every pass; the unsaved-data
question of 1.0 is unchanged and does not ask about decoder results (they can be recomputed) but about the
capture as before.

Every command above is reachable from the keyboard: the shortcut table gets rows (menu **Decoders**: add,
configure, run, cancel, remove, show lane, next and previous annotation, go to selected annotation; the
shortcut-coverage test requires it).

## Frame plan additions

`FramePlan` gets, next to `channels` and `markers`:

| Field | Meaning |
|-------|---------|
| `lanes` | one lane plan per visible instance in stack order (data-model.md "Lane plan"): rows of cells (`Span`, `Mark`, `Dense`) with the instance id, the name and the state flags |
| `overlays` | per channel of `channels`, the overlay cells that target it (`Mark`, `Edge`, `Dense`) |

The plan stays a pure function of `State` and `ViewSize`; building it costs one window query per
instance (`O(log n + W log n)` plus the cells drawn, at most `4W` per lane and `2W` per channel). It never
reads samples and never fails: an instance whose store cannot be read yields an empty lane and a failed
state, not a panic.

## Events and polling

`poll` additionally drains, after the save and load outcomes and before the capture: the supervisor of the
decode jobs (heartbeats, deadlines, abandoned workers), the progress and results of every job, and the
propagation through stacks. It adds `Notice`s for faults (grouped like all others) and returns `DECODERS`
(and `NOTICES`) when something changed. `wants_ticks()` answers `Fast` while any job runs, as it does for a
capture or a save. With no job running, `poll` does no decoder work and the idle application neither
redraws nor uses measurable CPU.

## Faults

A decoder fault is a notice with a stable code of the `decoder` domain (decoder-api.md and research R7).
The notice names the instance; the state of the instance carries the same code. A fault never changes the
capture, the unsaved state or another instance.

## Sessions

Saving writes `DECO` (when there are instances) and `DANN` (when `save_results` is on and results exist);
opening restores the setup and starts the passes again, or shows the saved results marked `loaded` (then no
pass starts until the user runs one or changes the setup). `Unsaved` becomes `Dirty` when the setup
changes, not when a pass finishes.

## Version

`HOST_API_VERSION` becomes `1.1`. Removing or changing any 1.0 behaviour would need a major version.
