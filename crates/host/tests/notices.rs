//! The grouped, bounded list of messages for the user (data-model "Message").
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::manual_range_contains
)]

use la_host::notices::{MAX_NOTICES, NoticeContent, NoticeList, Severity, notice_text};
use la_i18n::{Language, Localizer};

fn error(code: &'static str) -> NoticeContent {
    NoticeContent::error(code)
}

#[test]
fn identical_consecutive_notices_are_grouped() {
    let mut list = NoticeList::new();
    let first = list.add(Severity::Error, error("session.damaged"), 100);
    let second = list.add(Severity::Error, error("session.damaged"), 105);
    assert_eq!(first, second);
    assert_eq!(list.len(), 1);
    let notice = list.iter().next().unwrap();
    assert_eq!(notice.count, 2);
    assert_eq!(notice.first_at, 100);
    assert_eq!(notice.last_at, 105);
}

#[test]
fn different_notices_are_not_grouped() {
    let mut list = NoticeList::new();
    let a = list.add(Severity::Error, error("session.damaged"), 1);
    let b = list.add(Severity::Warning, error("session.damaged"), 2);
    let c = list.add(Severity::Warning, error("session.truncated"), 3);
    let d = list.add(Severity::Warning, error("session.damaged"), 4);
    assert_eq!(list.len(), 4);
    assert!(a != b && b != c && c != d);
    // Only the most recent notice groups: an older identical one does not.
    let e = list.add(Severity::Warning, error("session.damaged"), 5);
    assert_eq!(d, e);
    assert_eq!(list.len(), 4);
}

#[test]
fn info_notices_group_on_key_and_text() {
    let mut list = NoticeList::new();
    list.add(Severity::Info, NoticeContent::info("app-title", None), 1);
    list.add(Severity::Info, NoticeContent::info("app-title", None), 2);
    list.add(
        Severity::Info,
        NoticeContent::info("app-title", Some("x".into())),
        3,
    );
    assert_eq!(list.len(), 2);
}

#[test]
fn the_list_is_bounded_and_drops_the_oldest() {
    let mut list = NoticeList::new();
    let codes = ["session.damaged", "session.truncated"];
    let mut ids = Vec::new();
    for i in 0..(MAX_NOTICES + 10) {
        ids.push(list.add(
            Severity::Error,
            error(codes[i % 2]),
            i64::try_from(i).unwrap(),
        ));
    }
    assert_eq!(MAX_NOTICES, 50);
    assert_eq!(list.len(), MAX_NOTICES);
    let first_kept = list.iter().next().unwrap().id;
    assert_eq!(first_kept, ids[10]);
    // Ids keep increasing.
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn notices_can_be_dismissed_one_by_one_and_all_at_once() {
    let mut list = NoticeList::new();
    let a = list.add(Severity::Error, error("session.damaged"), 1);
    let b = list.add(Severity::Error, error("session.truncated"), 2);
    list.dismiss(a);
    assert_eq!(list.len(), 1);
    assert_eq!(list.iter().next().unwrap().id, b);
    list.dismiss(a);
    assert_eq!(list.len(), 1);
    list.dismiss_all();
    assert!(list.is_empty());
}

#[test]
fn the_text_follows_the_current_language() {
    let localizer = Localizer::new(Language::En);
    let mut list = NoticeList::new();
    list.add(Severity::Error, error("session.damaged"), 1);
    let notice = list.iter().next().unwrap().clone();
    let english = notice_text(&notice, &localizer);
    assert_eq!(english, localizer.error_message_for_code("session.damaged"));
    assert!(!english.is_empty());
    localizer.set_language(Language::Uk);
    let ukrainian = notice_text(&notice, &localizer);
    assert_ne!(ukrainian, english);
    assert_eq!(
        ukrainian,
        localizer.error_message_for_code("session.damaged")
    );
}

#[test]
fn an_info_notice_uses_its_key() {
    let localizer = Localizer::new(Language::De);
    let mut list = NoticeList::new();
    list.add(Severity::Info, NoticeContent::info("app-title", None), 1);
    let notice = list.iter().next().unwrap().clone();
    assert_eq!(notice_text(&notice, &localizer), "Logic Analyzer");
}
