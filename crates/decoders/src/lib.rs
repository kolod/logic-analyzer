//! Protocol decoder framework.
//!
//! A decoder reads channels of a capture, or the output of another decoder, and produces
//! annotations: ranges, points and edge markers placed in the decoder's own lane or as an
//! overlay on a channel. The framework runs decoders in the background in chunks, contains every
//! way they can fail, indexes their results so that drawing costs the same on any capture, and
//! stores the setup and results in optional sections of the session file. See
//! `specs/007-decoder-framework-annotations` and `docs/decoder-api.md`.
//!
//! **Never log signal data or the text of annotations.** Log only decoder ids, counts, positions
//! and codes.

#![deny(missing_docs)]

// Used by the generators (T036); declared now so the crate graph is final.
use la_devices as _;

pub mod decoders;
pub mod descriptor;
pub mod error;
pub mod generators;
pub mod input;
pub mod lanes;
pub mod model;
pub mod persist;
pub mod registry;
pub mod run;
pub mod setup;
pub mod stack;
pub mod store;
#[cfg(feature = "test-support")]
pub mod testing;
pub mod text;

/// Version of the decoder API contract (`docs/decoder-api.md`). A change to the interface a
/// decoder is written against changes this version and the document in the same merge.
pub const DECODER_API_VERSION: &str = "1.2";
