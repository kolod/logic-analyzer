//! The static description of a decoder (`data-model.md`, "Decoder descriptor").
//!
//! A descriptor is plain immutable data, defined once by the decoder. The application builds the
//! list of decoders, the configuration dialog, the lane and the saved setup from it, so a new
//! decoder needs nothing else (spec SC-012). Texts are referred to by static message keys.

/// The most rows a decoder's lane can have.
pub const MAX_LANE_ROWS: usize = 3;
/// The most signal roles a decoder can have.
pub const MAX_SIGNALS: usize = 16;
/// The most options a decoder can have.
pub const MAX_OPTIONS: usize = 16;
/// The longest identifier (decoder id, role, option, choice value), in characters.
pub const MAX_ID_LEN: usize = 48;

/// What a decoder reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// The channels of the capture.
    Signals,
    /// The output items of this kind of another instance (stacking).
    Items(&'static str),
}

/// One signal role of a decoder (a pin it needs, such as a clock or a data line).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signal {
    /// The role, unique in the decoder.
    pub role: &'static str,
    /// The message key of the label.
    pub label: &'static str,
    /// Whether a channel must be assigned.
    pub required: bool,
    /// Whether this role may use a channel another role also uses.
    pub may_share: bool,
}

/// The type of an option and the rules of its values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionKind {
    /// One of a fixed list of `(value id, message key of the label)`.
    Choice {
        /// The choices; not empty.
        choices: &'static [(&'static str, &'static str)],
        /// The default; one of the value ids.
        default: &'static str,
    },
    /// A whole number between `min` and `max`, both inclusive.
    Integer {
        /// The smallest value.
        min: i64,
        /// The largest value.
        max: i64,
        /// The default, inside the range.
        default: i64,
    },
    /// A switch.
    Boolean {
        /// The default.
        default: bool,
    },
}

/// One option of a decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionSpec {
    /// The identifier, unique in the decoder.
    pub id: &'static str,
    /// The message key of the label.
    pub label: &'static str,
    /// The type and rules.
    pub kind: OptionKind,
}

/// What a decoder says about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Descriptor {
    /// The stable identifier of the decoder.
    pub id: &'static str,
    /// Bumped when saved options or behaviour change incompatibly; starts at 1.
    pub version: u16,
    /// The message key of the name.
    pub name: &'static str,
    /// The message key of the description.
    pub description: &'static str,
    /// The signal roles, in the order they are shown.
    pub signals: &'static [Signal],
    /// The options, in the order they are shown.
    pub options: &'static [OptionSpec],
    /// What the decoder reads.
    pub input: Input,
    /// The kinds of output item it produces.
    pub outputs: &'static [&'static str],
    /// Whether a generator exists for it.
    pub generator: bool,
    /// The rows of spans and marks of its lane, 1 to [`MAX_LANE_ROWS`] (decoder API 1.2); a decoder that
    /// draws blocks of the two bands says 2.
    pub lane_rows: u8,
}

/// Whether `text` is a well-formed identifier: ASCII lowercase letters, digits, `.` and `_`, at
/// most [`MAX_ID_LEN`] characters, not empty.
#[must_use]
pub fn is_identifier(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_ID_LEN
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
}

fn unique<'a>(mut items: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    items.all(|item| seen.insert(item))
}

impl OptionSpec {
    fn validate(&self) -> Result<(), String> {
        if !is_identifier(self.id) {
            return Err(format!("option id `{}` is not an identifier", self.id));
        }
        match self.kind {
            OptionKind::Choice { choices, default } => {
                if choices.is_empty() {
                    return Err(format!("option `{}` has no choices", self.id));
                }
                if choices.iter().any(|(value, _)| !is_identifier(value)) {
                    return Err(format!("option `{}` has a malformed choice id", self.id));
                }
                if !unique(choices.iter().map(|(value, _)| *value)) {
                    return Err(format!("option `{}` repeats a choice", self.id));
                }
                if !choices.iter().any(|(value, _)| *value == default) {
                    return Err(format!("option `{}` default is not a choice", self.id));
                }
            }
            OptionKind::Integer { min, max, default } => {
                if min > max || default < min || default > max {
                    return Err(format!("option `{}` default is outside its range", self.id));
                }
            }
            OptionKind::Boolean { .. } => {}
        }
        Ok(())
    }
}

impl Descriptor {
    /// Checks the descriptor: identifiers well formed and unique, defaults inside their ranges,
    /// counts within the limits, a first version.
    ///
    /// # Errors
    ///
    /// A short English reason for the log (a descriptor that fails is a developer error).
    pub fn validate(&self) -> Result<(), String> {
        if !is_identifier(self.id) {
            return Err(format!("decoder id `{}` is not an identifier", self.id));
        }
        if self.version == 0 {
            return Err("version starts at 1".to_owned());
        }
        if self.lane_rows == 0 || usize::from(self.lane_rows) > MAX_LANE_ROWS {
            return Err(format!("the rows of the lane are 1 to {MAX_LANE_ROWS}"));
        }
        if self.signals.len() > MAX_SIGNALS {
            return Err(format!("more than {MAX_SIGNALS} signals"));
        }
        if self.options.len() > MAX_OPTIONS {
            return Err(format!("more than {MAX_OPTIONS} options"));
        }
        if self.signals.iter().any(|s| !is_identifier(s.role)) {
            return Err("a signal role is not an identifier".to_owned());
        }
        if !unique(self.signals.iter().map(|s| s.role)) {
            return Err("a signal role is repeated".to_owned());
        }
        if !unique(self.options.iter().map(|o| o.id)) {
            return Err("an option id is repeated".to_owned());
        }
        for option in self.options {
            option.validate()?;
        }
        if let Input::Items(kind) = self.input {
            if !is_identifier(kind) {
                return Err("the input item kind is not an identifier".to_owned());
            }
            if !self.signals.is_empty() {
                return Err("a decoder that reads items has no signals".to_owned());
            }
        }
        if self.outputs.iter().any(|kind| !is_identifier(kind))
            || !unique(self.outputs.iter().copied())
        {
            return Err("an output item kind is malformed or repeated".to_owned());
        }
        Ok(())
    }

    /// The signal spec of a role.
    #[must_use]
    pub fn signal(&self, role: &str) -> Option<&'static Signal> {
        self.signals.iter().find(|s| s.role == role)
    }

    /// The option spec with this id.
    #[must_use]
    pub fn option(&self, id: &str) -> Option<&'static OptionSpec> {
        self.options.iter().find(|o| o.id == id)
    }
}
