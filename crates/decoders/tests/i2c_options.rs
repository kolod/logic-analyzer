//! The options of the I2C decoder and 10-bit addresses (spec 008, user story 4, task T028).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::i2c::{
    Bus, Decoded, Expected, GREEN, HALF, SDA, YELLOW, ack_end, byte_arrows, byte_lane, decode_with,
    dot, part, read_tx, sorted, start_block, stop_block, whole, write_tx,
};
use la_decoders::decoders::i2c::items;
use la_decoders::model::{Item, Payload};
use la_decoders::registry::Registry;
use la_decoders::run::ChunkLimits;
use la_decoders::setup::{Setup, Value};

fn choice(value: &str) -> Value {
    Value::Choice(value.to_owned())
}

fn decode_with_options(words: &[u32], options: &[(&str, Value)]) -> Decoded {
    decode_with(words, options, &ChunkLimits::default())
}

/// A bus with a write and a read, and the expectations of both with the default options.
fn two_transactions() -> (Vec<u32>, Vec<Expected>) {
    let mut bus = Bus::new();
    bus.idle(64);
    let mut expected = write_tx(&mut bus, 0x50, &[0x12, 0x34]);
    bus.idle(64);
    expected.extend(read_tx(&mut bus, 0x51, &[0x01, 0x02]));
    bus.idle(64);
    (bus.words(), expected)
}

#[test]
fn the_defaults_are_the_seven_bit_address_and_hexadecimal() {
    let (words, expected) = two_transactions();
    assert_eq!(
        decode_with_options(&words, &[]).all,
        sorted(expected.clone())
    );
    // Saying so does the same.
    let said = decode_with_options(
        &words,
        &[("address", choice("addr7")), ("radix", choice("hex"))],
    );
    assert_eq!(said.all, sorted(expected));
}

#[test]
fn the_eight_bit_notation_adds_the_direction_bit_and_changes_nothing_else() {
    let (words, expected) = two_transactions();
    let decoded = decode_with_options(&words, &[("address", choice("addr8"))]);
    // Only the text of addresses and transactions differs: 0x50 write is 0xA0, 0x51 read is 0xA3.
    let rewrite = |mut e: Expected| {
        let read = e.key.ends_with("read");
        match e.key.as_str() {
            "ann-i2c-address-write"
            | "ann-i2c-address-read"
            | "ann-i2c-transaction-write"
            | "ann-i2c-transaction-read" => {
                let address = u8::from_str_radix(e.args[0].trim_start_matches("0x"), 16).unwrap();
                e.args = vec![format!("0x{:02X}", address << 1 | u8::from(read))];
            }
            _ => {}
        }
        e
    };
    let wanted: Vec<Expected> = expected.into_iter().map(rewrite).collect();
    assert_eq!(decoded.all, sorted(wanted));
    // The items carry the address without the direction bit, whatever is shown.
    let Payload::Bytes(first) = &decoded.items[1].payload else {
        panic!()
    };
    assert_eq!((first[0], first[4], first[5]), (items::ADDRESS, 0, 0x50));
    assert!(decoded.all.iter().any(|e| e.args == ["0xA0"]));
    assert!(decoded.all.iter().any(|e| e.args == ["0xA3"]));
}

#[test]
fn the_number_base_formats_addresses_and_bytes() {
    let mut bus = Bus::new();
    bus.idle(64);
    write_tx(&mut bus, 0x50, &[0x12, 0x34]);
    bus.idle(64);
    let words = bus.words();
    let texts = |options: &[(&str, Value)]| -> Vec<String> {
        let mut texts: Vec<String> = decode_with_options(&words, options)
            .lane()
            .into_iter()
            .filter(|e| e.key.contains("address") || e.key == "ann-i2c-data")
            .map(|e| e.args[0].clone())
            .collect();
        texts.sort();
        texts
    };
    assert_eq!(texts(&[("radix", choice("dec"))]), ["18", "52", "80"]);
    assert_eq!(
        texts(&[("radix", choice("bin"))]),
        ["0b00010010", "0b00110100", "0b1010000"]
    );
    assert_eq!(
        texts(&[("radix", choice("bin")), ("address", choice("addr8"))]),
        ["0b00010010", "0b00110100", "0b10100000"]
    );
    assert_eq!(
        texts(&[("radix", choice("dec")), ("address", choice("addr8"))]),
        ["160", "18", "52"]
    );
}

/// A write to a 10-bit address: the header byte, the low byte of the address, then the data.
fn ten_bit_write(bus: &mut Bus, address: u16, data: &[u8]) -> Vec<Expected> {
    let mut expected = Vec::new();
    let start = bus.start();
    expected.push(start_block(start, "ann-i2c-start"));
    expected.push(dot(SDA, start, "ann-i2c-start"));
    let header_value = 0xF0 | ((address >> 8) as u8) << 1;
    let header = bus.byte(header_value, true);
    let low_value = (address & 0xFF) as u8;
    let low = bus.byte(low_value, true);
    let text = format!("0x{address:02X}");
    // One address block for each of the two bytes, both naming the whole address, and one acknowledge
    // block for each.
    for at in [header, low] {
        expected.extend(byte_lane(
            at,
            "ann-i2c-address-write",
            &text,
            YELLOW,
            "ann-i2c-ack",
            false,
        ));
    }
    expected.extend(byte_arrows(header, header_value, "ann-i2c-ack"));
    expected.extend(byte_arrows(low, low_value, "ann-i2c-ack"));
    let mut last_ack_end = ack_end(low);
    for value in data {
        let at = bus.byte(*value, true);
        last_ack_end = ack_end(at);
        expected.extend(byte_lane(
            at,
            "ann-i2c-data",
            &format!("0x{value:02X}"),
            YELLOW,
            "ann-i2c-ack",
            false,
        ));
        expected.extend(byte_arrows(at, *value, "ann-i2c-ack"));
    }
    let stop = bus.stop();
    expected.push(stop_block(last_ack_end, stop));
    expected.push(dot(SDA, stop, "ann-i2c-stop"));
    expected.push(whole(
        start - HALF,
        stop,
        GREEN,
        "ann-i2c-transaction-write",
        &[&text],
    ));
    expected
}

fn address_items(items: &[Item]) -> Vec<(u8, u8, u8, u8, u16, u64, u64)> {
    items
        .iter()
        .filter_map(|item| {
            let Payload::Bytes(b) = &item.payload else {
                return None;
            };
            (b[0] == items::ADDRESS).then(|| {
                (
                    b[0],
                    b[1],
                    b[2],
                    b[3],
                    u16::from(b[4]) << 8 | u16::from(b[5]),
                    item.start,
                    item.end,
                )
            })
        })
        .collect()
}

#[test]
fn a_ten_bit_address_is_one_address_with_the_direction() {
    let mut bus = Bus::new();
    bus.idle(64);
    let expected = ten_bit_write(&mut bus, 0x123, &[0x55, 0x66]);
    bus.idle(64);
    let decoded = decode_with_options(&bus.words(), &[]);
    assert_eq!(decoded.all, sorted(expected));
    // One address item over both bytes, flagged as a 10-bit address, then the data bytes.
    let addresses = address_items(&decoded.items);
    assert_eq!(addresses.len(), 1);
    let (_, direction, ack, flags, value, ..) = addresses[0];
    assert_eq!(
        (direction, ack, flags, value),
        (items::WRITE, items::ACK, items::TEN_BIT, 0x123)
    );
    assert!(decoded.all.iter().all(|e| !e.error));
}

#[test]
fn the_read_part_of_a_ten_bit_transaction_shows_the_address_already_known() {
    let mut bus = Bus::new();
    bus.idle(64);
    let start = bus.start();
    let header = bus.byte(0xF0 | 1 << 1, true); // 11110 01 0: the high bits are 0b01
    let low = bus.byte(0x23, true);
    let restart = bus.repeated_start();
    let read_header = bus.byte(0xF0 | 1 << 1 | 1, true); // 11110 01 1
    let data = bus.byte(0x99, false);
    let stop = bus.stop();
    bus.idle(64);
    let decoded = decode_with_options(&bus.words(), &[]);
    let lane = decoded.lane();
    // The write part: the header and the low byte are two address blocks that both name the address.
    for at in [header, low] {
        assert!(lane.contains(&part(
            at.first - HALF,
            at.ack - HALF,
            YELLOW,
            "ann-i2c-address-write",
            &["0x123"]
        )));
    }
    // The read part (green) after a repeated start block, and the data block that ends it.
    assert!(lane.contains(&part(
        read_header.first - HALF,
        read_header.ack - HALF,
        GREEN,
        "ann-i2c-address-read",
        &["0x123"]
    )));
    assert!(lane.contains(&start_block(restart, "ann-i2c-repeated-start")));
    assert!(lane.contains(&whole(
        start - HALF,
        stop,
        GREEN,
        "ann-i2c-transaction-write",
        &["0x123"]
    )));
    assert!(lane.contains(&part(
        data.first - HALF,
        data.ack - HALF,
        GREEN,
        "ann-i2c-data",
        &["0x99"]
    )));
    // One ACK block for each of the three address bytes; the NAK that ends the read is a NAK block.
    assert_eq!(lane.iter().filter(|e| e.key == "ann-i2c-ack").count(), 3);
    assert_eq!(lane.iter().filter(|e| e.key == "ann-i2c-nak").count(), 1);
    assert!(decoded.all.iter().all(|e| !e.error));
    let addresses = address_items(&decoded.items);
    assert_eq!(
        addresses
            .iter()
            .map(|a| (a.1, a.3, a.4))
            .collect::<Vec<_>>(),
        vec![
            (items::WRITE, items::TEN_BIT, 0x123),
            (items::READ, items::TEN_BIT, 0x123),
        ]
    );
    assert_eq!(
        (addresses[1].5, addresses[1].6),
        (read_header.first, read_header.ack + 1)
    );
}

#[test]
fn a_ten_bit_read_header_with_no_write_before_it_is_an_ordinary_address() {
    let mut bus = Bus::new();
    bus.idle(64);
    let expected = {
        let mut e = read_tx(&mut bus, 0x79, &[0x01, 0x02]);
        e.retain(|_| true);
        e
    };
    bus.idle(64);
    let decoded = decode_with_options(&bus.words(), &[]);
    assert_eq!(decoded.all, sorted(expected));
    assert!(decoded.all.iter().all(|e| !e.error));
}

#[test]
fn option_values_that_do_not_exist_are_refused_before_a_run() {
    let registry = Registry::builtin();
    for (option, bad) in [("address", "addr9"), ("radix", "octal"), ("radix", "")] {
        let mut setup = Setup::new(1, "i2c");
        setup.channels.insert("scl".into(), 0);
        setup.channels.insert("sda".into(), 1);
        setup.options.insert(option.to_owned(), choice(bad));
        let error = setup.validate(&registry, &[], 2).unwrap_err();
        assert_eq!(
            error.code(),
            la_decoders::error::INVALID_SETUP,
            "{option}={bad}"
        );
    }
    let mut setup = Setup::new(1, "i2c");
    setup.channels.insert("scl".into(), 0);
    setup.channels.insert("sda".into(), 1);
    setup
        .options
        .insert("address".to_owned(), Value::Integer(7));
    assert!(setup.validate(&registry, &[], 2).is_err());
    setup.options.clear();
    setup.options.insert("radix".to_owned(), choice("bin"));
    setup.validate(&registry, &[], 2).unwrap();
}
