//! Messages for the user: a bounded list in which identical neighbours are grouped.
//!
//! A notice stores *what* happened (a stable error code, or an information key), not the text;
//! the text is produced when it is displayed, in the current language (see [`notice_text`]).

use std::collections::VecDeque;

use la_i18n::{Args, Key, Localizer};

/// The most notices kept; the oldest is dropped when a new one would exceed it.
pub const MAX_NOTICES: usize = 50;

/// Identifies a notice; increases with every new notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoticeId(pub u64);

/// How important a notice is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Something the user may want to know.
    Info,
    /// Something went wrong but nothing was lost.
    Warning,
    /// An operation failed.
    Error,
}

/// What a notice says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeContent {
    /// The failure with this stable error code (the message comes from `errors.ftl`).
    Error {
        /// The stable code, such as `session.damaged`.
        code: String,
    },
    /// An information message from `ui.ftl` with an optional text argument named `text`.
    Info {
        /// The message key.
        key: &'static str,
        /// The text argument, if the message has one.
        text: Option<String>,
    },
}

impl NoticeContent {
    /// The content for the error with this code.
    #[must_use]
    pub fn error(code: &str) -> Self {
        NoticeContent::Error {
            code: code.to_owned(),
        }
    }

    /// The content for an information message.
    #[must_use]
    pub fn info(key: &'static str, text: Option<String>) -> Self {
        NoticeContent::Info { key, text }
    }
}

/// One message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// Its identity.
    pub id: NoticeId,
    /// How important it is.
    pub severity: Severity,
    /// What it says.
    pub content: NoticeContent,
    /// How many identical notices were grouped into this one (at least 1).
    pub count: u32,
    /// Time of the first occurrence, seconds since the Unix epoch (display only).
    pub first_at: i64,
    /// Time of the latest occurrence.
    pub last_at: i64,
}

/// The recent notices, oldest first.
#[derive(Debug, Clone, Default)]
pub struct NoticeList {
    items: VecDeque<Notice>,
    next_id: u64,
}

impl NoticeList {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a notice, or counts it into the most recent one when that is identical. Returns the
    /// id of the notice that now represents it.
    pub fn add(&mut self, severity: Severity, content: NoticeContent, now: i64) -> NoticeId {
        if let Some(last) = self.items.back_mut() {
            if last.severity == severity && last.content == content {
                last.count = last.count.saturating_add(1);
                last.last_at = now;
                return last.id;
            }
        }
        let id = NoticeId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.items.push_back(Notice {
            id,
            severity,
            content,
            count: 1,
            first_at: now,
            last_at: now,
        });
        while self.items.len() > MAX_NOTICES {
            self.items.pop_front();
        }
        id
    }

    /// Removes one notice (nothing happens for an unknown id).
    pub fn dismiss(&mut self, id: NoticeId) {
        self.items.retain(|n| n.id != id);
    }

    /// Removes every notice.
    pub fn dismiss_all(&mut self) {
        self.items.clear();
    }

    /// The notices, oldest first.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Notice> {
        self.items.iter()
    }

    /// How many notices there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// The text of `notice` in the current language of `localizer`.
#[must_use]
pub fn notice_text(notice: &Notice, localizer: &Localizer) -> String {
    match &notice.content {
        NoticeContent::Error { code } => localizer.error_message_for_code(code),
        NoticeContent::Info { key, text } => {
            let mut args = Args::new();
            if let Some(text) = text {
                args = args.text("text", text);
            }
            localizer.message(&Key::from_static(key), &args)
        }
    }
}
