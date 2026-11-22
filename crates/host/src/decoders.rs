//! Decoders in the host: the catalogue built from the descriptors, the instances on the capture on
//! screen, their passes on supervised workers, the selected annotation, and the plans of their
//! lanes and overlays (`docs/host-api.md`, "Decoders").
//!
//! The state a user interface reads is plain data ([`DecoderState`]); the threads and the results
//! live in the shell (`DecoderRuntime`) because they cannot be cloned with the state.

use std::collections::BTreeMap;
use std::sync::Arc;

use la_decoders::descriptor::{Descriptor, Input, OptionKind};
use la_decoders::input::SnapshotSource;
use la_decoders::lanes::{self, LANE_ROWS, OVERLAY_ROWS};
use la_decoders::model::{Annotation, Kind, Target, Text};
use la_decoders::registry::Registry;
use la_decoders::run::{
    ChunkLimits, DEFAULT_CHUNK_EDGES, DEFAULT_CHUNK_SPAN, Ended, Job, JobSpec, Outcome, Results,
};
use la_decoders::setup::{MAX_INSTANCES, MAX_STACK_DEPTH, Setup, Source};
use la_decoders::stack::{ItemStream, dependants, stack_order};
use la_decoders::store::{
    Budget, DEFAULT_MAX_ANNOTATIONS, DEFAULT_MEMORY_LIMIT, Full, Limits as StoreLimits,
};
use la_i18n::{Args, Key, Localizer};

use crate::command::{AnnotationTarget, Changes};
use crate::notices::Severity;
use crate::plan::{LanePlan, OverlayPlan};
use crate::sanitize::{MAX_NAME_CHARS, sanitize_text};
use crate::shell::{Executed, Reject, Shell, mark_dirty, report_into};
use crate::state::State;
use crate::window::ViewWindow;

/// A signal of a decoder, for the setup form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueSignal {
    /// The role.
    pub role: String,
    /// The label in the current language.
    pub label: String,
    /// Whether a channel is required.
    pub required: bool,
}

/// The type and rules of an option, for the setup form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogueOptionKind {
    /// One of a list of `(value id, label)`.
    Choice {
        /// The choices.
        choices: Vec<(String, String)>,
        /// The default value id.
        default: String,
    },
    /// A whole number between `min` and `max`.
    Integer {
        /// The smallest value.
        min: i64,
        /// The largest value.
        max: i64,
        /// The default.
        default: i64,
    },
    /// A switch.
    Boolean {
        /// The default.
        default: bool,
    },
}

/// An option of a decoder, for the setup form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueOption {
    /// The identifier.
    pub id: String,
    /// The label in the current language.
    pub label: String,
    /// The type and rules.
    pub kind: CatalogueOptionKind,
}

/// A registered decoder as a user interface lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueEntry {
    /// The decoder id.
    pub id: String,
    /// The name in the current language.
    pub name: String,
    /// The description in the current language.
    pub description: String,
    /// The signals, in order.
    pub signals: Vec<CatalogueSignal>,
    /// The options, in order.
    pub options: Vec<CatalogueOption>,
    /// Whether it reads the output of another decoder instead of channels.
    pub reads_items: bool,
    /// Whether a generator exists for it.
    pub has_generator: bool,
}

/// Where an instance is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceState {
    /// Not running: not set up yet or waiting for a free decode thread.
    Waiting,
    /// A pass is running (or following a capture that grows).
    Running,
    /// The last pass finished.
    Finished,
    /// The last pass was cancelled; what it found is kept, incomplete.
    Cancelled,
    /// The last pass failed; what it found before is kept, incomplete.
    Failed,
}

/// One decoder instance as a user interface sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceView {
    /// The instance id.
    pub id: u32,
    /// What the user chose.
    pub setup: Setup,
    /// Where it is in its life.
    pub state: InstanceState,
    /// Whether the setup is complete (a pass can run).
    pub ready: bool,
    /// Everything before this position has been processed.
    pub covered: u64,
    /// The length of the capture the pass works on.
    pub length: u64,
    /// The results are not whole.
    pub incomplete: bool,
    /// The results no longer match the setup or the source.
    pub outdated: bool,
    /// The results came from a file.
    pub loaded: bool,
    /// The stable code of the last fault, if any.
    pub fault: Option<&'static str>,
    /// How many annotations are stored.
    pub annotations: u64,
    /// How many records the framework refused.
    pub refused: u64,
}

/// The reason a decoder command was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The stable error code.
    pub code: &'static str,
    /// The setting that was refused (`name`, `source`, `channels.<role>`, `options.<id>`), or empty.
    pub setting: String,
}

/// Which annotation: the store it lives in and how to find it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnotationRef {
    /// The lane or the overlay of a channel.
    pub target: Target,
    /// The id in that store.
    pub id: u64,
    /// The first position of the annotation.
    pub start: u64,
}

/// What the interface shows about the selected annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationDetails {
    /// `range`, `point` or `edge`.
    pub kind: &'static str,
    /// The text in the current language.
    pub text: String,
    /// The first position.
    pub start: u64,
    /// The end position (equal to `start` for a point).
    pub end: u64,
    /// The length in time, formatted for the language.
    pub duration: String,
    /// Whether it reports a problem in the signal.
    pub error: bool,
}

/// The selected annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    /// The instance.
    pub instance: u32,
    /// The annotation.
    pub annotation: AnnotationRef,
    /// What to show about it.
    pub details: AnnotationDetails,
}

/// The limits in force, for the setup form and the messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderLimits {
    /// The most instances.
    pub instances: usize,
    /// The deepest stack.
    pub stack_depth: usize,
    /// The annotations one instance may store.
    pub annotations: usize,
    /// The memory the results of all instances may use.
    pub memory_bytes: u64,
}

/// Everything about decoders that a user interface reads.
#[derive(Debug, Clone, PartialEq)]
pub struct DecoderState {
    /// The registered decoders in the current language.
    pub catalogue: Vec<CatalogueEntry>,
    /// The instances, in creation order.
    pub instances: Vec<InstanceView>,
    /// Why the last command was refused (cleared by the next accepted one).
    pub refusal: Option<Refusal>,
    /// The selected annotation.
    pub selection: Option<Selection>,
    /// Whether results are saved with the session.
    pub save_results: bool,
    /// The limits in force.
    pub limits: DecoderLimits,
    /// The id the next instance gets (ids are never reused).
    pub(crate) next_id: u32,
    /// Records of the opened file that are written back as they were read.
    pub(crate) kept: Vec<crate::decoder_sessions::Kept>,
}

impl DecoderState {
    pub(crate) fn new(catalogue: Vec<CatalogueEntry>) -> Self {
        Self {
            catalogue,
            instances: Vec::new(),
            refusal: None,
            selection: None,
            save_results: false,
            limits: DecoderLimits {
                instances: MAX_INSTANCES,
                stack_depth: MAX_STACK_DEPTH,
                annotations: DEFAULT_MAX_ANNOTATIONS,
                memory_bytes: DEFAULT_MEMORY_LIMIT,
            },
            next_id: 1,
            kept: Vec::new(),
        }
    }

    /// The instance with this id.
    #[must_use]
    pub fn instance(&self, id: u32) -> Option<&InstanceView> {
        self.instances.iter().find(|i| i.id == id)
    }

    pub(crate) fn instance_mut(&mut self, id: u32) -> Option<&mut InstanceView> {
        self.instances.iter_mut().find(|i| i.id == id)
    }

    /// Forgets every instance (the capture they belong to is gone).
    pub(crate) fn reset(&mut self) {
        self.instances.clear();
        self.kept.clear();
        self.selection = None;
        self.refusal = None;
    }
}

/// The threads and results of the passes (not part of the cloneable state).
pub(crate) struct DecoderRuntime {
    pub(crate) registry: Registry,
    pub(crate) budget: Arc<Budget>,
    pub(crate) jobs: BTreeMap<u32, Job>,
    pub(crate) results: BTreeMap<u32, Results>,
    /// The items of the latest pass of each instance, read by the instances stacked on it.
    pub(crate) streams: BTreeMap<u32, ItemStream>,
    /// Saved results that are being checked and rebuilt on a worker.
    pub(crate) loading: Option<std::sync::mpsc::Receiver<LoadOutcome>>,
    /// A new memory limit that waits for the running passes to end.
    pub(crate) pending_limit: Option<u64>,
    /// The deadlines the supervisor enforces.
    pub(crate) deadlines: la_decoders::run::Deadlines,
    /// How big a chunk may be.
    pub(crate) chunk_limits: ChunkLimits,
    /// The most passes that run at once.
    pub(crate) thread_cap: usize,
    /// Instances that wait for a free decode thread, in the order they asked.
    queue: std::collections::VecDeque<u32>,
}

/// The most decode threads that run at once: half the cores, at least one, so that the capture and
/// the interface keep the rest.
#[must_use]
pub fn default_thread_cap() -> usize {
    std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .checked_div(2)
        .unwrap_or(1)
        .max(1)
}

impl Drop for DecoderRuntime {
    /// The passes are asked to stop and given a moment to do so, so that no worker is still using
    /// shared state while the process (or a test) is torn down.
    fn drop(&mut self) {
        for job in self.jobs.values() {
            job.cancel();
        }
        let deadline = std::time::Instant::now().checked_add(std::time::Duration::from_secs(2));
        while self.jobs.values().any(|job| !job.is_finished())
            && deadline.is_some_and(|d| std::time::Instant::now() < d)
        {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

impl DecoderRuntime {
    pub(crate) fn new(memory_limit: u64) -> Self {
        Self {
            registry: Registry::builtin(),
            budget: Budget::new(memory_limit),
            jobs: BTreeMap::new(),
            results: BTreeMap::new(),
            pending_limit: None,
            loading: None,
            streams: BTreeMap::new(),
            deadlines: la_decoders::run::Deadlines::default(),
            chunk_limits: ChunkLimits {
                max_edges: DEFAULT_CHUNK_EDGES,
                max_span: DEFAULT_CHUNK_SPAN,
            },
            thread_cap: default_thread_cap(),
            queue: std::collections::VecDeque::new(),
        }
    }

    /// Puts results that were read from a file in place of a pass.
    pub(crate) fn install_loaded(&mut self, id: u32, results: Results, stream: ItemStream) {
        self.results.insert(id, results);
        self.streams.insert(id, stream);
    }

    /// Cancels and forgets what belongs to instances that no longer exist.
    pub(crate) fn retain(&mut self, keep: &dyn Fn(u32) -> bool) {
        self.jobs.retain(|id, job| {
            let kept = keep(*id);
            if !kept {
                job.cancel();
            }
            kept
        });
        self.results.retain(|id, _| keep(*id));
        self.streams.retain(|id, _| keep(*id));
        self.queue.retain(|id| keep(*id));
    }
}

/// The catalogue in the current language.
pub(crate) fn catalogue(registry: &Registry, localizer: &Localizer) -> Vec<CatalogueEntry> {
    let text = |key: &'static str| localizer.message(&Key::from_static(key), &Args::new());
    registry
        .list()
        .into_iter()
        .map(|d| CatalogueEntry {
            id: d.id.to_owned(),
            name: text(d.name),
            description: text(d.description),
            signals: d
                .signals
                .iter()
                .map(|s| CatalogueSignal {
                    role: s.role.to_owned(),
                    label: text(s.label),
                    required: s.required,
                })
                .collect(),
            options: d
                .options
                .iter()
                .map(|o| CatalogueOption {
                    id: o.id.to_owned(),
                    label: text(o.label),
                    kind: match o.kind {
                        OptionKind::Choice { choices, default } => CatalogueOptionKind::Choice {
                            choices: choices
                                .iter()
                                .map(|(id, label)| ((*id).to_owned(), text(label)))
                                .collect(),
                            default: default.to_owned(),
                        },
                        OptionKind::Integer { min, max, default } => {
                            CatalogueOptionKind::Integer { min, max, default }
                        }
                        OptionKind::Boolean { default } => CatalogueOptionKind::Boolean { default },
                    },
                })
                .collect(),
            reads_items: matches!(d.input, Input::Items(_)),
            has_generator: d.generator,
        })
        .collect()
}

/// The text of an annotation in the language of `localizer`: a message with its arguments, or the
/// literal text as it is.
#[must_use]
pub fn annotation_text(localizer: &Localizer, text: &Text) -> String {
    match text {
        Text::Literal(text) => text.clone(),
        Text::Message { key, args } => {
            let names = ["value", "second", "third", "fourth"];
            let mut formatted = Args::new();
            for (name, value) in names.iter().zip(args) {
                formatted = formatted.text(name, value);
            }
            localizer.message(&Key::from_static(key), &formatted)
        }
    }
}

/// The shorter form of the text of a block, when it has one: a message whose key ends in `-long` is
/// the long form, and the same message without that ending the short one, which takes the same
/// arguments. A block shows the long form when it fits and the short one when only that does.
#[must_use]
pub fn annotation_text_short(localizer: &Localizer, text: &Text) -> Option<String> {
    let Text::Message { key, args } = text else {
        return None;
    };
    let short = key.strip_suffix("-long")?;
    Some(annotation_text(
        localizer,
        &Text::Message {
            key: short,
            args: args.clone(),
        },
    ))
}

fn refusal_of(error: &la_core::Error) -> Refusal {
    // The detail of a setup error starts with the setting it is about: `channels.clock: ...`.
    let setting = error
        .detail()
        .split_once(':')
        .map(|(setting, _)| setting.trim())
        .filter(|s| !s.contains(' '))
        .unwrap_or_default()
        .to_owned();
    Refusal {
        code: error.code(),
        setting,
    }
}

/// The channels that are shown as lanes, in their order: the ones that are on and were recorded.
pub(crate) fn shown_channels(state: &State) -> Vec<u8> {
    let Some(capture) = state.capture.as_ref() else {
        return Vec::new();
    };
    let recorded = capture.description.enabled_mask();
    state
        .presentation
        .channels
        .iter()
        .filter(|style| style.visible && recorded.contains(style.index))
        .map(|style| style.index)
        .collect()
}

impl Shell {
    /// The channels of the capture on screen (0 without one).
    fn decoder_channels(state: &State) -> u8 {
        state.capture.as_ref().map_or(0, |c| c.snapshot.channels())
    }

    /// The other instances' setups (for validation).
    fn other_setups(state: &State, except: u32) -> Vec<Setup> {
        state
            .decoders
            .instances
            .iter()
            .filter(|i| i.id != except)
            .map(|i| i.setup.clone())
            .collect()
    }

    /// The channels that are recorded: those of the capture on screen, or, while a capture runs, those
    /// it is recording. `None` when there is nothing to tell (no capture).
    #[must_use]
    pub fn recorded_channels(state: &State) -> Option<Vec<u8>> {
        if state.activity != crate::Activity::Idle {
            let mask = state.config.enabled_channels;
            return Some(
                (0..la_storage::MAX_CHANNELS)
                    .filter(|c| mask.contains(*c))
                    .collect(),
            );
        }
        state
            .capture
            .as_ref()
            .map(crate::capture::CaptureView::enabled_channels)
    }

    /// Whether every channel the setup reads is recorded (FR-026): a decoder on a channel that was
    /// switched off has nothing to read.
    #[must_use]
    pub fn channels_are_recorded(state: &State, setup: &Setup) -> bool {
        Self::recorded_channels(state)
            .is_none_or(|recorded| setup.channels.values().all(|c| recorded.contains(c)))
    }

    pub(crate) fn setup_is_ready(&self, state: &State, setup: &Setup) -> bool {
        if !Self::channels_are_recorded(state, setup) {
            return false;
        }
        let others = Self::other_setups(state, setup.instance);
        setup
            .validate(
                &self.decoders.registry,
                &others,
                Self::decoder_channels(state),
            )
            .is_ok()
    }

    /// Where passes read from: the running capture, or the finished one on screen.
    fn decoder_source(&self, state: &State) -> Option<Arc<dyn SnapshotSource>> {
        if let Some(run) = &self.running {
            return Some(Arc::new(run.acquisition.capture()));
        }
        let capture = state.capture.as_ref()?;
        Some(Arc::new(capture.snapshot.clone()))
    }

    /// Starts a new pass of an instance whose setup is complete; a pass that was running is
    /// abandoned and its results dropped.
    pub(crate) fn start_pass(&mut self, state: &mut State, id: u32) {
        self.start_pass_keeping(state, id, false);
    }

    /// Starts a pass. With `keep_old` the results of the pass before stay (flagged outdated) until
    /// the new pass publishes its first results, which replace them at once; otherwise they go.
    pub(crate) fn start_pass_keeping(&mut self, state: &mut State, id: u32, keep_old: bool) {
        if let Some(job) = self.decoders.jobs.remove(&id) {
            job.cancel();
        }
        self.decoders.queue.retain(|queued| *queued != id);
        self.apply_pending_decoder_limit();
        let kept = keep_old && self.decoders.results.contains_key(&id);
        if !kept {
            self.decoders.results.remove(&id);
        }
        let Some(instance) = state.decoders.instance(id).cloned() else {
            return;
        };
        let ready = self.setup_is_ready(state, &instance.setup);
        let Some(descriptor) = self.decoders.registry.get(&instance.setup.decoder) else {
            return;
        };
        if ready && la_decoders::run::stuck_workers() >= la_decoders::run::MAX_STUCK_WORKERS {
            // Too many workers are stuck: new passes are refused until the application restarts.
            let error = la_decoders::error::too_many_stuck("stuck workers are still running");
            report_into(state, Severity::Error, &error);
            if let Some(slot) = state.decoders.instance_mut(id) {
                slot.ready = true;
                slot.state = InstanceState::Failed;
                slot.fault = Some(error.code());
                slot.incomplete = true;
                // The old results were dropped above (when not kept), so nothing of them may still
                // be claimed: not that they come from a file, nor how much they covered.
                slot.loaded = false;
                slot.outdated = kept;
                if !kept {
                    slot.covered = 0;
                    slot.annotations = 0;
                    slot.refused = 0;
                }
            }
            return;
        }
        let source = self.decoder_source(state);
        // A decoder that reads items needs the stream of the instance below; without one (that
        // instance has not started) it waits, and is started when the one below starts.
        let input = match instance.setup.source {
            Source::Capture => Some(None),
            Source::Instance(below) => self.decoders.streams.get(&below).cloned().map(Some),
        };
        let started = match (ready, source, input) {
            (true, Some(source), Some(input)) => Some((source, descriptor, input)),
            _ => None,
        };
        let mut view = instance;
        view.ready = ready;
        view.covered = 0;
        if !kept {
            view.annotations = 0;
            view.refused = 0;
        }
        view.incomplete = false;
        view.outdated = kept;
        view.loaded = false;
        view.fault = None;
        view.state = InstanceState::Waiting;
        if started.is_some() && self.decoders.jobs.len() >= self.decoders.thread_cap {
            // Every decode thread is busy: this instance waits its turn.
            self.decoders.queue.push_back(id);
            if let Some(slot) = state.decoders.instance_mut(id) {
                *slot = view;
            }
            return;
        }
        if let Some((source, descriptor, input)) = started {
            match self.decoders.registry.create(&view.setup) {
                Ok(run) => {
                    let output = ItemStream::new();
                    self.decoders.streams.insert(id, output.clone());
                    let job = Job::spawn(JobSpec {
                        descriptor,
                        setup: view.setup.clone(),
                        run,
                        capture: source,
                        chunk_limits: self.decoders.chunk_limits,
                        limits: StoreLimits {
                            max_annotations: state.decoders.limits.annotations,
                        },
                        budget: Arc::clone(&self.decoders.budget),
                        deadlines: self.decoders.deadlines,
                        output,
                        input,
                    });
                    self.decoders.jobs.insert(id, job);
                    view.state = InstanceState::Running;
                }
                Err(error) => {
                    report_into(state, Severity::Error, &error);
                    view.state = InstanceState::Failed;
                    view.fault = Some(error.code());
                    view.incomplete = true;
                }
            }
        }
        if let Some(slot) = state.decoders.instance_mut(id) {
            *slot = view;
        }
        // The instances stacked on this one read its new items: they start again, their old
        // results kept (flagged outdated) until the new ones arrive.
        let above: Vec<u32> = state
            .decoders
            .instances
            .iter()
            .filter(|i| i.setup.source == Source::Instance(id))
            .map(|i| i.id)
            .collect();
        for dependant in above {
            self.start_pass_keeping(state, dependant, true);
        }
    }

    /// The source of a simulated capture: the generators of the instances that have one, on their
    /// channels, with the random signals on the rest.
    pub(crate) fn generator_source(
        &self,
        state: &State,
    ) -> Option<Arc<dyn la_devices::SignalSourceFactory>> {
        let setups: Vec<Setup> = state
            .decoders
            .instances
            .iter()
            .map(|i| i.setup.clone())
            .collect();
        let claims = self.decoders.registry.claims(&setups);
        if claims.is_empty() {
            return None;
        }
        Some(Arc::new(
            la_decoders::generators::composite::CompositeFactory::new(
                claims,
                state.config.profile.mean_gap(),
            ),
        ))
    }

    /// A new capture starts: every instance decodes it from the start.
    pub(crate) fn restart_decoders(&mut self, state: &mut State) {
        state.decoders.selection = None;
        // Instances that read the capture start; the ones stacked on them follow.
        let ids: Vec<u32> = state
            .decoders
            .instances
            .iter()
            .filter(|i| i.setup.source == Source::Capture)
            .map(|i| i.id)
            .collect();
        for id in ids {
            self.start_pass(state, id);
        }
    }

    /// Forgets every pass and result (a capture is replaced).
    pub(crate) fn clear_decoder_runtime(&mut self) {
        self.decoders.loading = None;
        self.decoders.retain(&|_| false);
    }

    /// Applies a new memory limit as soon as no pass is running.
    pub(crate) fn apply_pending_decoder_limit(&mut self) {
        if self.decoders.jobs.is_empty() {
            if let Some(limit) = self.decoders.pending_limit.take() {
                self.decoders.budget.set_limit(limit);
            }
        }
    }

    pub(crate) fn run_decoder(&mut self, state: &mut State, id: u32) -> Result<Executed, Reject> {
        let Some(instance) = state.decoders.instance(id) else {
            return Err(Reject::Ignored(crate::error::INVALID_COMMAND));
        };
        if instance.state == InstanceState::Running || state.capture.is_none() {
            return Err(Reject::Ignored(crate::error::INVALID_COMMAND));
        }
        state.decoders.refusal = None;
        self.start_pass(state, id);
        Ok(Executed::Changed(Changes::DECODERS))
    }

    pub(crate) fn cancel_decoder(
        &mut self,
        state: &mut State,
        id: u32,
    ) -> Result<Executed, Reject> {
        let job = self
            .decoders
            .jobs
            .get(&id)
            .ok_or(Reject::Ignored(crate::error::INVALID_COMMAND))?;
        job.cancel();
        let _ = state;
        Ok(Executed::Changed(Changes::NONE))
    }

    /// Removes an instance and every instance stacked on it, after the user agreed.
    pub(crate) fn remove_with_dependants(
        &mut self,
        state: &mut State,
        id: u32,
    ) -> Result<Executed, Reject> {
        let setups: Vec<Setup> = state
            .decoders
            .instances
            .iter()
            .map(|i| i.setup.clone())
            .collect();
        let mut doomed = dependants(&setups, id);
        doomed.push(id);
        for gone in doomed.iter().rev() {
            if let Some(job) = self.decoders.jobs.remove(gone) {
                job.cancel();
            }
            self.decoders.results.remove(gone);
            self.decoders.streams.remove(gone);
        }
        state.decoders.instances.retain(|i| !doomed.contains(&i.id));
        for gone in &doomed {
            state.presentation.forget_decoder(*gone);
        }
        if state
            .decoders
            .selection
            .as_ref()
            .is_some_and(|s| doomed.contains(&s.instance))
        {
            state.decoders.selection = None;
        }
        state.decoders.refusal = None;
        mark_dirty(state);
        Ok(Executed::Changed(Changes::DECODERS))
    }

    /// What the instance could read from: the capture (for a decoder that reads signals) or an
    /// instance whose output fits and that would not close a cycle or make the stack too deep.
    #[must_use]
    pub fn decoder_sources(&self, id: u32) -> Vec<Source> {
        let state = &self.state;
        let Some(instance) = state.decoders.instance(id) else {
            return Vec::new();
        };
        let channels = Self::decoder_channels(state);
        let others = Self::other_setups(state, id);
        let mut candidates = vec![Source::Capture];
        candidates.extend(
            state
                .decoders
                .instances
                .iter()
                .filter(|i| i.id != id)
                .map(|i| Source::Instance(i.id)),
        );
        candidates
            .into_iter()
            .filter(|source| {
                let mut setup = instance.setup.clone();
                setup.source = *source;
                // The channels and options are not what is asked here.
                match setup.validate(&self.decoders.registry, &others, channels) {
                    Ok(()) => true,
                    Err(e) => {
                        e.code() == la_decoders::error::INVALID_SETUP
                            && e.detail().starts_with("channels.")
                    }
                }
            })
            .collect()
    }

    /// Cancels the passes and removes the results of instances that are gone.
    pub(crate) fn sync_decoders(&mut self) {
        let ids: Vec<u32> = self.state.decoders.instances.iter().map(|i| i.id).collect();
        self.decoders.retain(&|id| ids.contains(&id));
    }

    pub(crate) fn refuse(state: &mut State, error: &la_core::Error) -> Executed {
        tracing::info!(target: "la_host", code = error.code(), "a decoder command was refused");
        state.decoders.refusal = Some(refusal_of(error));
        Executed::Changed(Changes::DECODERS)
    }

    pub(crate) fn add_decoder(
        &mut self,
        state: &mut State,
        decoder: &str,
    ) -> Result<Executed, Reject> {
        if state.capture.is_none() || self.decoders.registry.get(decoder).is_none() {
            return Err(Reject::Ignored(crate::error::INVALID_COMMAND));
        }
        let id = state.decoders.next_id;
        let setup = Setup::new(id, decoder);
        if state
            .decoders
            .instances
            .len()
            .saturating_add(state.decoders.kept.len())
            >= MAX_INSTANCES
        {
            let error = la_decoders::error::too_many_instances("the instance limit is reached");
            return Ok(Self::refuse(state, &error));
        }
        state.decoders.next_id = id.saturating_add(1);
        state.decoders.refusal = None;
        state.decoders.instances.push(InstanceView {
            id,
            setup,
            state: InstanceState::Waiting,
            ready: false,
            covered: 0,
            length: 0,
            incomplete: false,
            outdated: false,
            loaded: false,
            fault: None,
            annotations: 0,
            refused: 0,
        });
        mark_dirty(state);
        Ok(Executed::Changed(Changes::DECODERS))
    }

    /// Replaces the setup of an instance; a change that alters results starts a new pass.
    pub(crate) fn configure_decoder(
        &mut self,
        state: &mut State,
        id: u32,
        mut setup: Setup,
    ) -> Result<Executed, Reject> {
        let Some(current) = state.decoders.instance(id).cloned() else {
            return Err(Reject::Ignored(crate::error::INVALID_COMMAND));
        };
        setup.instance = id;
        setup.decoder = current.setup.decoder.clone();
        setup.name = sanitize_text(&setup.name, MAX_NAME_CHARS);
        let others = Self::other_setups(state, id);
        let channels = Self::decoder_channels(state);
        // A setup that is not complete yet (a required channel missing) is kept as the user
        // builds it; what is wrong with the parts that are there is refused.
        if let Err(error) = setup.validate_partial(&self.decoders.registry, &others, channels) {
            return Ok(Self::refuse(state, &error));
        }
        state.decoders.refusal = None;
        let changes_results = {
            let (a, b) = (&current.setup, &setup);
            (a.channels.clone(), a.options.clone(), a.source)
                != (b.channels.clone(), b.options.clone(), b.source)
                || a.generator != b.generator
        };
        if let Some(slot) = state.decoders.instance_mut(id) {
            slot.setup = setup;
        }
        mark_dirty(state);
        if changes_results || current.state == InstanceState::Waiting {
            self.start_pass_keeping(state, id, true);
        }
        Ok(Executed::Changed(Changes::DECODERS))
    }

    pub(crate) fn remove_decoder(
        &mut self,
        state: &mut State,
        id: u32,
    ) -> Result<Executed, Reject> {
        if state.decoders.instance(id).is_none() {
            return Err(Reject::Ignored(crate::error::INVALID_COMMAND));
        }
        if let Some(job) = self.decoders.jobs.remove(&id) {
            job.cancel();
        }
        self.decoders.results.remove(&id);
        state.decoders.instances.retain(|i| i.id != id);
        state.presentation.forget_decoder(id);
        if state
            .decoders
            .selection
            .as_ref()
            .is_some_and(|s| s.instance == id)
        {
            state.decoders.selection = None;
        }
        state.decoders.refusal = None;
        mark_dirty(state);
        Ok(Executed::Changed(Changes::DECODERS))
    }

    /// The annotation `reference` of `instance`, with the text in the current language.
    fn annotation_of(&self, instance: u32, reference: AnnotationRef) -> Option<Annotation> {
        let results = self.decoders.results.get(&instance)?;
        let snapshot = match reference.target {
            Target::Lane => &results.lane,
            Target::Channel(c) => results.overlays.get(&c)?,
        };
        snapshot
            .find(reference.start, reference.id)
            .map(|e| e.annotation.clone())
    }

    fn details_of(&self, annotation: &Annotation, sample_rate_hz: u64) -> AnnotationDetails {
        let (start, end) = annotation.span();
        let kind = match annotation.kind {
            Kind::Range { .. } => "range",
            Kind::Point { .. } => "point",
            Kind::Edge { .. } => "edge",
        };
        let seconds = if sample_rate_hz == 0 {
            0.0
        } else {
            end.saturating_sub(start) as f64 / sample_rate_hz as f64
        };
        AnnotationDetails {
            kind,
            text: self.annotation_text(&annotation.text),
            start,
            end,
            duration: self.localizer.format_duration(seconds),
            error: annotation.error,
        }
    }

    /// The text of an annotation in the current language.
    #[must_use]
    pub fn annotation_text(&self, text: &Text) -> String {
        annotation_text(&self.localizer, text)
    }

    pub(crate) fn select_annotation(
        &mut self,
        state: &mut State,
        instance: u32,
        reference: AnnotationRef,
    ) -> Result<Executed, Reject> {
        let annotation = self
            .annotation_of(instance, reference)
            .ok_or(Reject::Ignored(crate::error::INVALID_COMMAND))?;
        let rate = state
            .capture
            .as_ref()
            .map_or(0, |c| c.snapshot.sample_rate_hz());
        state.decoders.selection = Some(Selection {
            instance,
            annotation: reference,
            details: self.details_of(&annotation, rate),
        });
        Ok(Executed::Changed(Changes::DECODERS))
    }

    /// Moves the window to the selected, next or previous annotation of the selected instance.
    pub(crate) fn go_to_annotation(
        &mut self,
        state: &mut State,
        which: AnnotationTarget,
    ) -> Result<Executed, Reject> {
        let selection = state
            .decoders
            .selection
            .clone()
            .ok_or(Reject::Ignored(crate::error::INVALID_COMMAND))?;
        let results = self
            .decoders
            .results
            .get(&selection.instance)
            .ok_or(Reject::Ignored(crate::error::INVALID_COMMAND))?;
        let snapshot = match selection.annotation.target {
            Target::Lane => &results.lane,
            Target::Channel(c) => results
                .overlays
                .get(&c)
                .ok_or(Reject::Ignored(crate::error::INVALID_COMMAND))?,
        };
        let mut probes = 0;
        let (target, entry) = match which {
            AnnotationTarget::Selected => (
                selection.annotation,
                snapshot.find(selection.annotation.start, selection.annotation.id),
            ),
            AnnotationTarget::Next => {
                let e = snapshot.next_after(selection.annotation.start, &mut probes);
                (selection.annotation, e)
            }
            AnnotationTarget::Previous => {
                let e = snapshot.prev_before(selection.annotation.start, &mut probes);
                (selection.annotation, e)
            }
        };
        let entry = entry.ok_or(Reject::Ignored(crate::error::INVALID_COMMAND))?;
        let reference = AnnotationRef {
            target: target.target,
            id: entry.id,
            start: entry.start(),
        };
        let annotation = entry.annotation.clone();
        let (start, end) = annotation.span();
        let limits = crate::shell::capture_limits_of(state)
            .ok_or(Reject::Ignored(crate::error::INVALID_COMMAND))?;
        let current = crate::shell::current_window(state, &limits);
        let width = end.saturating_sub(start).max(1);
        state.window = if width.saturating_mul(4) <= current.length.saturating_mul(3) {
            // It fits at the current zoom: centre it.
            let centre = start.saturating_add(width / 2);
            ViewWindow {
                start: centre.saturating_sub(current.length / 2),
                length: current.length,
            }
            .clamp(&limits)
        } else {
            let pad = (width / 8).saturating_add(1);
            ViewWindow::zoom_to(&limits, start.saturating_sub(pad), end.saturating_add(pad))
        };
        state.auto_fit = false;
        state.follow_latest = false;
        let rate = state
            .capture
            .as_ref()
            .map_or(0, |c| c.snapshot.sample_rate_hz());
        state.decoders.selection = Some(Selection {
            instance: selection.instance,
            annotation: reference,
            details: self.details_of(&annotation, rate),
        });
        Ok(Executed::Changed(Changes::DECODERS | Changes::WINDOW))
    }

    /// Follows the passes: progress, results and the end of each.
    pub(crate) fn poll_decoders(&mut self, state: &mut State) -> Changes {
        let mut changes = Changes::NONE;
        self.apply_pending_decoder_limit();
        changes.insert(self.poll_loading(state));
        changes.insert(self.start_queued(state));
        let ids: Vec<u32> = self.decoders.jobs.keys().copied().collect();
        for id in ids {
            let Some(job) = self.decoders.jobs.get_mut(&id) else {
                continue;
            };
            // The supervisor: a decoder that does not answer in time is abandoned; its thread is left
            // alone and nothing it still does is read.
            if let Some(why) = job.overdue() {
                tracing::error!(target: "la_host", ?why, "a decoder is not responding");
                if let Some(stream) = self.decoders.streams.get(&id) {
                    stream.end(false);
                }
                if let Some(stuck) = self.decoders.jobs.remove(&id) {
                    if let Some(latest) = stuck.latest() {
                        self.decoders.results.insert(id, latest);
                    }
                    stuck.abandon();
                }
                let error =
                    la_decoders::error::not_responding("the decoder did not answer in time");
                report_into(state, Severity::Error, &error);
                changes.insert(Changes::NOTICES | Changes::DECODERS);
                if let Some(view) = state.decoders.instance_mut(id) {
                    view.state = InstanceState::Failed;
                    view.fault = Some(error.code());
                    view.incomplete = true;
                }
                continue;
            }
            let progress = job.progress();
            let latest = job.latest();
            let ended = job.poll();
            if let Some(results) = latest {
                // The first results of a new pass replace the old ones at once.
                self.decoders.results.insert(id, results);
                if let Some(view) = state.decoders.instance_mut(id) {
                    view.outdated = false;
                }
            }
            let mut notice: Option<la_core::Error> = None;
            let finished = ended.is_some();
            let (new_state, fault, results) = match ended {
                None => (InstanceState::Running, None, None),
                Some(Ended::Done(end)) => {
                    let (new_state, fault) = match end.outcome {
                        Ok(Outcome::Complete) => (InstanceState::Finished, None),
                        Ok(Outcome::Cancelled) => (InstanceState::Cancelled, None),
                        Ok(Outcome::Limit(full)) => {
                            let error = match full {
                                Full::Count => {
                                    la_decoders::error::annotation_limit("annotation budget")
                                }
                                Full::Memory => la_decoders::error::memory_limit("results memory"),
                            };
                            let code = error.code();
                            notice = Some(error);
                            (InstanceState::Failed, Some(code))
                        }
                        Err(error) => {
                            let code = error.code();
                            notice = Some(error);
                            (InstanceState::Failed, Some(code))
                        }
                    };
                    (new_state, fault, Some(end.results))
                }
                Some(Ended::Panicked { failure, results }) => {
                    notice = Some(la_decoders::error::panicked(failure.to_string()));
                    (
                        InstanceState::Failed,
                        Some(la_decoders::error::PANICKED),
                        results,
                    )
                }
            };
            if let Some(results) = results {
                self.decoders.results.insert(id, results);
            }
            if finished {
                self.decoders.jobs.remove(&id);
            }
            if let Some(error) = notice {
                report_into(state, Severity::Error, &error);
                changes.insert(Changes::NOTICES);
            }
            let stored = self.decoders.results.get(&id);
            let annotations = stored.map_or(0, |r| {
                r.overlays
                    .values()
                    .fold(r.lane.len(), |sum, s| sum.saturating_add(s.len()))
            });
            let (incomplete, refused) =
                stored.map_or((false, 0), |r| (r.coverage.incomplete, r.coverage.refused));
            if let Some(view) = state.decoders.instance_mut(id) {
                let before = view.clone();
                view.state = new_state;
                view.covered = progress
                    .covered
                    .max(stored.map_or(0, |r| r.coverage.covered));
                view.length = progress.length;
                view.annotations = u64::try_from(annotations).unwrap_or(u64::MAX);
                view.refused = refused;
                view.incomplete = incomplete
                    || new_state == InstanceState::Cancelled
                    || new_state == InstanceState::Failed;
                if fault.is_some() {
                    view.fault = fault;
                }
                if *view != before {
                    changes.insert(Changes::DECODERS);
                }
            }
        }
        changes
    }

    /// Starts the instances that waited for a free decode thread.
    fn start_queued(&mut self, state: &mut State) -> Changes {
        let mut changes = Changes::NONE;
        while self.decoders.jobs.len() < self.decoders.thread_cap {
            let Some(id) = self.decoders.queue.pop_front() else {
                break;
            };
            let waiting = state
                .decoders
                .instance(id)
                .is_some_and(|i| i.state == InstanceState::Waiting);
            if waiting {
                self.start_pass_keeping(state, id, true);
                changes.insert(Changes::DECODERS);
            }
        }
        changes
    }

    /// Sets how many passes may run at once (tests).
    #[cfg(feature = "test-support")]
    pub fn set_decoder_thread_cap(&mut self, cap: usize) {
        self.decoders.thread_cap = cap.max(1);
    }

    /// Whether any pass is running.
    pub(crate) fn decoders_running(&self) -> bool {
        !self.decoders.jobs.is_empty()
            || !self.decoders.queue.is_empty()
            || self.decoders.loading.is_some()
    }

    /// The decoder instances whose lanes are shown, in stack order (as the frame plan lists them).
    pub(crate) fn shown_decoders(&self, state: &State) -> Vec<u32> {
        let setups: Vec<Setup> = state
            .decoders
            .instances
            .iter()
            .map(|i| i.setup.clone())
            .collect();
        let order = stack_order(&setups);
        let mut ordered: Vec<&InstanceView> = state.decoders.instances.iter().collect();
        ordered.sort_by_key(|i| order.iter().position(|o| *o == i.id));
        ordered
            .into_iter()
            .filter(|i| i.setup.lane_visible && self.decoders.results.contains_key(&i.id))
            .map(|i| i.id)
            .collect()
    }

    /// Moves a lane among the shown lanes (`Command::MoveLane`).
    pub(crate) fn move_lane(
        &mut self,
        state: &mut State,
        from: usize,
        to: usize,
    ) -> Result<Executed, Reject> {
        let channels = shown_channels(state);
        let decoders = self.shown_decoders(state);
        let slots: Vec<Option<u16>> = decoders
            .iter()
            .map(|id| state.presentation.decoder_slots.get(id).copied())
            .collect();
        let mut order = crate::plan::lane_order(channels.len(), &slots);
        if from == to || from >= order.len() || to >= order.len() {
            return Err(Reject::Ignored(crate::error::INVALID_COMMAND));
        }
        let moved = order.remove(from);
        order.insert(to, moved);
        // The shown channels take the places of the shown channels in the order of all of them (a
        // hidden channel stays where it is), in their new relative order.
        let new_channels: Vec<u8> = order
            .iter()
            .filter_map(|slot| match slot {
                crate::plan::LaneSlot::Channel(n) => channels.get(*n).copied(),
                crate::plan::LaneSlot::Decoder(_) => None,
            })
            .collect();
        let styles = &mut state.presentation.channels;
        let positions: Vec<usize> = styles
            .iter()
            .enumerate()
            .filter(|(_, s)| channels.contains(&s.index))
            .map(|(at, _)| at)
            .collect();
        let reordered: Vec<crate::presentation::ChannelStyle> = new_channels
            .iter()
            .filter_map(|c| styles.iter().find(|s| s.index == *c).cloned())
            .collect();
        for (at, style) in positions.into_iter().zip(reordered) {
            if let Some(place) = styles.get_mut(at) {
                *place = style;
            }
        }
        // Every shown decoder lane is pinned where it now is, so that the order is exactly this one.
        for (at, slot) in order.iter().enumerate() {
            if let crate::plan::LaneSlot::Decoder(n) = slot {
                if let Some(id) = decoders.get(*n) {
                    state
                        .presentation
                        .decoder_slots
                        .insert(*id, u16::try_from(at).unwrap_or(u16::MAX));
                }
            }
        }
        mark_dirty(state);
        Ok(Executed::Changed(Changes::PRESENTATION))
    }

    /// The lane plans and the overlay plans of the visible instances for a window.
    pub(crate) fn decoder_plans(
        &self,
        window: &ViewWindow,
        pixels: u32,
        shown_channels: &[u8],
    ) -> (Vec<LanePlan>, Vec<OverlayPlan>) {
        let view = lanes::Window {
            start: window.start,
            end: window.end(),
            columns: pixels,
        };
        let mut lane_plans = Vec::new();
        let mut overlay_plans = Vec::new();
        // The lanes come in stack order: every decoder below the ones that read from it.
        let setups: Vec<Setup> = self
            .state
            .decoders
            .instances
            .iter()
            .map(|i| i.setup.clone())
            .collect();
        let order = stack_order(&setups);
        let mut ordered: Vec<&InstanceView> = self.state.decoders.instances.iter().collect();
        ordered.sort_by_key(|i| order.iter().position(|o| *o == i.id));
        for instance in ordered {
            if !instance.setup.lane_visible {
                continue;
            }
            let Some(results) = self.decoders.results.get(&instance.id) else {
                continue;
            };
            let name = if instance.setup.name.is_empty() {
                self.state
                    .decoders
                    .catalogue
                    .iter()
                    .find(|c| c.id == instance.setup.decoder)
                    .map_or_else(String::new, |c| c.name.clone())
            } else {
                instance.setup.name.clone()
            };
            lane_plans.push(LanePlan {
                instance: instance.id,
                name,
                state: instance.state,
                incomplete: instance.incomplete,
                outdated: instance.outdated,
                plan: lanes::plan(
                    &results.lane,
                    &view,
                    self.decoders
                        .registry
                        .get(&instance.setup.decoder)
                        .map_or(LANE_ROWS, |d| usize::from(d.lane_rows)),
                ),
                height: self
                    .state
                    .presentation
                    .decoder_heights
                    .get(&instance.id)
                    .copied()
                    .unwrap_or(0),
                slot: self
                    .state
                    .presentation
                    .decoder_slots
                    .get(&instance.id)
                    .copied(),
                rows: self
                    .decoders
                    .registry
                    .get(&instance.setup.decoder)
                    .map_or(3, |d| d.lane_rows),
            });
            for channel in shown_channels {
                if let Some(snapshot) = results.overlays.get(channel) {
                    overlay_plans.push(OverlayPlan {
                        channel: *channel,
                        instance: instance.id,
                        plan: lanes::plan(snapshot, &view, OVERLAY_ROWS),
                    });
                }
            }
        }
        (lane_plans, overlay_plans)
    }

    /// Changes one instance's setup with `edit` and applies it like `ConfigureDecoder`.
    pub(crate) fn edit_decoder(
        &mut self,
        state: &mut State,
        id: u32,
        edit: impl FnOnce(&mut Setup),
    ) -> Result<Executed, Reject> {
        let mut setup = state
            .decoders
            .instance(id)
            .map(|i| i.setup.clone())
            .ok_or(Reject::Ignored(crate::error::INVALID_COMMAND))?;
        edit(&mut setup);
        self.configure_decoder(state, id, setup)
    }

    /// Registers one more decoder (tests: a decoder that the shell was not built with appears in
    /// the catalogue without any change to the shell).
    ///
    /// # Errors
    ///
    /// The descriptor is invalid or already registered.
    #[cfg(feature = "test-support")]
    pub fn register_decoder(
        &mut self,
        descriptor: &'static Descriptor,
        create: la_decoders::registry::Create,
    ) -> Result<(), la_core::Error> {
        self.decoders
            .registry
            .register_decoder(descriptor, create)?;
        self.state.decoders.catalogue = catalogue(&self.decoders.registry, &self.localizer);
        self.state.versions.bump(Changes::DECODERS);
        Ok(())
    }

    /// Sets the size of a chunk (tests: many small chunks make a fault happen part way).
    #[cfg(feature = "test-support")]
    pub fn set_decoder_chunk_limits(&mut self, limits: ChunkLimits) {
        self.decoders.chunk_limits = limits;
    }

    /// Sets the deadlines of the supervisor (tests: fail faster than the 5 s of a real run).
    #[cfg(feature = "test-support")]
    pub fn set_decoder_deadlines(&mut self, deadlines: la_decoders::run::Deadlines) {
        self.decoders.deadlines = deadlines;
    }

    /// The bytes the results of all decoders are counted as (for the tests of the limit).
    #[must_use]
    pub fn decoder_memory_used(&self) -> u64 {
        self.decoders.budget.used()
    }

    /// The results of an instance so far (a pass in progress has some), for the interface and
    /// the tests.
    #[must_use]
    pub fn decoder_results(&self, instance: u32) -> Option<&Results> {
        self.decoders.results.get(&instance)
    }

    /// The descriptor of an instance's decoder.
    #[must_use]
    pub fn decoder_descriptor(&self, instance: u32) -> Option<&'static Descriptor> {
        let view = self.state.decoders.instance(instance)?;
        self.decoders.registry.get(&view.setup.decoder)
    }
}

/// One instance's results as they come back from the worker that rebuilt them.
pub(crate) struct LoadedBlock {
    instance: u32,
    results: Results,
    stream: ItemStream,
    covered: u64,
    incomplete: bool,
    annotations: u64,
}

/// What the worker that checks and rebuilds saved results found.
pub(crate) enum LoadOutcome {
    /// The results of these instances.
    Loaded(Vec<LoadedBlock>),
    /// The section is not valid (the reason is for the log).
    Invalid(&'static str),
    /// The results do not fit the memory limit.
    TooBig,
}

impl Shell {
    /// Checks and rebuilds saved results on a worker; `poll` takes the outcome.
    pub(crate) fn start_loading_results(
        &mut self,
        created: Vec<u32>,
        all_ids: Vec<u32>,
        payload: Vec<u8>,
        bounds: crate::view_section::Bounds,
    ) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let budget = Arc::clone(&self.decoders.budget);
        // The worker is detached: a panic in it is contained and logged, and the closed channel
        // tells `poll` that nothing will come.
        let _ = la_diagnostics::spawn_worker("decoder-results-load", move || {
            let context = la_decoders::persist::dann::Context {
                capture_length: bounds.length,
                channels: bounds.channels,
                instances: &all_ids,
            };
            let outcome = match la_decoders::persist::dann::decode(&payload, &context) {
                Err(la_decoders::persist::Rejection::Invalid(why)) => LoadOutcome::Invalid(why.0),
                Err(la_decoders::persist::Rejection::Newer) => {
                    LoadOutcome::Invalid("newer version")
                }
                Ok(blocks) => {
                    let mut loaded = Vec::new();
                    let mut too_big = false;
                    for block in blocks.iter().filter(|b| created.contains(&b.instance)) {
                        match la_decoders::persist::results_of(block, &budget) {
                            Ok((results, stream)) => {
                                let annotations = results
                                    .overlays
                                    .values()
                                    .fold(results.lane.len(), |n, s| n.saturating_add(s.len()));
                                loaded.push(LoadedBlock {
                                    instance: block.instance,
                                    results,
                                    stream,
                                    covered: block.covered,
                                    incomplete: block.incomplete,
                                    annotations: u64::try_from(annotations).unwrap_or(u64::MAX),
                                });
                            }
                            Err(_) => too_big = true,
                        }
                    }
                    if too_big {
                        LoadOutcome::TooBig
                    } else {
                        LoadOutcome::Loaded(loaded)
                    }
                }
            };
            let _ = sender.send(outcome);
        });
        self.decoders.loading = Some(receiver);
    }

    /// Takes the result of the worker that rebuilds saved results, when it is there.
    pub(crate) fn poll_loading(&mut self, state: &mut State) -> Changes {
        use std::sync::mpsc::TryRecvError;
        let Some(receiver) = &self.decoders.loading else {
            return Changes::NONE;
        };
        let outcome = match receiver.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return Changes::NONE,
            Err(TryRecvError::Disconnected) => {
                LoadOutcome::Invalid("the worker ended without a result")
            }
        };
        self.decoders.loading = None;
        let mut changes = Changes::DECODERS;
        let mut loaded_ids = Vec::new();
        match outcome {
            LoadOutcome::Loaded(blocks) => {
                for block in blocks {
                    // Something the user did meanwhile (a new pass) wins over saved results.
                    let waiting = state
                        .decoders
                        .instance(block.instance)
                        .is_some_and(|v| v.state == InstanceState::Waiting);
                    if !waiting {
                        continue;
                    }
                    self.decoders
                        .install_loaded(block.instance, block.results, block.stream);
                    if let Some(view) = state.decoders.instance_mut(block.instance) {
                        view.state = InstanceState::Finished;
                        view.ready = true;
                        view.loaded = true;
                        view.covered = block.covered;
                        view.incomplete = block.incomplete;
                        view.annotations = block.annotations;
                    }
                    loaded_ids.push(block.instance);
                }
            }
            LoadOutcome::Invalid(why) => {
                tracing::warn!(target: "la_host", why, "the saved decoder results are not valid");
                let error = la_decoders::error::section_invalid(format!("DANN: {why}"));
                report_into(state, Severity::Warning, &error);
                changes.insert(Changes::NOTICES);
            }
            LoadOutcome::TooBig => {
                let error = la_decoders::error::memory_limit(
                    "the saved results do not fit the memory limit",
                );
                report_into(state, Severity::Warning, &error);
                changes.insert(Changes::NOTICES);
            }
        }
        self.start_idle_instances(state, &loaded_ids);
        changes
    }

    /// Starts the instances that have no results (they decode the capture again), lowest first.
    pub(crate) fn start_idle_instances(&mut self, state: &mut State, loaded: &[u32]) {
        let setups: Vec<Setup> = state
            .decoders
            .instances
            .iter()
            .map(|i| i.setup.clone())
            .collect();
        for id in stack_order(&setups) {
            let idle = state
                .decoders
                .instance(id)
                .is_some_and(|i| !loaded.contains(&id) && i.state == InstanceState::Waiting);
            if idle {
                self.start_pass(state, id);
            }
        }
    }
}
