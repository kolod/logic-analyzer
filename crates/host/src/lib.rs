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

pub mod axis;
pub mod capture;
mod command;
mod decoder_sessions;
pub mod decoders;
pub mod devices;
pub mod error;
pub mod layout_section;
pub mod markers;
pub mod notices;
pub mod plan;
pub mod presentation;
pub mod sanitize;
pub mod sessions;
pub mod settings;
mod shell;
pub mod shortcuts;
mod state;
pub mod status;
pub mod view_section;
pub mod window;

pub use command::{
    AnnotationTarget, Changes, Command, ConfigEdit, Decision, Guarded, NEEDS_PATH, Outcome,
    PanStep, Tick, ZoomAnchor, ZoomDirection, ZoomFactor,
};
pub use devices::SignalProfile;
pub use shell::{Shell, ShellConfig};
pub use state::{
    Activity, EdgeDirection, OperationKind, OperationView, SidePanel, State, Unsaved, Versions,
};

/// Version of the host API contract (`docs/host-api.md`). A change to commands, state fields,
/// events or their meaning changes this value together with the document.
pub const HOST_API_VERSION: &str = "1.2";
