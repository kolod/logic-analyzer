//! Localization: messages in six languages, plurals, locale formatting and the parity check.
//!
//! Messages are written in Project Fluent files under `resources/i18n/` and compiled into the
//! program (see `specs/005-i18n-foundation`). English is the source of truth for keys and the
//! fallback for everything missing.
//!
//! **Never log placeholder values.** Log only language, key and file names.

#![deny(missing_docs)]
#![allow(
    dead_code,
    reason = "used by the localizer and the languages added in later tasks"
)]

mod resources;
