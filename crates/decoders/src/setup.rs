//! What the user chooses for one decoder instance, and the rules a choice must obey
//! (`data-model.md`, "Decoder setup").

use std::collections::BTreeMap;

use la_core::Error;

use crate::descriptor::{Descriptor, Input, OptionKind};
use crate::error;
use crate::registry::Registry;
use crate::text::sanitize;

/// The most decoder instances in one capture.
pub const MAX_INSTANCES: usize = 8;
/// The most instances in one chain of stacked decoders.
pub const MAX_STACK_DEPTH: usize = 4;
/// The longest instance name, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// The id of a decoder instance, unique in a session and never reused.
pub type InstanceId = u32;

/// Where an instance reads from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The channels of the capture.
    Capture,
    /// The output items of another instance.
    Instance(InstanceId),
}

/// The value of an option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// The id of one choice.
    Choice(String),
    /// A whole number.
    Integer(i64),
    /// A switch.
    Boolean(bool),
}

/// The choices for one instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setup {
    /// The instance id.
    pub instance: InstanceId,
    /// The id of the decoder.
    pub decoder: String,
    /// The instance name (empty means the localized default of the decoder).
    pub name: String,
    /// Where it reads from.
    pub source: Source,
    /// The channel of each signal role; a role without an entry has no channel.
    pub channels: BTreeMap<String, u8>,
    /// The value of each option; a missing option takes its default.
    pub options: BTreeMap<String, Value>,
    /// Whether the lane is shown.
    pub lane_visible: bool,
    /// The options of the generator of the decoder, kept with the instance.
    pub generator: BTreeMap<String, i64>,
}

impl Setup {
    /// A setup of `decoder` reading the capture, with nothing chosen yet.
    #[must_use]
    pub fn new(instance: InstanceId, decoder: impl Into<String>) -> Self {
        Self {
            instance,
            decoder: decoder.into(),
            name: String::new(),
            source: Source::Capture,
            channels: BTreeMap::new(),
            options: BTreeMap::new(),
            lane_visible: true,
            generator: BTreeMap::new(),
        }
    }

    /// The value of an option: the chosen one, or the default of the descriptor.
    #[must_use]
    pub fn option_value(&self, descriptor: &Descriptor, id: &str) -> Option<Value> {
        let spec = descriptor.option(id)?;
        if let Some(value) = self.options.get(id) {
            return Some(value.clone());
        }
        Some(match spec.kind {
            OptionKind::Choice { default, .. } => Value::Choice(default.to_owned()),
            OptionKind::Integer { default, .. } => Value::Integer(default),
            OptionKind::Boolean { default } => Value::Boolean(default),
        })
    }

    /// Checks the setup against the descriptor, the capture and the other instances.
    ///
    /// `others` are the setups of the other instances of the capture (an entry with the same
    /// instance id is ignored, so the old version of this setup may be left in); `channels` is the
    /// number of channels of the capture (or of the device before a capture exists).
    ///
    /// # Errors
    ///
    /// The first rule that is broken, with a code of the `decoder` domain; the detail names the
    /// setting (`name`, `source`, `channels.<role>`, `options.<id>`).
    pub fn validate(
        &self,
        registry: &Registry,
        others: &[Setup],
        channels: u8,
    ) -> Result<(), Error> {
        let others: Vec<&Setup> = others
            .iter()
            .filter(|o| o.instance != self.instance)
            .collect();
        if others.len() >= MAX_INSTANCES {
            return Err(error::too_many_instances(format!(
                "{MAX_INSTANCES} instances already exist"
            )));
        }
        let Some(descriptor) = registry.get(&self.decoder) else {
            return Err(refused("decoder", "unknown decoder"));
        };
        if sanitize(&self.name).chars().count() > MAX_NAME_CHARS {
            return Err(refused("name", "longer than the limit"));
        }
        self.validate_channels(descriptor, channels)?;
        self.validate_options(descriptor)?;
        self.validate_source(registry, descriptor, &others)
    }

    fn validate_channels(&self, descriptor: &Descriptor, channels: u8) -> Result<(), Error> {
        for (role, channel) in &self.channels {
            let setting = format!("channels.{role}");
            if descriptor.signal(role).is_none() {
                return Err(refused(&setting, "unknown signal"));
            }
            if *channel >= channels {
                return Err(refused(&setting, "the channel does not exist"));
            }
        }
        for signal in descriptor.signals {
            let setting = format!("channels.{}", signal.role);
            match self.channels.get(signal.role) {
                None if signal.required => return Err(refused(&setting, "a channel is required")),
                None => {}
                Some(channel) => {
                    let shared = self.channels.iter().any(|(other, c)| {
                        other != signal.role
                            && c == channel
                            && !(signal.may_share
                                && descriptor.signal(other).is_some_and(|s| s.may_share))
                    });
                    if shared {
                        return Err(refused(&setting, "the channel is used by another signal"));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_options(&self, descriptor: &Descriptor) -> Result<(), Error> {
        for (id, value) in &self.options {
            let setting = format!("options.{id}");
            let Some(spec) = descriptor.option(id) else {
                return Err(refused(&setting, "unknown option"));
            };
            let fits = match (spec.kind, value) {
                (OptionKind::Choice { choices, .. }, Value::Choice(v)) => {
                    choices.iter().any(|(id, _)| *id == v)
                }
                (OptionKind::Integer { min, max, .. }, Value::Integer(v)) => *v >= min && *v <= max,
                (OptionKind::Boolean { .. }, Value::Boolean(_)) => true,
                _ => false,
            };
            if !fits {
                return Err(refused(
                    &setting,
                    "the value is of the wrong type or out of range",
                ));
            }
        }
        Ok(())
    }

    fn validate_source(
        &self,
        registry: &Registry,
        descriptor: &Descriptor,
        others: &[&Setup],
    ) -> Result<(), Error> {
        let find = |id: InstanceId| others.iter().copied().find(|o| o.instance == id);
        match (descriptor.input, self.source) {
            (Input::Signals, Source::Capture) => Ok(()),
            (Input::Signals, Source::Instance(_)) => Err(error::stack_incompatible(
                "source: this decoder reads channels, not another decoder",
            )),
            (Input::Items(_), Source::Capture) => Err(error::stack_incompatible(
                "source: this decoder reads another decoder, not channels",
            )),
            (Input::Items(kind), Source::Instance(first)) => {
                if first == self.instance {
                    return Err(error::stack_cycle("source: an instance cannot read itself"));
                }
                let Some(lower) = find(first) else {
                    return Err(refused("source", "the instance does not exist"));
                };
                let produces = registry
                    .get(&lower.decoder)
                    .is_some_and(|d| d.outputs.contains(&kind));
                if !produces {
                    return Err(error::stack_incompatible(format!(
                        "source: instance {first} does not produce `{kind}`"
                    )));
                }
                // Walk down the chain: reaching this instance again is a cycle.
                let mut below = 1_usize;
                let mut current = lower;
                while let Source::Instance(next) = current.source {
                    if next == self.instance {
                        return Err(error::stack_cycle("source: the choice closes a cycle"));
                    }
                    let Some(next_setup) = find(next) else { break };
                    below = below.saturating_add(1);
                    if below > others.len() {
                        return Err(error::stack_cycle("source: the chain loops"));
                    }
                    current = next_setup;
                }
                let above = height_above(self.instance, others, 0);
                let total = below.saturating_add(1).saturating_add(above);
                if total > MAX_STACK_DEPTH {
                    return Err(error::stack_too_deep(format!(
                        "source: the chain would have {total} instances, at most {MAX_STACK_DEPTH}"
                    )));
                }
                Ok(())
            }
        }
    }
}

/// How many instances are stacked above `id` along the longest chain (0 when none).
fn height_above(id: InstanceId, others: &[&Setup], depth: usize) -> usize {
    if depth > MAX_INSTANCES {
        return 0;
    }
    others
        .iter()
        .filter(|o| o.source == Source::Instance(id))
        .map(|o| height_above(o.instance, others, depth.saturating_add(1)).saturating_add(1))
        .max()
        .unwrap_or(0)
}

fn refused(setting: &str, why: &str) -> Error {
    error::invalid_setup(format!("{setting}: {why}"))
}
