//! The `CAPT` section: the capture (see `contracts/file-format.md`).
//!
//! Encoding walks an immutable snapshot twice: a sizing pass computes the exact byte length of
//! every channel stream (so the channel table and the section length are known before anything is
//! written), then the encoding pass streams the bytes. Decoding reads the table first, checks it
//! against the section and the memory budget, and only then decodes transitions into a
//! [`CaptureRestorer`], which enforces every rule of the capture data model.

use la_core::Result;
use la_storage::{Capture, CaptureConfig, CaptureRestorer, Gap, Limits, RestoredStatus, Snapshot};

use crate::error;
use crate::varint::{Cursor, encode_into, encoded_len};

const FLAG_STOPPED_BY_LIMIT: u8 = 1;
const FLAG_INTERRUPTED: u8 = 2;
const KNOWN_FLAGS: u8 = FLAG_STOPPED_BY_LIMIT | FLAG_INTERRUPTED;

/// Where encoded bytes go.
pub(crate) trait ByteSink {
    /// Accepts the next bytes of the payload.
    fn put(&mut self, data: &[u8]) -> Result<()>;
}

impl ByteSink for Vec<u8> {
    fn put(&mut self, data: &[u8]) -> Result<()> {
        self.extend_from_slice(data);
        Ok(())
    }
}

/// Where payload bytes come from while decoding; reads are bounded by the section.
pub(crate) trait PayloadSource {
    /// Payload bytes not yet read.
    fn remaining(&self) -> u64;
    /// Reads one byte.
    fn u8(&mut self) -> Result<u8>;
    /// Reads a little-endian `u64`.
    fn u64(&mut self) -> Result<u64>;
    /// Reads a canonical varint.
    fn varint(&mut self) -> Result<u64>;
}

impl PayloadSource for Cursor<'_> {
    fn remaining(&self) -> u64 {
        u64::try_from(Cursor::remaining(self)).unwrap_or(u64::MAX)
    }

    fn u8(&mut self) -> Result<u8> {
        Cursor::u8(self)
    }

    fn u64(&mut self) -> Result<u64> {
        Cursor::u64(self)
    }

    fn varint(&mut self) -> Result<u64> {
        Cursor::varint(self)
    }
}

/// What the sizing pass found for one channel.
#[derive(Debug, Clone, Copy)]
struct ChannelPlan {
    initial_high: bool,
    transitions: u64,
    stream_len: u64,
}

/// The exact layout of a `CAPT` payload.
#[derive(Debug)]
pub(crate) struct Layout {
    channels: Vec<ChannelPlan>,
    /// The size of the payload in bytes.
    pub(crate) payload_len: u64,
}

/// Sizing pass: how long every part of the payload will be.
pub(crate) fn plan(snapshot: &Snapshot) -> Result<Layout> {
    let overflow = || error::too_large("the capture section", u64::MAX, u64::MAX);
    let length = snapshot.length();
    let mut payload: u64 = 1 + 1 + 8 + 8;
    let gaps = snapshot.gaps();
    payload = payload
        .checked_add(
            u64::try_from(encoded_len(u64::try_from(gaps.len()).unwrap_or(u64::MAX))).unwrap_or(10),
        )
        .ok_or_else(overflow)?;
    let mut previous_end = 0_u64;
    for gap in gaps {
        let delta = gap.start.saturating_sub(previous_end);
        previous_end = gap.end().unwrap_or(u64::MAX);
        let bytes = encoded_len(delta).saturating_add(encoded_len(gap.length));
        payload = payload
            .checked_add(u64::try_from(bytes).unwrap_or(20))
            .ok_or_else(overflow)?;
    }
    let mut channels = Vec::new();
    for channel in 0..snapshot.channels() {
        let transitions = snapshot.transition_count(channel)?;
        let mut stream_len = 0_u64;
        if transitions > 0 {
            let mut previous = 0_u64;
            for edge in snapshot.transitions(channel, 0, length)? {
                stream_len = stream_len
                    .checked_add(
                        u64::try_from(encoded_len(edge.position.saturating_sub(previous)))
                            .unwrap_or(10),
                    )
                    .ok_or_else(overflow)?;
                previous = edge.position;
            }
        }
        let entry = encoded_len(transitions)
            .saturating_add(encoded_len(stream_len))
            .saturating_add(1);
        payload = payload
            .checked_add(u64::try_from(entry).unwrap_or(21))
            .and_then(|p| p.checked_add(stream_len))
            .ok_or_else(overflow)?;
        channels.push(ChannelPlan {
            initial_high: snapshot.initial_high(channel)?,
            transitions,
            stream_len,
        });
    }
    Ok(Layout {
        channels,
        payload_len: payload,
    })
}

/// Encoding pass: streams the payload described by `layout` into `sink`.
pub(crate) fn encode(snapshot: &Snapshot, layout: &Layout, sink: &mut impl ByteSink) -> Result<()> {
    let status = snapshot.status();
    let mut flags = 0_u8;
    if status.stopped_by_limit {
        flags |= FLAG_STOPPED_BY_LIMIT;
    }
    // A capture that is not closed is still being recorded: it is saved as interrupted.
    if status.interrupted || !status.closed {
        flags |= FLAG_INTERRUPTED;
    }
    let mut head = Vec::with_capacity(64);
    head.push(snapshot.channels());
    head.push(flags);
    head.extend_from_slice(&snapshot.sample_rate_hz().to_le_bytes());
    head.extend_from_slice(&snapshot.length().to_le_bytes());
    let gaps = snapshot.gaps();
    encode_into(u64::try_from(gaps.len()).unwrap_or(u64::MAX), &mut head);
    let mut previous_end = 0_u64;
    for gap in gaps {
        encode_into(gap.start.saturating_sub(previous_end), &mut head);
        encode_into(gap.length, &mut head);
        previous_end = gap.end().unwrap_or(u64::MAX);
    }
    for channel in &layout.channels {
        head.push(u8::from(channel.initial_high));
        encode_into(channel.transitions, &mut head);
        encode_into(channel.stream_len, &mut head);
    }
    sink.put(&head)?;

    let length = snapshot.length();
    let mut buffer = Vec::with_capacity(16 * 1024);
    for (index, channel) in layout.channels.iter().enumerate() {
        if channel.transitions == 0 {
            continue;
        }
        let index = u8::try_from(index).map_err(|_| error::internal("channel index"))?;
        let mut previous = 0_u64;
        let mut written = 0_u64;
        for edge in snapshot.transitions(index, 0, length)? {
            encode_into(edge.position.saturating_sub(previous), &mut buffer);
            previous = edge.position;
            if buffer.len() >= 16 * 1024 - 16 {
                written = written.saturating_add(u64::try_from(buffer.len()).unwrap_or(0));
                sink.put(&buffer)?;
                buffer.clear();
            }
        }
        written = written.saturating_add(u64::try_from(buffer.len()).unwrap_or(0));
        sink.put(&buffer)?;
        buffer.clear();
        if written != channel.stream_len {
            return Err(error::internal(
                "a channel stream differs from its planned size",
            ));
        }
    }
    Ok(())
}

/// Maps an error of the storage restorer to a session error.
fn map_restore(e: &la_core::Error) -> la_core::Error {
    if e.code() == la_storage::error::MEMORY_LIMIT {
        error::memory_budget(e.detail())
    } else {
        error::inconsistent(format!("the capture is not consistent: {}", e.detail()))
    }
}

/// Decodes a `CAPT` payload into a closed capture.
pub(crate) fn decode(source: &mut impl PayloadSource, limits: &Limits) -> Result<Capture> {
    let channels = source.u8()?;
    let flags = source.u8()?;
    if flags & !KNOWN_FLAGS != 0 {
        return Err(error::damaged("a reserved capture flag is set"));
    }
    let sample_rate_hz = source.u64()?;
    let length = source.u64()?;

    // Gaps: every gap takes at least two bytes, which bounds the count by what the section holds.
    let gap_count = source.varint()?;
    if gap_count > source.remaining().checked_div(2).unwrap_or(0) {
        return Err(error::damaged("the gap count does not fit the section"));
    }
    let gap_bytes =
        gap_count.saturating_mul(u64::try_from(std::mem::size_of::<Gap>()).unwrap_or(16));
    if gap_bytes > limits.memory_budget_bytes {
        return Err(error::memory_budget("the gaps alone exceed the budget"));
    }
    let mut gaps = Vec::new();
    let mut previous_end = 0_u64;
    for _ in 0..gap_count {
        let delta = source.varint()?;
        let gap_length = source.varint()?;
        let start = previous_end
            .checked_add(delta)
            .ok_or_else(|| error::damaged("a gap position overflows"))?;
        previous_end = start
            .checked_add(gap_length)
            .ok_or_else(|| error::damaged("a gap position overflows"))?;
        gaps.push(Gap {
            start,
            length: gap_length,
        });
    }

    // The channel table (at most 255 entries, so it cannot be used to allocate much).
    let mut table = Vec::new();
    let mut streams_total = 0_u64;
    for _ in 0..channels {
        let initial = source.u8()?;
        if initial > 1 {
            return Err(error::damaged("an initial level is neither 0 nor 1"));
        }
        let count = source.varint()?;
        let stream_len = source.varint()?;
        if count > stream_len || (count == 0 && stream_len != 0) {
            return Err(error::damaged(
                "a channel stream length does not match its transition count",
            ));
        }
        streams_total = streams_total
            .checked_add(stream_len)
            .ok_or_else(|| error::damaged("the channel streams are longer than any section"))?;
        table.push((initial == 1, count, stream_len));
    }
    if streams_total != source.remaining() {
        return Err(error::damaged(
            "the channel streams do not fill the rest of the section exactly",
        ));
    }

    let mut restorer = CaptureRestorer::new(
        CaptureConfig {
            channels,
            sample_rate_hz,
            limits: *limits,
        },
        length,
        RestoredStatus {
            stopped_by_limit: flags & FLAG_STOPPED_BY_LIMIT != 0,
            interrupted: flags & FLAG_INTERRUPTED != 0,
        },
        gaps,
    )
    .map_err(|e| map_restore(&e))?;
    let counts: Vec<u64> = table.iter().map(|(_, count, _)| *count).collect();
    restorer
        .check_budget(&counts)
        .map_err(|e| map_restore(&e))?;

    for (index, (initial_high, count, stream_len)) in table.into_iter().enumerate() {
        let channel = u8::try_from(index).map_err(|_| error::internal("channel index"))?;
        restorer
            .begin_channel(channel, initial_high, count)
            .map_err(|e| map_restore(&e))?;
        let before = source.remaining();
        let mut previous = 0_u64;
        for _ in 0..count {
            let delta = source.varint()?;
            let position = previous
                .checked_add(delta)
                .ok_or_else(|| error::damaged("a transition position overflows"))?;
            restorer
                .push_transition(position)
                .map_err(|e| map_restore(&e))?;
            previous = position;
        }
        if before.saturating_sub(source.remaining()) != stream_len {
            return Err(error::damaged(
                "a channel stream does not have the length its table entry declares",
            ));
        }
    }
    restorer.finish().map_err(|e| map_restore(&e))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    fn config(channels: u8) -> CaptureConfig {
        CaptureConfig {
            channels,
            sample_rate_hz: 1000,
            limits: Limits::default(),
        }
    }

    fn payload_of(snapshot: &Snapshot) -> Vec<u8> {
        let layout = plan(snapshot).unwrap();
        let mut out = Vec::new();
        encode(snapshot, &layout, &mut out).unwrap();
        assert_eq!(
            out.len() as u64,
            layout.payload_len,
            "the sizing pass is exact"
        );
        out
    }

    fn decode_bytes(bytes: &[u8]) -> Result<Capture> {
        decode(&mut Cursor::new(bytes), &Limits::default())
    }

    /// A hand-built payload.
    #[derive(Clone)]
    struct Payload {
        channels: u8,
        flags: u8,
        rate: u64,
        length: u64,
        gaps: Vec<(u64, u64)>,
        table: Vec<(u8, u64, u64)>,
        streams: Vec<Vec<u64>>,
    }

    impl Payload {
        fn bytes(&self) -> Vec<u8> {
            let mut out = vec![self.channels, self.flags];
            out.extend_from_slice(&self.rate.to_le_bytes());
            out.extend_from_slice(&self.length.to_le_bytes());
            encode_into(self.gaps.len() as u64, &mut out);
            for (delta, length) in &self.gaps {
                encode_into(*delta, &mut out);
                encode_into(*length, &mut out);
            }
            for (initial, count, len) in &self.table {
                out.push(*initial);
                encode_into(*count, &mut out);
                encode_into(*len, &mut out);
            }
            for stream in &self.streams {
                for value in stream {
                    encode_into(*value, &mut out);
                }
            }
            out
        }
    }

    /// 2 channels, 5 samples, a gap of 3, one more sample (the capture of the layout test).
    fn small() -> Payload {
        Payload {
            channels: 2,
            flags: 0,
            rate: 1000,
            length: 9,
            gaps: vec![(5, 3)],
            table: vec![(1, 1, 1), (0, 2, 2)],
            streams: vec![vec![3], vec![2, 6]],
        }
    }

    fn small_snapshot() -> Snapshot {
        let mut c = Capture::new(config(2)).unwrap();
        c.append(0, &[0b01, 0b01, 0b11, 0b10, 0b10]).unwrap();
        c.record_gap(3).unwrap();
        c.append(8, &[0b00]).unwrap();
        c.close().unwrap();
        c.reader().snapshot()
    }

    #[test]
    fn the_layout_is_exactly_the_documented_one() {
        let bytes = payload_of(&small_snapshot());
        let mut expected = vec![2, 0]; // 2 channels; a closed capture with a gap is not "interrupted"
        expected.extend_from_slice(&1000_u64.to_le_bytes());
        expected.extend_from_slice(&9_u64.to_le_bytes());
        expected.extend_from_slice(&[1, 5, 3]); // one gap: start 5, length 3
        expected.extend_from_slice(&[1, 1, 1]); // channel 0: starts high, 1 transition, 1 byte
        expected.extend_from_slice(&[0, 2, 2]); // channel 1: starts low, 2 transitions, 2 bytes
        expected.extend_from_slice(&[3]); // channel 0: position 3
        expected.extend_from_slice(&[2, 6]); // channel 1: positions 2 and 8
        assert_eq!(bytes, expected);
        assert_eq!(bytes, small().bytes());
    }

    #[test]
    fn encode_then_decode_gives_the_same_capture() {
        let snapshot = small_snapshot();
        let decoded = decode_bytes(&payload_of(&snapshot)).unwrap();
        let back = decoded.reader().snapshot();
        assert_eq!(back.length(), 9);
        assert_eq!(back.gaps(), snapshot.gaps());
        assert_eq!(back.transition_count(0).unwrap(), 1);
        assert_eq!(back.transition_count(1).unwrap(), 2);
        assert!(back.initial_high(0).unwrap());
        assert!(back.status().closed);
    }

    #[test]
    fn the_sizing_pass_is_exact_for_random_captures() {
        let mut rng = Rng(77);
        for case in 0..200 {
            let channels = u8::try_from(1 + rng.next() % 32).unwrap();
            let mut c = Capture::new(config(channels)).unwrap();
            let mut word = 0_u32;
            for chunk in 0..(1 + rng.next() % 4) {
                if chunk > 0 && rng.next() % 3 == 0 {
                    c.record_gap(1 + rng.next() % 100).unwrap();
                }
                let len = 1 + usize::try_from(rng.next() % 3000).unwrap();
                let words: Vec<u32> = (0..len)
                    .map(|_| {
                        if rng.next() % 40 == 0 {
                            word ^= u32::try_from(rng.next() & 0xFFFF_FFFF).unwrap();
                        }
                        word
                    })
                    .collect();
                c.append(c.length(), &words).unwrap();
            }
            if case % 2 == 0 {
                c.close().unwrap();
            }
            let snapshot = c.reader().snapshot();
            let bytes = payload_of(&snapshot);
            let decoded = decode_bytes(&bytes).unwrap().reader().snapshot();
            assert_eq!(decoded.length(), snapshot.length());
            for ch in 0..channels {
                assert_eq!(
                    decoded.transition_count(ch).unwrap(),
                    snapshot.transition_count(ch).unwrap()
                );
            }
        }
    }

    #[test]
    fn a_capture_that_is_still_open_is_saved_as_interrupted() {
        let mut c = Capture::new(config(1)).unwrap();
        c.append(0, &[0, 1, 0]).unwrap();
        let bytes = payload_of(&c.reader().snapshot());
        assert_eq!(bytes[1], FLAG_INTERRUPTED);
        let status = decode_bytes(&bytes).unwrap().reader().snapshot().status();
        assert!(status.closed && status.interrupted && !status.complete);
    }

    fn class(bytes: &[u8]) -> &'static str {
        decode_bytes(bytes).unwrap_err().code()
    }

    #[test]
    fn the_hand_built_payload_decodes() {
        assert!(decode_bytes(&small().bytes()).is_ok());
    }

    #[test]
    fn encoding_violations_are_damaged() {
        let mut p = small();
        p.flags = 4;
        assert_eq!(class(&p.bytes()), error::DAMAGED, "reserved flag");
        let mut p = small();
        p.table[0].0 = 2;
        assert_eq!(class(&p.bytes()), error::DAMAGED, "initial level 2");
        let mut p = small();
        p.table[1].2 = 5;
        assert_eq!(
            class(&p.bytes()),
            error::DAMAGED,
            "stream longer than its data"
        );
        let mut p = small();
        p.table[1].2 = 1;
        assert_eq!(
            class(&p.bytes()),
            error::DAMAGED,
            "stream shorter than its data"
        );
        let mut p = small();
        p.table[1].1 = 3;
        assert_eq!(
            class(&p.bytes()),
            error::DAMAGED,
            "count larger than the stream"
        );
        let mut p = small();
        p.table[0] = (1, 0, 1);
        assert_eq!(
            class(&p.bytes()),
            error::DAMAGED,
            "no transitions but a stream"
        );
        let mut bytes = small().bytes();
        bytes.push(0);
        assert_eq!(class(&bytes), error::DAMAGED, "trailing byte");
        let bytes = small().bytes();
        assert_eq!(
            class(&bytes[..bytes.len() - 1]),
            error::DAMAGED,
            "cut short"
        );
        for cut in 0..bytes.len() {
            assert!(decode_bytes(&bytes[..cut]).is_err(), "cut at {cut}");
        }
        let mut p = small();
        p.gaps = vec![(u64::MAX, 5)];
        assert_eq!(class(&p.bytes()), error::DAMAGED, "gap position overflow");
        let mut few = vec![1, 0];
        few.extend_from_slice(&1_u64.to_le_bytes());
        few.extend_from_slice(&10_u64.to_le_bytes());
        encode_into(200, &mut few);
        assert_eq!(
            class(&few),
            error::DAMAGED,
            "more gaps than the section can hold"
        );
    }

    #[test]
    fn content_violations_are_inconsistent() {
        let mut p = small();
        p.channels = 0;
        p.table.clear();
        p.streams.clear();
        assert_eq!(class(&p.bytes()), error::INCONSISTENT, "no channels");
        let mut p = small();
        p.rate = 0;
        assert_eq!(class(&p.bytes()), error::INCONSISTENT, "rate 0");
        let mut p = small();
        p.length = 7;
        assert_eq!(
            class(&p.bytes()),
            error::INCONSISTENT,
            "gap past the length"
        );
        let mut p = small();
        p.gaps = vec![(5, 0)];
        assert_eq!(class(&p.bytes()), error::INCONSISTENT, "empty gap");
        let mut p = small();
        p.gaps = vec![(2, 2), (0, 2)];
        assert_eq!(class(&p.bytes()), error::INCONSISTENT, "adjacent gaps");
        let mut p = small();
        p.length = 0;
        p.gaps.clear();
        assert_eq!(
            class(&p.bytes()),
            error::INCONSISTENT,
            "transitions in an empty capture"
        );
        let mut p = small();
        p.streams[1] = vec![6, 0];
        assert_eq!(class(&p.bytes()), error::INCONSISTENT, "equal positions");
        let mut p = small();
        p.streams[0] = vec![0];
        assert_eq!(class(&p.bytes()), error::INCONSISTENT, "position 0");
        let mut p = small();
        p.streams[0] = vec![5];
        assert_eq!(class(&p.bytes()), error::INCONSISTENT, "inside the gap");
        let mut p = small();
        p.streams[1] = vec![2, 100];
        assert_eq!(class(&p.bytes()), error::INCONSISTENT, "beyond the length");
        // A capture that is one big gap has no stored sample: its channels start low.
        let all_gap = Payload {
            channels: 1,
            flags: 2,
            rate: 1,
            length: 10,
            gaps: vec![(0, 10)],
            table: vec![(1, 0, 0)],
            streams: vec![vec![]],
        };
        assert_eq!(class(&all_gap.bytes()), error::INCONSISTENT);
        let mut ok = all_gap.clone();
        ok.table[0].0 = 0;
        assert!(decode_bytes(&ok.bytes()).is_ok());
    }

    #[test]
    fn lying_counts_are_rejected_without_allocating_from_them() {
        let mut p = small();
        p.table[0] = (1, u64::MAX, u64::MAX);
        assert_eq!(class(&p.bytes()), error::DAMAGED);
        let mut p = small();
        p.table[0] = (1, 1, u64::MAX);
        p.table[1] = (0, 2, 2);
        assert_eq!(class(&p.bytes()), error::DAMAGED);
        // A gap count of u64::MAX.
        let mut bytes = vec![1, 0];
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&10_u64.to_le_bytes());
        encode_into(u64::MAX, &mut bytes);
        assert_eq!(class(&bytes), error::DAMAGED);
    }

    #[test]
    fn a_capture_that_does_not_fit_the_budget_is_rejected_before_its_transitions_are_read() {
        // 300,000 transitions of one channel cost more than 1 MiB.
        let count = 300_000_u64;
        let stream: Vec<u64> = vec![1; usize::try_from(count).unwrap()];
        let p = Payload {
            channels: 1,
            flags: 0,
            rate: 1000,
            length: count + 10,
            gaps: vec![],
            table: vec![(0, count, count)],
            streams: vec![stream],
        };
        let bytes = p.bytes();
        let tiny = Limits {
            memory_budget_bytes: 1024 * 1024,
            ..Limits::default()
        };
        assert_eq!(
            decode(&mut Cursor::new(&bytes), &tiny).unwrap_err().code(),
            error::MEMORY_BUDGET
        );
        assert!(decode(&mut Cursor::new(&bytes), &Limits::default()).is_ok());
    }
}
