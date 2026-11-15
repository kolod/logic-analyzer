//! Stable error codes and constructors of the decoders.
//!
//! Every fallible decoder operation returns [`la_core::Error`]. The codes below are stable
//! identifiers: the localization layer maps them to the user-facing text (`errors.ftl`), and tests
//! compare against them. `detail` is English text for the log only and never contains signal data,
//! annotation text or user text.

use la_core::{Error, ErrorKind};

/// A decoder returned an error; what it produced before is kept, marked incomplete.
pub const FAILED: &str = "decoder.failed";
/// A decoder panicked; what it produced before is kept, marked incomplete.
pub const PANICKED: &str = "decoder.panicked";
/// A chunk or a cancel exceeded its deadline; the worker was abandoned.
pub const NOT_RESPONDING: &str = "decoder.not_responding";
/// The results memory limit was reached.
pub const MEMORY_LIMIT: &str = "decoder.memory_limit";
/// The annotation count budget of an instance was reached.
pub const ANNOTATION_LIMIT: &str = "decoder.annotation_limit";
/// Some annotations or items were refused (counted, reported once per pass).
pub const INVALID_OUTPUT: &str = "decoder.invalid_output";
/// A channel or option choice was refused before any run.
pub const INVALID_SETUP: &str = "decoder.invalid_setup";
/// A stacking choice would close a cycle.
pub const STACK_CYCLE: &str = "decoder.stack_cycle";
/// A stacking choice joins decoders whose item kinds do not fit.
pub const STACK_INCOMPATIBLE: &str = "decoder.stack_incompatible";
/// A stacking choice would make a chain deeper than allowed.
pub const STACK_TOO_DEEP: &str = "decoder.stack_too_deep";
/// The limit of decoder instances per capture was reached.
pub const TOO_MANY_INSTANCES: &str = "decoder.too_many_instances";
/// Too many stuck workers were abandoned; new passes are refused.
pub const TOO_MANY_STUCK: &str = "decoder.too_many_stuck";
/// The decoder section of a session file is invalid; the valid part opens.
pub const SECTION_INVALID: &str = "decoder.section_invalid";
/// A saved decoder is not known to this version of the application.
pub const UNKNOWN_DECODER: &str = "decoder.unknown_decoder";
/// Some saved options of a decoder were not understood and were skipped.
pub const OPTIONS_SKIPPED: &str = "decoder.options_skipped";

/// Every code of this module, for the completeness checks.
pub const ALL_CODES: [&str; 15] = [
    FAILED,
    PANICKED,
    NOT_RESPONDING,
    MEMORY_LIMIT,
    ANNOTATION_LIMIT,
    INVALID_OUTPUT,
    INVALID_SETUP,
    STACK_CYCLE,
    STACK_INCOMPATIBLE,
    STACK_TOO_DEEP,
    TOO_MANY_INSTANCES,
    TOO_MANY_STUCK,
    SECTION_INVALID,
    UNKNOWN_DECODER,
    OPTIONS_SKIPPED,
];

macro_rules! constructor {
    ($(#[$doc:meta])* $name:ident, $kind:ident, $code:ident) => {
        $(#[$doc])*
        #[must_use]
        pub fn $name(detail: impl Into<String>) -> Error {
            Error::new(ErrorKind::$kind, $code, detail)
        }
    };
}

constructor!(
    /// A decoder returned an error.
    failed,
    Internal,
    FAILED
);
constructor!(
    /// A decoder panicked.
    panicked,
    Internal,
    PANICKED
);
constructor!(
    /// A decoder did not answer in time.
    not_responding,
    Internal,
    NOT_RESPONDING
);
constructor!(
    /// The results memory limit was reached.
    memory_limit,
    Resource,
    MEMORY_LIMIT
);
constructor!(
    /// The annotation budget was reached.
    annotation_limit,
    Resource,
    ANNOTATION_LIMIT
);
constructor!(
    /// Some output was refused.
    invalid_output,
    Internal,
    INVALID_OUTPUT
);
constructor!(
    /// A setup choice was refused.
    invalid_setup,
    InvalidInput,
    INVALID_SETUP
);
constructor!(
    /// A stacking choice would close a cycle.
    stack_cycle,
    InvalidInput,
    STACK_CYCLE
);
constructor!(
    /// A stacking choice joins incompatible decoders.
    stack_incompatible,
    InvalidInput,
    STACK_INCOMPATIBLE
);
constructor!(
    /// A stacking choice is too deep.
    stack_too_deep,
    InvalidInput,
    STACK_TOO_DEEP
);
constructor!(
    /// Too many decoder instances.
    too_many_instances,
    Resource,
    TOO_MANY_INSTANCES
);
constructor!(
    /// Too many stuck workers.
    too_many_stuck,
    Resource,
    TOO_MANY_STUCK
);
constructor!(
    /// The saved decoder section is invalid.
    section_invalid,
    Corrupt,
    SECTION_INVALID
);
constructor!(
    /// A saved decoder is unknown.
    unknown_decoder,
    Corrupt,
    UNKNOWN_DECODER
);
constructor!(
    /// Some saved options were skipped.
    options_skipped,
    Corrupt,
    OPTIONS_SKIPPED
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_in_the_decoder_domain() {
        let mut codes = ALL_CODES.to_vec();
        assert!(codes.iter().all(|c| c.starts_with("decoder.")));
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), 15);
    }

    #[test]
    fn constructors_use_their_codes_and_kinds() {
        assert_eq!(failed("x").code(), FAILED);
        assert_eq!(panicked("x").kind(), ErrorKind::Internal);
        assert_eq!(memory_limit("x").kind(), ErrorKind::Resource);
        assert_eq!(invalid_setup("x").kind(), ErrorKind::InvalidInput);
        assert_eq!(section_invalid("x").kind(), ErrorKind::Corrupt);
        assert_eq!(options_skipped("x").code(), OPTIONS_SKIPPED);
    }
}
