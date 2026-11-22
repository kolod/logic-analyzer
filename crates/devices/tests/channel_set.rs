//! `ChannelSet` against a plain `BTreeSet` model (feature 017, task T008).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::collections::BTreeSet;

use la_devices::ChannelSet;
use la_storage::MAX_CHANNELS;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn same(set: &ChannelSet, model: &BTreeSet<u8>) {
    assert_eq!(
        set.iter().collect::<Vec<_>>(),
        model.iter().copied().collect::<Vec<_>>()
    );
    assert_eq!(set.count(), model.len());
    assert_eq!(set.is_empty(), model.is_empty());
    assert_eq!(set.highest(), model.iter().next_back().copied());
    for c in 0..=255_u8 {
        assert_eq!(set.contains(c), model.contains(&c), "channel {c}");
    }
}

#[test]
fn random_operations_agree_with_the_model() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut set, mut model) = (ChannelSet::empty(), BTreeSet::new());
    for step in 0..4000 {
        let c = (rng.next() % 256) as u8;
        match rng.next() % 3 {
            0 => {
                set.insert(c);
                if c < MAX_CHANNELS {
                    model.insert(c);
                }
            }
            1 => {
                set.remove(c);
                model.remove(&c);
            }
            _ => {}
        }
        if step % 50 == 0 {
            same(&set, &model);
        }
    }
    same(&set, &model);
}

#[test]
fn the_bound_is_max_channels_and_all_takes_the_first_n() {
    let mut set = ChannelSet::empty();
    set.insert(MAX_CHANNELS);
    set.insert(255);
    assert!(set.is_empty());
    set.insert(MAX_CHANNELS - 1);
    assert_eq!(set.highest(), Some(159));
    for n in [0_u8, 1, 31, 32, 33, 64, 159, 160, 200] {
        let all = ChannelSet::all(n);
        let want: Vec<u8> = (0..n.min(MAX_CHANNELS)).collect();
        assert_eq!(all.iter().collect::<Vec<_>>(), want, "all({n})");
    }
}

#[test]
fn and_keeps_the_common_channels() {
    let mut a = ChannelSet::empty();
    let mut b = ChannelSet::empty();
    for c in [0, 5, 31, 32, 100, 159] {
        a.insert(c);
    }
    for c in [5, 6, 32, 99, 159] {
        b.insert(c);
    }
    assert_eq!(a.and(&b).iter().collect::<Vec<_>>(), vec![5, 32, 159]);
}

#[test]
fn the_first_word_converts_both_ways() {
    let set = ChannelSet::from_u32(0b1010_0001);
    assert_eq!(set.iter().collect::<Vec<_>>(), vec![0, 5, 7]);
    assert_eq!(set.to_u32(), 0b1010_0001);
    let mut wide = set;
    wide.insert(40);
    assert_eq!(
        wide.to_u32(),
        0b1010_0001,
        "channels above 31 are not in the first word"
    );
    assert_eq!(wide.word(1), 1 << 8);
    assert_eq!(ChannelSet::from_u32(u32::MAX), ChannelSet::all(32));
}

#[test]
fn it_is_a_copy_value_with_equality() {
    let a = ChannelSet::all(10);
    let b = a;
    assert_eq!(a, b);
    assert_ne!(a, ChannelSet::all(11));
}
