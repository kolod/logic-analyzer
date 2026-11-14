//! Storage of captured digital signals with fast viewport queries.
//!
//! The store keeps every channel as its transitions in immutable blocks, so memory
//! depends on the number of level changes, not on the duration of the capture, and a
//! viewport query costs a number of probes that is polylogarithmic in the capture size
//! (see `specs/002-signal-storage-core`).
//!
//! **Never log signal data.** Log only metadata such as sizes, counts and positions.
