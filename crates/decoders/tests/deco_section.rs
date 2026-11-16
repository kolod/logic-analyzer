//! The `DECO` section against hand-written bytes of `contracts/deco-section.md` (spec US5, SC-008).
//!
//! The vectors are written here byte by byte with a tiny encoder of their own, independent of the
//! writer under test.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_decoders::descriptor::{Descriptor, Input, OptionKind, OptionSpec, Signal};
use la_decoders::persist::Rejection;
use la_decoders::persist::deco::{Context, Parsed, Record, assemble, decode, encode_instance};
use la_decoders::registry::Registry;
use la_decoders::setup::{Setup, Source, Value};

// ---- an encoder of the contract, written for the tests ------------------------------------

fn varint(mut v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let low = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(low);
            return out;
        }
        out.push(low | 0x80);
    }
}

fn zigzag(v: i64) -> Vec<u8> {
    varint(((v << 1) ^ (v >> 63)) as u64)
}

fn string(s: &str) -> Vec<u8> {
    let mut out = varint(s.len() as u64);
    out.extend_from_slice(s.as_bytes());
    out
}

fn record(content: &[u8]) -> Vec<u8> {
    let mut out = varint(content.len() as u64);
    out.extend_from_slice(content);
    out
}

/// What an instance record holds.
struct Spec<'a> {
    id: u64,
    decoder: &'a str,
    version: u64,
    name: &'a str,
    source: u64,
    flags: u8,
    channels: Vec<(&'a str, u64)>,
    options: Vec<(&'a str, Vec<u8>)>,
    generator: Vec<(&'a str, i64)>,
}

impl<'a> Spec<'a> {
    fn sample(id: u64) -> Self {
        Spec {
            id,
            decoder: "sample.clocked_bits",
            version: 1,
            name: "",
            source: 0,
            flags: 1,
            channels: vec![("clock", 0), ("data", 1)],
            options: vec![("bits", [vec![0], zigzag(12)].concat())],
            generator: vec![],
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = varint(self.id);
        out.extend(string(self.decoder));
        out.extend(varint(self.version));
        out.extend(string(self.name));
        out.extend(varint(self.source));
        out.push(self.flags);
        out.extend(varint(self.channels.len() as u64));
        for (role, channel) in &self.channels {
            out.extend(string(role));
            out.extend(varint(*channel));
        }
        out.extend(varint(self.options.len() as u64));
        for (id, value) in &self.options {
            out.extend(string(id));
            out.extend(value);
        }
        let mut generator = varint(self.generator.len() as u64);
        for (id, value) in &self.generator {
            generator.extend(string(id));
            generator.push(0);
            generator.extend(zigzag(*value));
        }
        out.extend(record(&generator));
        out
    }
}

fn payload(version: u8, records: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![version];
    out.extend(varint(records.len() as u64));
    for r in records {
        out.extend(record(r));
    }
    out
}

// ---- a decoder with options of every kind --------------------------------------------------

static SIGNALS: [Signal; 2] = [
    Signal {
        role: "clock",
        label: "k",
        required: true,
        may_share: false,
    },
    Signal {
        role: "data",
        label: "k",
        required: true,
        may_share: false,
    },
];
static OPTIONS: [OptionSpec; 3] = [
    OptionSpec {
        id: "bits",
        label: "k",
        kind: OptionKind::Integer {
            min: 4,
            max: 16,
            default: 8,
        },
    },
    OptionSpec {
        id: "invert",
        label: "k",
        kind: OptionKind::Boolean { default: false },
    },
    OptionSpec {
        id: "order",
        label: "k",
        kind: OptionKind::Choice {
            choices: &[("lsb", "k"), ("msb", "k")],
            default: "msb",
        },
    },
];
static ALL_KINDS: Descriptor = Descriptor {
    id: "test.all_kinds",
    version: 2,
    name: "k",
    description: "k",
    signals: &SIGNALS,
    options: &OPTIONS,
    input: Input::Signals,
    outputs: &["byte"],
    generator: false,
};
static READER: Descriptor = Descriptor {
    id: "test.reader",
    version: 1,
    name: "k",
    description: "k",
    signals: &[],
    options: &[],
    input: Input::Items("byte"),
    outputs: &[],
    generator: false,
};

fn registry() -> Registry {
    let mut registry = Registry::builtin();
    registry.register(&ALL_KINDS).unwrap();
    registry.register(&READER).unwrap();
    registry
}

fn context(registry: &Registry) -> Context<'_> {
    Context {
        registry,
        channels: 8,
    }
}

fn all_kinds(id: u64, source: u64) -> Spec<'static> {
    Spec {
        id,
        decoder: "test.all_kinds",
        version: 2,
        name: "Bus A",
        source,
        flags: 0,
        channels: vec![("clock", 2), ("data", 3)],
        options: vec![
            ("bits", [vec![0], zigzag(16)].concat()),
            ("invert", vec![1, 1]),
            ("order", [vec![2], string("lsb")].concat()),
        ],
        generator: vec![("error_rate", 4), ("errors", 1)],
    }
}

fn rejected(bytes: &[u8]) -> String {
    let registry = registry();
    match decode(bytes, &context(&registry)) {
        Err(Rejection::Invalid(why)) => why.0.to_owned(),
        Err(Rejection::Newer) => "newer".to_owned(),
        Ok(_) => panic!("accepted"),
    }
}

// ---- tests ----------------------------------------------------------------------------------

#[test]
fn an_empty_section_is_accepted() {
    let registry = registry();
    assert!(decode(&[1, 0], &context(&registry)).unwrap().is_empty());
    assert_eq!(assemble(&[]), vec![1, 0]);
}

#[test]
fn a_hand_written_instance_is_read_as_the_contract_says() {
    let registry = registry();
    let spec = Spec::sample(7);
    let bytes = payload(1, &[spec.bytes()]);
    let records = decode(&bytes, &context(&registry)).unwrap();
    assert_eq!(records.len(), 1);
    let Record { id, raw, parsed } = &records[0];
    assert_eq!(*id, 7);
    assert_eq!(raw, &spec.bytes());
    let Parsed {
        setup,
        decoder_version,
        skipped_options,
    } = parsed.clone().unwrap();
    assert_eq!((decoder_version, skipped_options), (1, 0));
    assert_eq!(setup.instance, 7);
    assert_eq!(setup.decoder, "sample.clocked_bits");
    assert_eq!(setup.source, Source::Capture);
    assert!(setup.lane_visible);
    assert_eq!(setup.channels.get("clock"), Some(&0));
    assert_eq!(setup.channels.get("data"), Some(&1));
    assert_eq!(setup.options.get("bits"), Some(&Value::Integer(12)));
    // The writer produces exactly these bytes.
    assert_eq!(encode_instance(&setup, 1), spec.bytes());
    assert_eq!(assemble(&[encode_instance(&setup, 1)]), bytes);
}

#[test]
fn a_stack_with_options_of_all_three_kinds_round_trips() {
    let registry = registry();
    let lower = all_kinds(1, 0);
    let mut upper = Spec {
        decoder: "test.reader",
        version: 1,
        name: "On top",
        ..Spec::sample(2)
    };
    upper.source = 2; // instance 1
    upper.channels = vec![];
    upper.options = vec![];
    let bytes = payload(1, &[lower.bytes(), upper.bytes()]);
    let records = decode(&bytes, &context(&registry)).unwrap();
    let low = records[0].parsed.clone().unwrap().setup;
    assert_eq!(low.options.get("invert"), Some(&Value::Boolean(true)));
    assert_eq!(low.options.get("order"), Some(&Value::Choice("lsb".into())));
    assert_eq!(low.generator.get("errors"), Some(&1));
    assert_eq!(low.name, "Bus A");
    assert!(!low.lane_visible);
    let up = records[1].parsed.clone().unwrap().setup;
    assert_eq!(up.source, Source::Instance(1));
    // Writing the parsed setups gives the bytes that were read.
    let rewritten = assemble(&[encode_instance(&low, 2), encode_instance(&up, 1)]);
    assert_eq!(rewritten, bytes);
}

#[test]
fn a_maximal_section_of_eight_instances_is_accepted() {
    let registry = registry();
    let records: Vec<Vec<u8>> = (1..=8)
        .map(|id| {
            let mut spec = all_kinds(id, 0);
            spec.name = "n".repeat(64).leak();
            spec.bytes()
        })
        .collect();
    let bytes = payload(1, &records);
    assert_eq!(decode(&bytes, &context(&registry)).unwrap().len(), 8);
    let nine: Vec<Vec<u8>> = (1..=9).map(|id| all_kinds(id, 0).bytes()).collect();
    assert_eq!(
        rejected(&payload(1, &nine)),
        "a count or number is beyond its limit"
    );
}

#[test]
fn every_rule_of_the_contract_has_a_failing_fixture() {
    let ok = |spec: &Spec<'_>| payload(1, &[spec.bytes()]);
    // The version and the framing.
    assert_eq!(rejected(&[0, 0]), "the version is not 1");
    assert_eq!(rejected(&[2, 0]), "newer");
    assert_eq!(rejected(&[]), "payload ends");
    assert_eq!(rejected(&[1, 0x80, 0x00]), "varint is not canonical");
    let mut trailing = ok(&Spec::sample(1));
    trailing.push(0);
    assert_eq!(rejected(&trailing), "bytes remain after the last record");
    // An id twice.
    assert_eq!(
        rejected(&payload(
            1,
            &[Spec::sample(1).bytes(), Spec::sample(1).bytes()]
        )),
        "an id repeats"
    );
    // Names: not sanitized, too long.
    let mut spec = Spec::sample(1);
    spec.name = "a\u{202e}b";
    assert_eq!(rejected(&ok(&spec)), "a name is not in its sanitized form");
    spec.name = "n".repeat(65).leak();
    assert_eq!(rejected(&ok(&spec)), "a name is too long");
    // Sources: not an earlier record, a chain that is too deep.
    let mut spec = Spec::sample(1);
    spec.source = 6;
    assert_eq!(rejected(&ok(&spec)), "a source is not an earlier record");
    let chain: Vec<Vec<u8>> = (1..=5)
        .map(|id| {
            let mut s = Spec {
                decoder: "test.reader",
                version: 1,
                ..Spec::sample(id)
            };
            s.channels = vec![];
            s.options = vec![];
            s.source = if id == 1 { 0 } else { id - 1 + 1 };
            s.bytes()
        })
        .collect();
    assert_eq!(
        rejected(&payload(1, &chain)),
        "a chain is deeper than allowed"
    );
    // Reserved bits.
    let mut spec = Spec::sample(1);
    spec.flags = 2;
    assert_eq!(rejected(&ok(&spec)), "a reserved bit is set");
    // Channels: order, range, beyond the capture, a role the decoder does not have.
    let mut spec = Spec::sample(1);
    spec.channels = vec![("data", 1), ("clock", 0)];
    assert_eq!(rejected(&ok(&spec)), "roles are not in ascending order");
    spec.channels = vec![("clock", 32)];
    assert_eq!(
        rejected(&ok(&spec)),
        "a count or number is beyond its limit"
    );
    spec.channels = vec![("clock", 9)];
    assert_eq!(rejected(&ok(&spec)), "a channel is beyond the capture");
    spec.channels = vec![("mystery", 1)];
    assert_eq!(
        rejected(&ok(&spec)),
        "a signal role is unknown to its decoder"
    );
    // Options: order, a value outside its range, of the wrong type.
    let mut spec = all_kinds(1, 0);
    spec.options.swap(0, 1);
    assert_eq!(rejected(&ok(&spec)), "options are not in ascending order");
    let mut spec = Spec::sample(1);
    spec.options = vec![("bits", [vec![0], zigzag(99)].concat())];
    assert_eq!(
        rejected(&ok(&spec)),
        "an option value is outside its range or type"
    );
    spec.options = vec![("bits", vec![1, 1])];
    assert_eq!(
        rejected(&ok(&spec)),
        "an option value is outside its range or type"
    );
    spec.options = vec![("bits", vec![9, 0])];
    assert_eq!(rejected(&ok(&spec)), "an option has an unknown kind");
    spec.options = vec![("bits", vec![1, 2])];
    assert_eq!(rejected(&ok(&spec)), "a boolean is neither 0 nor 1");
    // Identifiers: capital letters, the wrong length.
    let mut spec = Spec::sample(1);
    spec.decoder = "Sample";
    assert_eq!(rejected(&ok(&spec)), "an identifier is malformed");
    // Not UTF-8 in a name.
    let mut bytes = ok(&Spec {
        name: "ab",
        ..Spec::sample(1)
    });
    let at = bytes.iter().position(|b| *b == b'a').unwrap();
    bytes[at] = 0xff;
    assert_eq!(rejected(&bytes), "a string is not UTF-8");
}

#[test]
fn unknown_decoders_and_options_are_kept_byte_for_byte() {
    let registry = registry();
    // A decoder this version does not have, a newer version of one it has, an unknown option.
    let unknown = Spec {
        decoder: "vendor.future",
        version: 3,
        ..Spec::sample(1)
    };
    let newer = Spec {
        version: 9,
        ..Spec::sample(2)
    };
    let mut extra = Spec::sample(3);
    extra.options = vec![
        ("bits", [vec![0], zigzag(10)].concat()),
        ("future_option", vec![1, 1]),
    ];
    let bytes = payload(1, &[unknown.bytes(), newer.bytes(), extra.bytes()]);
    let records = decode(&bytes, &context(&registry)).unwrap();
    assert!(records[0].parsed.is_none() && records[1].parsed.is_none());
    assert_eq!(records[0].raw, unknown.bytes());
    assert_eq!(records[1].raw, newer.bytes());
    let parsed = records[2].parsed.clone().unwrap();
    assert_eq!(parsed.skipped_options, 1);
    assert_eq!(parsed.setup.options.get("bits"), Some(&Value::Integer(10)));
    assert_eq!(records[2].raw, extra.bytes());
    // Written back from the raw bytes the section is the same.
    let again = assemble(&records.iter().map(|r| r.raw.clone()).collect::<Vec<_>>());
    assert_eq!(again, bytes);
}

#[test]
fn the_writer_is_deterministic() {
    let mut setup = Setup::new(1, "test.all_kinds");
    setup.channels.insert("data".into(), 3);
    setup.channels.insert("clock".into(), 2);
    setup
        .options
        .insert("order".into(), Value::Choice("msb".into()));
    setup.options.insert("bits".into(), Value::Integer(5));
    setup.generator.insert("errors".into(), 0);
    let a = encode_instance(&setup, 2);
    let b = encode_instance(&setup.clone(), 2);
    assert_eq!(a, b);
}

/// Every single-byte change, every truncation and some extensions of a payload: nothing panics,
/// and what is accepted writes back unchanged.
fn fuzz(bytes: &[u8]) {
    let registry = registry();
    let check = |mutated: &[u8]| {
        if let Ok(records) = decode(mutated, &context(&registry)) {
            let mut rebuilt = Vec::new();
            for r in &records {
                let content = match &r.parsed {
                    Some(p) if p.skipped_options == 0 => {
                        encode_instance(&p.setup, p.decoder_version)
                    }
                    _ => r.raw.clone(),
                };
                assert_eq!(content, r.raw, "a record was accepted but is not canonical");
                rebuilt.push(content);
            }
            assert_eq!(
                assemble(&rebuilt),
                mutated,
                "an accepted payload does not write back"
            );
        }
    };
    for at in 0..bytes.len() {
        for value in [
            0_u8,
            1,
            2,
            7,
            0x7f,
            0x80,
            0xff,
            bytes[at].wrapping_add(1),
            bytes[at] ^ 0x20,
        ] {
            let mut mutated = bytes.to_vec();
            mutated[at] = value;
            check(&mutated);
        }
    }
    for cut in 0..bytes.len() {
        check(&bytes[..cut]);
    }
    for extra in [vec![0], vec![0xff; 5], vec![1, 2, 3]] {
        let mut longer = bytes.to_vec();
        longer.extend(extra);
        check(&longer);
    }
}

#[test]
fn a_mutation_fuzzer_finds_nothing_in_the_small_payloads() {
    fuzz(&[1, 0]);
    fuzz(&payload(1, &[Spec::sample(1).bytes()]));
    let lower = all_kinds(1, 0);
    let mut upper = Spec {
        decoder: "test.reader",
        version: 1,
        ..Spec::sample(2)
    };
    upper.source = 2;
    upper.channels = vec![];
    upper.options = vec![];
    fuzz(&payload(1, &[lower.bytes(), upper.bytes()]));
}

#[test]
#[ignore = "randomized: set LA_DECO_FUZZ_ITERATIONS (a million in the nightly run)"]
fn a_randomized_fuzzer_finds_nothing() {
    let iterations: u64 = std::env::var("LA_DECO_FUZZ_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100_000);
    let registry = registry();
    let base = payload(1, &[all_kinds(1, 0).bytes(), Spec::sample(2).bytes()]);
    let mut state = 99_u64;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 16
    };
    for _ in 0..iterations {
        let mut bytes = base.clone();
        for _ in 0..=(next() % 4) {
            let at = (next() % bytes.len() as u64) as usize;
            bytes[at] = next() as u8;
        }
        if next() % 5 == 0 {
            bytes.truncate((next() % bytes.len() as u64) as usize);
        }
        if let Ok(records) = decode(&bytes, &context(&registry)) {
            let rebuilt: Vec<Vec<u8>> = records.iter().map(|r| r.raw.clone()).collect();
            assert_eq!(assemble(&rebuilt), bytes);
        }
    }
}
