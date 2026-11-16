//! The fixture loader of the I2C tests refuses damaged files (spec 008, T002).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

mod common;

use common::i2c::{Bus, SCL, SDA, parse_fixture};

#[test]
fn a_good_fixture_is_read() {
    let fixture = parse_fixture(
        "# one start condition\n11 4\n01 2\n--- expect\nlane point 4 ann-i2c-start\nch1 edge 5 rising\nlane point 6 ann-i2c-nak error\nlane range 1 9 ann-i2c-address-write 0x50\n",
    )
    .unwrap();
    assert_eq!(fixture.words.len(), 6);
    assert_eq!(fixture.words[0], (1 << SCL) | (1 << SDA));
    assert_eq!(fixture.words[4], 1 << SCL);
    assert_eq!(fixture.expected.len(), 4);
    assert_eq!(fixture.expected[0].key, "ann-i2c-start");
    assert_eq!(fixture.expected[1].channel, Some(1));
    assert_eq!(fixture.expected[1].rising, Some(true));
    assert!(fixture.expected[2].error);
    assert_eq!(fixture.expected[3].args, vec!["0x50".to_owned()]);
    assert_eq!(fixture.expected[3].positions, vec![1, 9]);
}

#[test]
fn damaged_fixtures_are_refused() {
    for text in [
        "11 4\n",                                   // no expectation section
        "--- expect\n",                             // no samples
        "12 4\n--- expect\n",                       // a level that does not exist
        "11 x\n--- expect\n",                       // a length that is no number
        "11 0\n--- expect\n",                       // an empty run
        "11 99999999999\n--- expect\n",             // a run that is too long
        "11\n--- expect\n",                         // a missing length
        "11 4\n--- expect\nlane\n",                 // a missing kind
        "11 4\n--- expect\nlane blob 3 x\n",        // an unknown kind
        "11 4\n--- expect\nlane range 3 x\n",       // a position that is no number
        "11 4\n--- expect\nch edge 3 rising\n",     // a target without a channel
        "11 4\n--- expect\nlane edge 3 sideways\n", // a direction that does not exist
    ] {
        assert!(parse_fixture(text).is_err(), "accepted: {text:?}");
    }
}

#[test]
fn the_builder_puts_the_edges_where_it_says() {
    let mut bus = Bus::new();
    bus.idle(8);
    let start = bus.start();
    let byte = bus.byte(0xA0, true);
    let stop = bus.stop();
    let words = bus.words();
    let scl = |p: u64| words[p as usize] >> SCL & 1;
    let sda = |p: u64| words[p as usize] >> SDA & 1;
    // The start: SDA falls with SCL high.
    assert_eq!((sda(start - 1), sda(start), scl(start)), (1, 0, 1));
    // The first bit (a 1) is sampled on a rising edge of SCL while SDA is high.
    assert_eq!(
        (scl(byte.first - 1), scl(byte.first), sda(byte.first)),
        (0, 1, 1)
    );
    // The acknowledge bit is low and ends with a falling edge.
    assert_eq!((scl(byte.ack), sda(byte.ack)), (1, 0));
    assert_eq!((scl(byte.fall - 1), scl(byte.fall)), (1, 0));
    // The stop: SDA rises with SCL high.
    assert_eq!((sda(stop - 1), sda(stop), scl(stop)), (0, 1, 1));
    assert_eq!(byte.ack - byte.first, 8 * 16);
}
