//! The composite source: generators on their channels, the random signals of the device on all
//! the others (spec US6, research R14).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_decoders::decoders::clocked_bits::ID;
use la_decoders::generators::clocked_bits::{claim, source};
use la_decoders::generators::composite::CompositeFactory;
use la_decoders::setup::Setup;
use la_devices::{RandomSource, SignalSource, SignalSourceFactory};

fn setup() -> Setup {
    let mut setup = Setup::new(1, ID);
    setup.channels.insert("clock".into(), 1);
    setup.channels.insert("data".into(), 2);
    setup
}

fn fill(source: &mut dyn SignalSource, length: usize, chunk: usize) -> Vec<u32> {
    let mut words = vec![0_u32; length];
    let mut start = 0;
    for piece in words.chunks_mut(chunk) {
        source.fill(start as u64, piece);
        start += piece.len();
    }
    words
}

fn factory() -> CompositeFactory {
    CompositeFactory::new(vec![claim(&setup()).unwrap()], 10)
}

#[test]
fn claimed_channels_come_from_the_generator_and_the_rest_from_the_random_source() {
    let mask = 0b1111_1111;
    let composite = fill(
        factory()
            .create(la_devices::ChannelSet::from_u32(mask), 1_000_000, 42)
            .as_mut(),
        5_000,
        5_000,
    );
    let generated = fill(&mut source(&setup(), 42).unwrap(), 5_000, 5_000);
    let random = fill(
        &mut RandomSource::new(la_devices::ChannelSet::from_u32(mask & !0b110), 42, 10),
        5_000,
        5_000,
    );
    for (i, word) in composite.iter().enumerate() {
        assert_eq!(
            word & 0b110,
            generated[i] & 0b110,
            "claimed channels at {i}"
        );
        assert_eq!(word & !0b110, random[i] & !0b110, "other channels at {i}");
    }
    // Both kinds of signal are really there.
    assert!(composite.iter().any(|w| w & 0b110 != 0));
    assert!(composite.iter().any(|w| w & !0b110 != 0));
}

#[test]
fn disabled_channels_stay_at_zero() {
    // Only channels 0 and 1 are enabled: the data channel (2) is claimed but disabled.
    let words = fill(
        factory()
            .create(la_devices::ChannelSet::from_u32(0b11), 1_000_000, 7)
            .as_mut(),
        4_000,
        4_000,
    );
    assert!(words.iter().all(|w| w & !0b11 == 0));
}

#[test]
fn the_same_seed_gives_the_same_signal_for_any_chunking() {
    let reference = fill(
        factory()
            .create(la_devices::ChannelSet::from_u32(u32::MAX), 1_000_000, 5)
            .as_mut(),
        9_000,
        9_000,
    );
    for chunk in [1, 7, 4_096] {
        let words = fill(
            factory()
                .create(la_devices::ChannelSet::from_u32(u32::MAX), 1_000_000, 5)
                .as_mut(),
            9_000,
            chunk,
        );
        assert_eq!(words, reference, "chunk {chunk}");
    }
    let other = fill(
        factory()
            .create(la_devices::ChannelSet::from_u32(u32::MAX), 1_000_000, 6)
            .as_mut(),
        9_000,
        9_000,
    );
    assert_ne!(other, reference);
}

#[test]
fn without_claims_it_is_the_random_source() {
    let none = CompositeFactory::new(Vec::new(), 10);
    let a = fill(
        none.create(la_devices::ChannelSet::from_u32(0b1111), 1_000_000, 3)
            .as_mut(),
        3_000,
        3_000,
    );
    let b = fill(
        &mut RandomSource::new(la_devices::ChannelSet::from_u32(0b1111), 3, 10),
        3_000,
        3_000,
    );
    assert_eq!(a, b);
}

#[test]
fn a_setup_without_channels_makes_no_claim() {
    assert!(claim(&Setup::new(1, ID)).is_none());
}

#[test]
fn the_i2c_claim_fills_only_its_two_channels_and_an_incomplete_setup_gives_none() {
    use la_decoders::generators::i2c;
    let mut setup = Setup::new(1, la_decoders::decoders::i2c::ID);
    setup.channels.insert("scl".into(), 1);
    setup.channels.insert("sda".into(), 2);
    let claim = i2c::claim(&setup).unwrap();
    assert_eq!(claim.channels, 0b110);
    let mask = 0b1111_1111;
    let factory = CompositeFactory::new(vec![claim], 10);
    let composite = fill(
        factory
            .create(la_devices::ChannelSet::from_u32(mask), 1_000_000, 42)
            .as_mut(),
        5_000,
        5_000,
    );
    let generated = fill(&mut i2c::source(&setup, 42).unwrap(), 5_000, 5_000);
    let random = fill(
        &mut RandomSource::new(la_devices::ChannelSet::from_u32(mask & !0b110), 42, 10),
        5_000,
        5_000,
    );
    for (i, word) in composite.iter().enumerate() {
        assert_eq!(
            word & 0b110,
            generated[i] & 0b110,
            "claimed channels at {i}"
        );
        assert_eq!(word & !0b110, random[i] & !0b110, "other channels at {i}");
    }
    // A setup with a missing channel claims nothing: those channels stay with the random signals.
    setup.channels.remove("sda");
    assert!(i2c::claim(&setup).is_none());
}
