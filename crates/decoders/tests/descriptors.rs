//! Descriptor validation, the registry and setup validation (spec FR-001 to FR-004, FR-018,
//! data-model.md "Decoder descriptor" and "Decoder setup").
//!
//! Expected outcomes are written by hand from the data model.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_decoders::descriptor::{
    Descriptor, Input, MAX_OPTIONS, MAX_SIGNALS, OptionKind, OptionSpec, Signal, is_identifier,
};
use la_decoders::error;
use la_decoders::registry::Registry;
use la_decoders::setup::{MAX_INSTANCES, Setup, Source, Value};

const fn signal(role: &'static str, required: bool, may_share: bool) -> Signal {
    Signal {
        role,
        label: "decoder-test-signal",
        required,
        may_share,
    }
}

static SIGNALS: [Signal; 3] = [
    signal("clock", true, false),
    signal("data", true, false),
    signal("enable", false, true),
];

static OPTIONS: [OptionSpec; 3] = [
    OptionSpec {
        id: "order",
        label: "decoder-test-option",
        kind: OptionKind::Choice {
            choices: &[
                ("lsb", "decoder-test-choice"),
                ("msb", "decoder-test-choice"),
            ],
            default: "lsb",
        },
    },
    OptionSpec {
        id: "bits",
        label: "decoder-test-option",
        kind: OptionKind::Integer {
            min: 1,
            max: 16,
            default: 8,
        },
    },
    OptionSpec {
        id: "invert",
        label: "decoder-test-option",
        kind: OptionKind::Boolean { default: false },
    },
];

/// Reads channels and produces `byte` items.
static LOW: Descriptor = Descriptor {
    id: "test.low",
    version: 1,
    name: "decoder-test-name",
    description: "decoder-test-description",
    signals: &SIGNALS,
    options: &OPTIONS,
    input: Input::Signals,
    outputs: &["byte"],
    generator: false,
};

/// Reads `byte` items and produces `word` items.
static MIDDLE: Descriptor = Descriptor {
    id: "test.middle",
    version: 1,
    name: "decoder-test-name",
    description: "decoder-test-description",
    signals: &[],
    options: &[],
    input: Input::Items("byte"),
    outputs: &["word"],
    generator: false,
};

/// Reads `word` items and produces nothing.
static TOP: Descriptor = Descriptor {
    id: "test.top",
    version: 1,
    name: "decoder-test-name",
    description: "decoder-test-description",
    signals: &[],
    options: &[],
    input: Input::Items("word"),
    outputs: &["byte", "word"],
    generator: false,
};

fn registry() -> Registry {
    let mut registry = Registry::builtin();
    registry.register(&LOW).unwrap();
    registry.register(&MIDDLE).unwrap();
    registry.register(&TOP).unwrap();
    registry
}

fn low(instance: u32) -> Setup {
    let mut setup = Setup::new(instance, "test.low");
    setup.channels.insert("clock".into(), 0);
    setup.channels.insert("data".into(), 1);
    setup
}

fn stacked(instance: u32, decoder: &str, below: u32) -> Setup {
    let mut setup = Setup::new(instance, decoder);
    setup.source = Source::Instance(below);
    setup
}

/// The code of the error and whether its detail names `setting`.
fn refusal(setup: &Setup, others: &[Setup]) -> (&'static str, String) {
    let e = setup.validate(&registry(), others, 8).unwrap_err();
    (e.code(), e.detail().to_owned())
}

#[test]
fn identifiers_have_the_documented_form() {
    for ok in ["a", "i2c.byte", "clocked_bits", "x9", &"a".repeat(48)] {
        assert!(is_identifier(ok), "{ok}");
    }
    for bad in ["", "A", "a b", "a-b", "é", "a/b", &"a".repeat(49)] {
        assert!(!is_identifier(bad), "{bad:?}");
    }
}

#[test]
fn a_valid_descriptor_is_accepted_and_found() {
    let registry = registry();
    assert_eq!(registry.list().len(), 3);
    assert_eq!(registry.get("test.low").unwrap().id, "test.low");
    assert!(registry.get("test.none").is_none());
    assert_eq!(registry.list()[0].id, "test.low", "registration order");
}

#[test]
fn an_invalid_descriptor_is_refused_and_leaves_the_registry_unchanged() {
    let mut registry = registry();
    let bad_id = Descriptor {
        id: "Bad Id",
        ..LOW
    };
    // Descriptors are static: leak the variants the test builds.
    let refuse = |registry: &mut Registry, d: Descriptor| {
        let d: &'static Descriptor = Box::leak(Box::new(d));
        let e = registry.register(d).unwrap_err();
        assert_eq!(e.code(), error::FAILED);
    };
    refuse(&mut registry, bad_id);
    refuse(
        &mut registry,
        Descriptor {
            id: "test.v0",
            version: 0,
            ..LOW
        },
    );
    // A duplicate id.
    refuse(&mut registry, LOW);
    // Duplicate roles.
    let dup_roles: &'static [Signal] = Box::leak(Box::new([
        signal("a", true, false),
        signal("a", false, false),
    ]));
    refuse(
        &mut registry,
        Descriptor {
            id: "test.dup1",
            signals: dup_roles,
            ..LOW
        },
    );
    // Duplicate options.
    let dup_opts: &'static [OptionSpec] = Box::leak(Box::new([OPTIONS[1], OPTIONS[1]]));
    refuse(
        &mut registry,
        Descriptor {
            id: "test.dup2",
            options: dup_opts,
            ..LOW
        },
    );
    // A default outside its range, a default that is not a choice, an empty choice list.
    for kind in [
        OptionKind::Integer {
            min: 1,
            max: 4,
            default: 5,
        },
        OptionKind::Integer {
            min: 4,
            max: 1,
            default: 2,
        },
        OptionKind::Choice {
            choices: &[("a", "k")],
            default: "b",
        },
        OptionKind::Choice {
            choices: &[],
            default: "a",
        },
        OptionKind::Choice {
            choices: &[("a", "k"), ("a", "k")],
            default: "a",
        },
    ] {
        let options: &'static [OptionSpec] = Box::leak(Box::new([OptionSpec {
            id: "o",
            label: "k",
            kind,
        }]));
        refuse(
            &mut registry,
            Descriptor {
                id: "test.range",
                options,
                ..LOW
            },
        );
    }
    // Too many signals or options.
    let many_signals: &'static [Signal] = Box::leak(
        (0..=MAX_SIGNALS)
            .map(|i| {
                let role: &'static str = Box::leak(format!("s{i}").into_boxed_str());
                signal(role, false, false)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    refuse(
        &mut registry,
        Descriptor {
            id: "test.many1",
            signals: many_signals,
            ..LOW
        },
    );
    let many_options: &'static [OptionSpec] = Box::leak(
        (0..=MAX_OPTIONS)
            .map(|i| {
                let id: &'static str = Box::leak(format!("o{i}").into_boxed_str());
                OptionSpec {
                    id,
                    label: "k",
                    kind: OptionKind::Boolean { default: false },
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    refuse(
        &mut registry,
        Descriptor {
            id: "test.many2",
            options: many_options,
            ..LOW
        },
    );
    // A decoder that reads items has no signals.
    refuse(
        &mut registry,
        Descriptor {
            id: "test.items",
            input: Input::Items("byte"),
            ..LOW
        },
    );
    assert_eq!(registry.list().len(), 3, "nothing was added");
}

#[test]
fn a_valid_setup_is_accepted_with_defaults_for_missing_options() {
    let registry = registry();
    let mut setup = low(1);
    assert_eq!(setup.validate(&registry, &[], 8).ok(), Some(()));
    assert_eq!(
        setup.option_value(&LOW, "bits"),
        Some(Value::Integer(8)),
        "a missing option takes its default"
    );
    assert_eq!(
        setup.option_value(&LOW, "order"),
        Some(Value::Choice("lsb".into()))
    );
    assert_eq!(
        setup.option_value(&LOW, "invert"),
        Some(Value::Boolean(false))
    );
    assert_eq!(setup.option_value(&LOW, "none"), None);
    setup.options.insert("bits".into(), Value::Integer(16));
    assert_eq!(setup.option_value(&LOW, "bits"), Some(Value::Integer(16)));
    assert_eq!(setup.validate(&registry, &[], 8).ok(), Some(()));
}

#[test]
fn an_unknown_decoder_is_refused_and_named() {
    let (code, detail) = refusal(&Setup::new(1, "test.nothing"), &[]);
    assert_eq!(code, error::INVALID_SETUP);
    assert!(detail.contains("decoder"), "{detail}");
}

#[test]
fn channels_must_exist_be_assigned_and_not_shared() {
    // A channel the capture does not have.
    let mut setup = low(1);
    setup.channels.insert("data".into(), 8);
    let (code, detail) = refusal(&setup, &[]);
    assert_eq!(
        (code, detail.contains("channels.data")),
        (error::INVALID_SETUP, true)
    );
    // A required role without a channel.
    let mut setup = low(1);
    setup.channels.remove("clock");
    let (code, detail) = refusal(&setup, &[]);
    assert_eq!(
        (code, detail.contains("channels.clock")),
        (error::INVALID_SETUP, true)
    );
    // A role the decoder does not have.
    let mut setup = low(1);
    setup.channels.insert("mystery".into(), 3);
    let (code, detail) = refusal(&setup, &[]);
    assert_eq!(
        (code, detail.contains("channels.mystery")),
        (error::INVALID_SETUP, true)
    );
    // Two roles on one channel where sharing is not allowed.
    let mut setup = low(1);
    setup.channels.insert("data".into(), 0);
    let (code, _) = refusal(&setup, &[]);
    assert_eq!(code, error::INVALID_SETUP);
    // An optional role may stay unassigned, and two roles that both allow sharing may share
    // (here only `enable` allows it, so sharing with `clock` is still refused).
    let mut setup = low(1);
    setup.channels.insert("enable".into(), 5);
    assert!(setup.validate(&registry(), &[], 8).is_ok());
    setup.channels.insert("enable".into(), 0);
    assert_eq!(refusal(&setup, &[]).0, error::INVALID_SETUP);
}

#[test]
fn options_must_be_of_their_type_and_inside_their_rules() {
    for (id, value) in [
        ("nothing", Value::Boolean(true)),
        ("bits", Value::Integer(0)),
        ("bits", Value::Integer(17)),
        ("bits", Value::Boolean(true)),
        ("order", Value::Choice("middle".into())),
        ("order", Value::Integer(1)),
        ("invert", Value::Integer(1)),
    ] {
        let mut setup = low(1);
        setup.options.insert(id.into(), value.clone());
        let (code, detail) = refusal(&setup, &[]);
        assert_eq!(code, error::INVALID_SETUP, "{id} {value:?}");
        assert!(detail.contains(&format!("options.{id}")), "{detail}");
    }
    // The ends of the range are inside it.
    for bits in [1, 16] {
        let mut setup = low(1);
        setup.options.insert("bits".into(), Value::Integer(bits));
        assert!(setup.validate(&registry(), &[], 8).is_ok(), "{bits}");
    }
}

#[test]
fn an_instance_name_is_limited_after_sanitizing() {
    let mut setup = low(1);
    setup.name = "n".repeat(64);
    assert!(setup.validate(&registry(), &[], 8).is_ok());
    setup.name = "n".repeat(65);
    let (code, detail) = refusal(&setup, &[]);
    assert_eq!(
        (code, detail.contains("name")),
        (error::INVALID_SETUP, true)
    );
    // Characters that sanitizing removes do not count.
    setup.name = format!("{}\u{200b}\u{202e}", "n".repeat(64));
    assert!(setup.validate(&registry(), &[], 8).is_ok());
}

#[test]
fn the_source_must_exist_and_match_the_input_kind() {
    let others = [low(1)];
    // A decoder that reads channels cannot read an instance.
    let mut setup = low(2);
    setup.source = Source::Instance(1);
    assert_eq!(refusal(&setup, &others).0, error::STACK_INCOMPATIBLE);
    // A decoder that reads items cannot read the capture.
    let setup = Setup::new(2, "test.middle");
    assert_eq!(refusal(&setup, &others).0, error::STACK_INCOMPATIBLE);
    // The source must exist.
    assert_eq!(
        refusal(&stacked(2, "test.middle", 9), &others).0,
        error::INVALID_SETUP
    );
    // The source must produce what the decoder reads: `low` produces bytes, `top` reads words.
    assert_eq!(
        refusal(&stacked(2, "test.top", 1), &others).0,
        error::STACK_INCOMPATIBLE
    );
    // A matching source is accepted.
    assert!(
        stacked(2, "test.middle", 1)
            .validate(&registry(), &others, 8)
            .is_ok()
    );
}

#[test]
fn a_stack_may_not_be_a_cycle() {
    let one = low(1);
    let two = stacked(2, "test.middle", 1);
    let three = stacked(3, "test.top", 2);
    assert!(
        three
            .validate(&registry(), &[one.clone(), two.clone()], 8)
            .is_ok()
    );
    // 2 reads 3 and 3 reads 2: a cycle (`top` produces bytes, which `middle` reads).
    let loop_back = stacked(2, "test.middle", 3);
    let (code, _) = refusal(&loop_back, &[one, two, three]);
    assert_eq!(code, error::STACK_CYCLE);
    // An instance cannot read itself.
    assert_eq!(
        refusal(&stacked(2, "test.middle", 2), &[]).0,
        error::STACK_CYCLE
    );
}

#[test]
fn a_chain_is_at_most_four_deep() {
    // 1 low, 2 middle, 3 top, 4 middle: four levels. `top` produces bytes as well as words.
    let mut chain = vec![
        low(1),
        stacked(2, "test.middle", 1),
        stacked(3, "test.top", 2),
    ];
    let four = stacked(4, "test.middle", 3);
    assert!(four.validate(&registry(), &chain, 8).is_ok(), "four levels");
    chain.push(four);
    let (code, _) = refusal(&stacked(5, "test.top", 4), &chain);
    assert_eq!(code, error::STACK_TOO_DEEP);
}

#[test]
fn moving_an_instance_above_a_long_chain_counts_the_instances_stacked_on_it() {
    // 1..4 as above, and a separate chain 5 low, 6 middle, 7 top (three levels).
    let others = vec![
        low(1),
        stacked(2, "test.middle", 1),
        stacked(3, "test.top", 2),
        stacked(4, "test.middle", 3),
        low(5),
        stacked(6, "test.middle", 5),
        stacked(7, "test.top", 6),
    ];
    // Moving 2 (with 3 and 4 stacked on it) onto 7 would make 5, 6, 7, 2, 3, 4: six levels.
    let moved = stacked(2, "test.middle", 7);
    assert_eq!(refusal(&moved, &others).0, error::STACK_TOO_DEEP);
    // Moving it onto 5 makes 5, 2, 3, 4: four levels, which is allowed.
    assert!(
        stacked(2, "test.middle", 5)
            .validate(&registry(), &others, 8)
            .is_ok()
    );
}

#[test]
fn the_instance_limit_is_enforced_and_replacing_an_instance_does_not_count_twice() {
    let others: Vec<Setup> = (1..=MAX_INSTANCES as u32).map(low).collect();
    assert_eq!(others.len(), 8);
    let (code, _) = refusal(&low(100), &others);
    assert_eq!(code, error::TOO_MANY_INSTANCES);
    // Changing the setup of an existing instance is not adding one.
    assert!(low(3).validate(&registry(), &others, 8).is_ok());
}
