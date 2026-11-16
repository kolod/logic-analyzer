//! The decoder sections of a session file: building `DECO` and `DANN` when saving, and applying
//! them when opening (`contracts/deco-section.md`, `docs/host-api.md`).
//!
//! A bad section never fails an open: the capture opens, the user gets a notice and the invalid
//! section is dropped (it is not written back). Records of decoders this version does not know are
//! kept as the bytes they came in and written back untouched.

use la_core::Error;
use la_decoders::persist::dann::{self, Block};
use la_decoders::persist::deco::{self, Record};
use la_decoders::persist::{Rejection, block_of, results_of};
use la_decoders::setup::{Setup, Source};
use la_decoders::stack::stack_order;
use la_session::{Section, SectionTag};

use crate::decoders::{InstanceState, InstanceView};
use crate::notices::{NoticeContent, Severity};
use crate::shell::{Shell, now_seconds, report_into};
use crate::state::State;
use crate::view_section::Bounds;

/// The tag of the setup section.
pub const DECO_TAG: &str = "DECO";
/// The tag of the results section.
pub const DANN_TAG: &str = "DANN";

/// A record of `DECO` that is written back as it was read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Kept {
    /// The instance id of the record.
    pub id: u32,
    /// Where the record was among the records of the file.
    pub position: usize,
    /// The content of the record.
    pub raw: Vec<u8>,
    /// For a record that was understood but has options this version does not know: the setup it
    /// was read as (the raw bytes are written back while the instance has not been changed).
    pub setup: Option<Setup>,
}

fn tag(text: &str) -> Option<SectionTag> {
    SectionTag::new(text).ok()
}

impl Shell {
    /// The sections to save for the instances on screen: `DECO` when there are records, `DANN`
    /// when the user chose to save results and some exist. A section of a newer version that was
    /// kept from the opened file is not replaced (the caller leaves ours out then).
    pub(crate) fn decoder_sections(&self, state: &State) -> Result<Vec<Section>, Error> {
        let instances = &state.decoders.instances;
        if instances.is_empty() && state.decoders.kept.is_empty() {
            return Ok(Vec::new());
        }
        let setups: Vec<Setup> = instances.iter().map(|i| i.setup.clone()).collect();
        let order = stack_order(&setups);
        let mut records: Vec<Vec<u8>> = Vec::new();
        for id in &order {
            let Some(instance) = state.decoders.instance(*id) else {
                continue;
            };
            let version = self
                .decoders
                .registry
                .get(&instance.setup.decoder)
                .map_or(1, |d| d.version);
            let unchanged = state
                .decoders
                .kept
                .iter()
                .find(|k| k.id == *id)
                .filter(|k| k.setup.as_ref() == Some(&instance.setup));
            records.push(match unchanged {
                Some(kept) => kept.raw.clone(),
                None => deco::encode_instance(&instance.setup, version),
            });
        }
        let mut unknown: Vec<&Kept> = state
            .decoders
            .kept
            .iter()
            .filter(|k| k.setup.is_none())
            .collect();
        unknown.sort_by_key(|k| k.position);
        for kept in unknown {
            let at = kept.position.min(records.len());
            records.insert(at, kept.raw.clone());
        }
        let mut sections = Vec::new();
        if let Some(tag) = tag(DECO_TAG) {
            sections.push(Section {
                tag,
                critical: false,
                payload: deco::assemble(&records),
            });
        }
        if state.decoders.save_results {
            let blocks: Vec<Block> = order
                .iter()
                .filter_map(|id| {
                    let results = self.decoders.results.get(id)?;
                    let view = state.decoders.instance(*id)?;
                    let items = self
                        .decoders
                        .streams
                        .get(id)
                        .map(la_decoders::stack::ItemStream::items)
                        .unwrap_or_default();
                    Some(block_of(results, items, *id, 1, view.incomplete))
                })
                .collect();
            if !blocks.is_empty() {
                if let Some(tag) = tag(DANN_TAG) {
                    sections.push(Section {
                        tag,
                        critical: false,
                        payload: dann::encode(&blocks),
                    });
                }
            }
        }
        Ok(sections)
    }

    /// Restores the decoders of an opened session. Returns the sections that stay in the session
    /// as they are (a newer version's) and whether notices were added.
    pub(crate) fn apply_decoder_sections(
        &mut self,
        state: &mut State,
        deco_section: Option<&Section>,
        dann_section: Option<&Section>,
        bounds: Bounds,
    ) -> (Vec<Section>, bool) {
        self.clear_decoder_runtime();
        let mut keep: Vec<Section> = Vec::new();
        let mut notices = false;
        let warn = |state: &mut State, error: &Error| {
            report_into(state, Severity::Warning, error);
        };
        let Some(section) = deco_section else {
            return (keep, notices);
        };
        let context = deco::Context {
            registry: &self.decoders.registry,
            channels: bounds.channels,
        };
        let records: Vec<Record> = match deco::decode(&section.payload, &context) {
            Ok(records) => records,
            Err(Rejection::Newer) => {
                keep.push(section.clone());
                if let Some(dann) = dann_section {
                    keep.push(dann.clone());
                }
                state.notices.add(
                    Severity::Info,
                    NoticeContent::Info {
                        key: "notice-decoders-newer",
                        text: None,
                    },
                    now_seconds(),
                );
                return (keep, true);
            }
            Err(Rejection::Invalid(why)) => {
                warn(
                    state,
                    &la_decoders::error::section_invalid(format!("DECO: {}", why.0)),
                );
                return (keep, true);
            }
        };
        let mut created: Vec<u32> = Vec::new();
        let mut unknown_reported = false;
        let mut skipped_reported = false;
        let mut all_ids: Vec<u32> = Vec::new();
        for (position, record) in records.into_iter().enumerate() {
            all_ids.push(record.id);
            let usable = record.parsed.filter(|parsed| match parsed.setup.source {
                Source::Capture => true,
                Source::Instance(below) => created.contains(&below),
            });
            let Some(parsed) = usable else {
                if !unknown_reported {
                    unknown_reported = true;
                    notices = true;
                    warn(
                        state,
                        &la_decoders::error::unknown_decoder("a saved decoder is not available"),
                    );
                }
                state.decoders.kept.push(Kept {
                    id: record.id,
                    position,
                    raw: record.raw,
                    setup: None,
                });
                continue;
            };
            if parsed.skipped_options > 0 {
                if !skipped_reported {
                    skipped_reported = true;
                    notices = true;
                    warn(
                        state,
                        &la_decoders::error::options_skipped("saved options were not understood"),
                    );
                }
                state.decoders.kept.push(Kept {
                    id: record.id,
                    position,
                    raw: record.raw,
                    setup: Some(parsed.setup.clone()),
                });
            }
            created.push(record.id);
            state.decoders.instances.push(InstanceView {
                id: record.id,
                setup: parsed.setup,
                state: InstanceState::Waiting,
                ready: false,
                covered: 0,
                length: bounds.length,
                incomplete: false,
                outdated: false,
                loaded: false,
                fault: None,
                annotations: 0,
                refused: 0,
            });
        }
        state.decoders.next_id = all_ids
            .iter()
            .copied()
            .max()
            .map_or(1, |m| m.saturating_add(1));
        // The results, when the file has them and they are good.
        let mut loaded: Vec<u32> = Vec::new();
        if let Some(dann_section) = dann_section {
            let context = dann::Context {
                capture_length: bounds.length,
                channels: bounds.channels,
                instances: &all_ids,
            };
            match dann::decode(&dann_section.payload, &context) {
                Ok(blocks) => {
                    for block in blocks {
                        if !created.contains(&block.instance) {
                            continue;
                        }
                        match results_of(&block, &self.decoders.budget) {
                            Ok((results, stream)) => {
                                let annotations = u64::try_from(
                                    results
                                        .overlays
                                        .values()
                                        .fold(results.lane.len(), |n, s| n.saturating_add(s.len())),
                                )
                                .unwrap_or(u64::MAX);
                                self.decoders
                                    .install_loaded(block.instance, results, stream);
                                if let Some(view) = state.decoders.instance_mut(block.instance) {
                                    view.state = InstanceState::Finished;
                                    view.ready = true;
                                    view.loaded = true;
                                    view.covered = block.covered;
                                    view.incomplete = block.incomplete;
                                    view.annotations = annotations;
                                }
                                loaded.push(block.instance);
                            }
                            Err(_) => {
                                notices = true;
                                warn(
                                    state,
                                    &la_decoders::error::memory_limit(
                                        "the saved results do not fit the memory limit",
                                    ),
                                );
                            }
                        }
                    }
                }
                Err(Rejection::Newer) => {
                    keep.push(dann_section.clone());
                    state.notices.add(
                        Severity::Info,
                        NoticeContent::Info {
                            key: "notice-decoders-newer",
                            text: None,
                        },
                        now_seconds(),
                    );
                    notices = true;
                }
                Err(Rejection::Invalid(why)) => {
                    notices = true;
                    warn(
                        state,
                        &la_decoders::error::section_invalid(format!("DANN: {}", why.0)),
                    );
                }
            }
        }
        // The instances without saved results decode the capture again.
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
        (keep, notices)
    }
}
