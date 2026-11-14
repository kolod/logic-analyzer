//! Hostile and inconsistent files are rejected safely (spec US3, FR-013, FR-015..019, SC-006).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::Path;

use common::{
    CaptPayload, Counting, MemFs, RawSection, TempDir, build_file, file_with_capture, load_bytes,
    load_error_code, peak_allocation, real_file, small_capt, split_sections, varint,
};
use la_session::error::{
    DAMAGED, INCONSISTENT, IO_READ, MEMORY_BUDGET, NOT_A_SESSION, TOO_LARGE, TRUNCATED,
};
use la_session::harness::{FaultFs, FaultPlan, Op, load_with};
use la_session::{Control, LoadLimits, load_blocking};
use la_storage::Limits;

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn capt_code(mutate: impl FnOnce(&mut CaptPayload)) -> &'static str {
    let mut capt = small_capt();
    mutate(&mut capt);
    load_error_code(&file_with_capture(&capt.bytes()))
}

#[test]
fn the_unmodified_crafted_file_loads() {
    let bytes = file_with_capture(&small_capt().bytes());
    let session = load_bytes(&bytes, &LoadLimits::default()).unwrap();
    assert_eq!(session.snapshot().length(), 50);
    assert_eq!(session.snapshot().gaps().len(), 1);
}

#[test]
fn files_that_are_not_session_files_are_named_as_such() {
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend(std::iter::repeat_n(7, 500));
    let mut zip = b"PK\x03\x04".to_vec();
    zip.extend(std::iter::repeat_n(1, 500));
    let random: Vec<u8> = (0..1000_u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8 | 1)
        .collect();
    for (what, bytes) in [
        ("empty", vec![]),
        ("one byte", vec![0x00]),
        ("png", png),
        ("zip", zip),
        (
            "text",
            b"this is not a logic analyzer session at all".to_vec(),
        ),
        ("random", random),
    ] {
        assert_eq!(load_error_code(&bytes), NOT_A_SESSION, "{what}");
    }
}

#[test]
fn well_formed_files_with_inconsistent_content_are_rejected() {
    // Transitions out of order, equal, beyond the length, inside the gap, at position 0.
    assert_eq!(
        capt_code(|c| c.streams[0] = vec![30, 0]),
        INCONSISTENT,
        "equal positions"
    );
    assert_eq!(
        capt_code(|c| c.streams[0] = vec![10, 100]),
        INCONSISTENT,
        "beyond the length"
    );
    assert_eq!(
        capt_code(|c| c.streams[1] = vec![21, 1]),
        INCONSISTENT,
        "inside the gap"
    );
    assert_eq!(
        capt_code(|c| c.streams[0] = vec![0, 5]),
        INCONSISTENT,
        "position 0"
    );
    assert_eq!(
        capt_code(|c| c.streams[0] = vec![50, 1]),
        INCONSISTENT,
        "at the length"
    );
    // Gaps: past the end, empty, adjacent, in the wrong order.
    assert_eq!(
        capt_code(|c| c.gaps = vec![(48, 5)]),
        INCONSISTENT,
        "gap past the end"
    );
    assert_eq!(
        capt_code(|c| c.gaps = vec![(20, 0)]),
        INCONSISTENT,
        "empty gap"
    );
    assert_eq!(
        capt_code(|c| c.gaps = vec![(20, 5), (0, 5)]),
        INCONSISTENT,
        "adjacent gaps"
    );
    // The first stored sample has nothing to differ from.
    assert_eq!(
        capt_code(|c| {
            c.gaps = vec![(0, 5)];
            c.streams[0] = vec![5, 5];
        }),
        INCONSISTENT,
        "transition at the first stored position"
    );
    // Channel count and rate disagree with the description (2 channels, 12.5 MHz).
    assert_eq!(
        capt_code(|c| {
            c.channels = 1;
            c.table.truncate(1);
            c.streams.truncate(1);
        }),
        INCONSISTENT,
        "channel count"
    );
    assert_eq!(capt_code(|c| c.rate = 1_000), INCONSISTENT, "sample rate");
    assert_eq!(capt_code(|c| c.rate = 0), INCONSISTENT, "zero sample rate");
    // Structural damage with valid checksums is `damaged`.
    assert_eq!(capt_code(|c| c.flags = 8), DAMAGED, "reserved flag");
    assert_eq!(capt_code(|c| c.table[0].0 = 3), DAMAGED, "initial level");
    assert_eq!(capt_code(|c| c.table[0].2 = 9), DAMAGED, "stream length");
}

#[test]
fn a_non_canonical_varint_is_damage_even_when_the_checksums_are_right() {
    // The gap length 5 written as 0x85 0x00 instead of 0x05: the same value, a second encoding.
    let mut bytes = small_capt().bytes();
    assert_eq!(bytes[20], 5);
    bytes.splice(20..21, [0x85, 0x00]);
    assert_eq!(load_error_code(&file_with_capture(&bytes)), DAMAGED);
    // The canonical form loads.
    assert!(
        load_bytes(
            &file_with_capture(&small_capt().bytes()),
            &LoadLimits::default()
        )
        .is_ok()
    );
}

#[test]
fn a_description_that_contradicts_itself_is_inconsistent() {
    let mut sections = split_sections(&real_file());
    // channels = 0 in META: after the two texts, the byte is at a fixed offset from the front.
    // flags(1) + id(1 + 9) + name(1 + 14 bytes of "Logic16 Клон") = 26 → channels at 26.
    let meta = &mut sections[0].payload;
    let channels_at = 1 + (1 + "usb:logic16-7".len()) + (1 + "Logic16 Клон".len());
    assert_eq!(meta[channels_at], 2);
    meta[channels_at] = 0;
    assert_eq!(load_error_code(&build_file(&sections)), INCONSISTENT);
}

#[test]
fn an_unknown_critical_section_and_extra_sections_are_handled() {
    let mut sections = split_sections(&real_file());
    sections.push(RawSection {
        tag: *b"NEWX",
        critical: true,
        payload: vec![1, 2, 3],
    });
    let error = load_bytes(&build_file(&sections), &LoadLimits::default()).unwrap_err();
    assert_eq!(error.code(), la_session::error::UNKNOWN_CRITICAL_SECTION);
    assert!(error.detail().contains("NEWX") && error.detail().contains("newer"));
    // 62 optional sections plus META and CAPT is the maximum.
    let mut sections = split_sections(&real_file());
    for i in 0..62_u8 {
        sections.push(RawSection {
            tag: [b'X', b'A' + i / 26, b'a' + i % 26, b'0'],
            critical: false,
            payload: vec![i; usize::from(i)],
        });
    }
    let session = load_bytes(&build_file(&sections), &LoadLimits::default()).unwrap();
    assert_eq!(session.extras().len(), 62);
    // One more is too many (the header says so before anything else is read).
    sections.push(RawSection {
        tag: *b"YYYY",
        critical: false,
        payload: vec![],
    });
    assert_eq!(load_error_code(&build_file(&sections)), TOO_LARGE);
}

#[test]
fn files_that_claim_absurd_sizes_are_rejected_without_allocating_from_the_claim() {
    let file_len_cases: Vec<(&str, Vec<u8>)> = {
        let mut cases = Vec::new();
        // Channel streams of u64::MAX bytes and transitions.
        let mut capt = small_capt();
        capt.table[0] = (0, u64::MAX, u64::MAX);
        cases.push(("huge channel stream", file_with_capture(&capt.bytes())));
        let mut capt = small_capt();
        capt.table[0] = (0, 1, u64::MAX);
        cases.push(("huge stream length", file_with_capture(&capt.bytes())));
        // A gap count of u64::MAX.
        let mut bytes = vec![2, 0];
        bytes.extend_from_slice(&12_500_000_u64.to_le_bytes());
        bytes.extend_from_slice(&50_u64.to_le_bytes());
        varint(u64::MAX, &mut bytes);
        cases.push(("huge gap count", file_with_capture(&bytes)));
        // A text length of u64::MAX in META.
        let mut sections = split_sections(&real_file());
        let mut meta = vec![0_u8];
        varint(u64::MAX, &mut meta);
        sections[0].payload = meta;
        cases.push(("huge text length", build_file(&sections)));
        // A section whose payload length is u64::MAX.
        let mut bytes = real_file();
        bytes[32 + 8..32 + 16].copy_from_slice(&u64::MAX.to_le_bytes());
        common::repair_checksums(&mut bytes);
        cases.push(("huge section length", bytes));
        // A header that claims a file of u64::MAX bytes.
        let mut bytes = real_file();
        bytes[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
        common::repair_checksums(&mut bytes);
        cases.push(("huge file length", bytes));
        cases
    };
    for (what, bytes) in file_len_cases {
        let (code, peak) = peak_allocation(|| {
            load_bytes(&bytes, &LoadLimits::default())
                .map(|_| "loaded")
                .unwrap_or_else(|e| e.code())
        });
        assert!(
            [DAMAGED, TRUNCATED, TOO_LARGE, INCONSISTENT].contains(&code),
            "{what}: {code}"
        );
        // The bytes themselves are copied once by the in-memory file system (the test's own
        // allocation); nothing proportional to the claim may come on top.
        assert!(
            peak < i64::try_from(bytes.len()).unwrap() * 3 + 1_048_576,
            "{what}: allocated {peak} bytes for a {} byte file",
            bytes.len()
        );
    }
}

#[test]
fn a_capture_that_does_not_fit_the_budget_is_rejected_before_its_data_is_read() {
    // 300,000 one-byte transitions: about 330 KB of file, more than 1 MiB once loaded.
    let count = 300_000_u64;
    let capt = CaptPayload {
        channels: 2,
        flags: 0,
        rate: 12_500_000,
        length: count * 2 + 10,
        gaps: vec![],
        table: vec![(0, count, count), (0, 0, 0)],
        streams: vec![vec![1; count as usize], vec![]],
    };
    let bytes = file_with_capture(&capt.bytes());
    assert!(bytes.len() > 300_000);
    let tiny = LoadLimits {
        storage: Limits {
            memory_budget_bytes: 1024 * 1024,
            ..Limits::default()
        },
        ..LoadLimits::default()
    };
    let fs = MemFs::new(bytes.clone());
    let error = load_with(&fs, Path::new("m"), &tiny, &Control::new()).unwrap_err();
    assert_eq!(error.code(), MEMORY_BUDGET);
    assert!(error.detail().contains("budget"));
    let read = fs.bytes_read.load(std::sync::atomic::Ordering::Relaxed);
    assert!(
        read <= 65_536,
        "{read} bytes were read before the budget check; the data must not be read"
    );
    // The same file loads with the default budget.
    assert!(load_bytes(&bytes, &LoadLimits::default()).is_ok());
}

#[test]
fn a_file_larger_than_the_file_limit_is_rejected_at_once() {
    let bytes = real_file();
    let limits = LoadLimits {
        max_file_bytes: 100,
        ..LoadLimits::default()
    };
    let fs = MemFs::new(bytes);
    let error = load_with(&fs, Path::new("m"), &limits, &Control::new()).unwrap_err();
    assert_eq!(error.code(), TOO_LARGE);
    assert_eq!(fs.bytes_read.load(std::sync::atomic::Ordering::Relaxed), 0);
}

#[test]
fn a_file_that_changes_while_it_is_read_is_an_error_or_a_correct_session() {
    let bytes = real_file();
    // The file shrinks after it was opened.
    let mut shrunk = MemFs::new(bytes[..bytes.len() - 40].to_vec());
    shrunk.reported_length = bytes.len() as u64;
    let error = load_with(
        &shrunk,
        Path::new("m"),
        &LoadLimits::default(),
        &Control::new(),
    )
    .unwrap_err();
    assert_eq!(error.code(), TRUNCATED);
    // The file grows after it was opened.
    let mut grown_bytes = bytes.clone();
    grown_bytes.extend_from_slice(&[9; 30]);
    let mut grown = MemFs::new(grown_bytes);
    grown.reported_length = bytes.len() as u64;
    let error = load_with(
        &grown,
        Path::new("m"),
        &LoadLimits::default(),
        &Control::new(),
    )
    .unwrap_err();
    assert_eq!(error.code(), DAMAGED);
}

#[test]
fn unreadable_paths_are_actionable_io_errors() {
    let dir = TempDir::new("unreadable");
    let limits = LoadLimits::default();
    let missing = load_blocking(&dir.join("missing"), &limits, &Control::new()).unwrap_err();
    assert_eq!(missing.code(), IO_READ);
    assert!(
        missing.detail().contains("does not exist"),
        "{}",
        missing.detail()
    );
    // A folder is not a file (the error differs by platform but is always a typed read error).
    let folder = load_blocking(dir.path(), &limits, &Control::new()).unwrap_err();
    assert_eq!(folder.code(), IO_READ);
    // Denied access and a read that fails half way, through the file-system seam.
    let path = dir.join("good");
    std::fs::write(&path, real_file()).unwrap();
    for (op, nth, kind) in [
        (Op::OpenRead, 0, std::io::ErrorKind::PermissionDenied),
        (Op::Read, 0, std::io::ErrorKind::Other),
    ] {
        let fs = FaultFs::failing(FaultPlan {
            op,
            nth,
            kind,
            partial_write: false,
        });
        let error = load_with(&fs, &path, &limits, &Control::new()).unwrap_err();
        assert_eq!(error.code(), IO_READ, "{op:?}");
    }
    let fs = FaultFs::failing(FaultPlan {
        op: Op::OpenRead,
        nth: 0,
        kind: std::io::ErrorKind::PermissionDenied,
        partial_write: false,
    });
    let denied = load_with(&fs, &path, &limits, &Control::new()).unwrap_err();
    assert!(
        denied.detail().contains("permission"),
        "{}",
        denied.detail()
    );
}

#[test]
fn loading_the_same_file_twice_at_once_works() {
    let dir = TempDir::new("twice");
    let path = dir.join("good");
    std::fs::write(&path, real_file()).unwrap();
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || {
                load_blocking(&path, &LoadLimits::default(), &Control::new())
                    .map(|s| s.snapshot().length())
            })
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap().unwrap(), 100);
    }
}
