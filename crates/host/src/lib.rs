//! Host API: everything a user interface needs from the core.
//!
//! The shell state, the commands that change it, the frame plan that reduces a capture to a few
//! segments per channel, the settings file and the localized notices live here, free of any UI
//! toolkit, so the whole behaviour of the application can be tested without a window. A user
//! interface sends [`Command`]s and reads the state; it never calls the storage, device or session
//! crates directly. The contract is `docs/host-api.md` (design source:
//! `specs/006-ui-shell-signal-view/contracts/host-api.md`).
//!
//! **Never log signal data or user text.** Log command names, codes, counts and file names.

#![deny(missing_docs)]

/// Version of the host API contract (`docs/host-api.md`). A change to commands, state fields,
/// events or their meaning changes this value together with the document.
pub const HOST_API_VERSION: &str = "1.0";
