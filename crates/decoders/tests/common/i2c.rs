//! Building I2C bus signals and reading recorded ones, for the tests of feature 008.
//!
//! The **bus builder** turns a list of bus events into samples with 16 samples per SCL period, exactly
//! as `specs/008-i2c-decoder-generator/contracts/i2c-generator.md` describes the bus: in a bit slot
//! SCL is low for samples 0..4, high for 4..12 and low for 12..16 and SDA holds the bit for all 16, so
//! the rising edge of SCL (the sampling edge) is at slot sample 4 and the falling edge at sample 12.
//! It is written independently of the decoder and of the generator; tests state their expectations
//! by hand from the positions it reports.
//!
//! The **fixture format** (a recorded signal and what a decode of it must give) is one text file:
//!
//! ```text
//! # a comment
//! 11 64              <- a run: SDA level, SCL level, number of samples
//! 01 4
//! --- expect         <- what follows is the expectation, one line each
//! lane range 68 85 ann-i2c-address-write 0x50
//! lane point 85 ann-i2c-ack
//! ch0 edge 72 rising
//! lane point 85 ann-i2c-nak error
//! ```
//!
//! An expectation line is `<target> <kind> <positions> <key> <arguments...> [error]` where the target
//! is `lane` or `chN` (an overlay on channel N); the kinds are `range <start> <end>`, `point <at>`
//! and `edge <at> <rising|falling>`; marks without text name no key. SCL is channel 0 and SDA channel
//! 1 (see [`SCL`] and [`SDA`]).
#![allow(dead_code)]

/// The channel the helpers put SCL on.
pub const SCL: u8 = 0;
/// The channel the helpers put SDA on.
pub const SDA: u8 = 1;
/// Samples in one SCL period.
pub const SLOT: u64 = 16;

/// Where the parts of a byte are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct At {
    /// The SCL rising edge of the first bit.
    pub first: u64,
    /// The SCL rising edge of the acknowledge bit.
    pub ack: u64,
    /// The SCL falling edge that ends the acknowledge bit.
    pub fall: u64,
}

/// A bus signal under construction: `(sda, scl)` per sample.
#[derive(Debug, Clone, Default)]
pub struct Bus {
    /// The samples.
    pub samples: Vec<(bool, bool)>,
}

impl Bus {
    /// An empty signal.
    pub fn new() -> Self {
        Self::default()
    }

    /// The position the next sample gets.
    pub fn pos(&self) -> u64 {
        self.samples.len() as u64
    }

    /// `n` samples of the given levels.
    pub fn raw(&mut self, sda: bool, scl: bool, n: u64) -> &mut Self {
        for _ in 0..n {
            self.samples.push((sda, scl));
        }
        self
    }

    /// `n` samples with both lines high.
    pub fn idle(&mut self, n: u64) -> &mut Self {
        self.raw(true, true, n)
    }

    /// A start condition: SDA falls while SCL is high. Returns the position of the SDA edge.
    pub fn start(&mut self) -> u64 {
        let at = self.pos();
        self.raw(false, true, 4);
        at
    }

    /// One clock pulse carrying `bit`. Returns the position of the SCL rising edge.
    pub fn bit(&mut self, bit: bool) -> u64 {
        let at = self.pos() + 4;
        self.raw(bit, false, 4);
        self.raw(bit, true, 8);
        self.raw(bit, false, 4);
        at
    }

    /// Eight bits, most significant first, and an acknowledge bit (`ack` true is a low SDA).
    pub fn byte(&mut self, value: u8, ack: bool) -> At {
        let mut first = 0;
        for i in 0..8 {
            let at = self.bit(value >> (7 - i) & 1 != 0);
            if i == 0 {
                first = at;
            }
        }
        let ack_at = self.bit(!ack);
        At {
            first,
            ack: ack_at,
            fall: ack_at + 8,
        }
    }

    /// The first `bits` bits of a byte and nothing else (SCL left low).
    pub fn bits(&mut self, value: u8, bits: u32) -> &mut Self {
        for i in 0..bits {
            self.bit(value >> (7 - i) & 1 != 0);
        }
        self
    }

    /// A repeated start: SDA is raised while SCL is low, SCL rises, SDA falls while SCL is high.
    /// Returns the position of the SDA falling edge.
    pub fn repeated_start(&mut self) -> u64 {
        self.raw(true, false, 4);
        self.raw(true, true, 4);
        let at = self.pos();
        self.raw(false, true, 4);
        at
    }

    /// A stop condition: SDA is lowered while SCL is low, SCL rises, SDA rises while SCL is high.
    /// Returns the position of the SDA rising edge (the first sample of the idle that follows).
    pub fn stop(&mut self) -> u64 {
        self.raw(false, false, 4);
        self.raw(false, true, 4);
        let at = self.pos();
        self.raw(true, true, 1);
        at
    }

    /// The sample words: SCL on [`SCL`], SDA on [`SDA`].
    pub fn words(&self) -> Vec<u32> {
        self.samples
            .iter()
            .map(|(sda, scl)| (u32::from(*scl) << SCL) | (u32::from(*sda) << SDA))
            .collect()
    }
}

/// A line of the expected result of a recorded signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected {
    /// `None` for the lane, the channel of an overlay otherwise.
    pub channel: Option<u8>,
    /// `range`, `point` or `edge`.
    pub kind: String,
    /// The positions of the annotation.
    pub positions: Vec<u64>,
    /// `Some(true)` for an edge mark that rises.
    pub rising: Option<bool>,
    /// The message key, empty for marks without text.
    pub key: String,
    /// The arguments of the message.
    pub args: Vec<String>,
    /// The error flag.
    pub error: bool,
}

/// A recorded signal and what a decode of it must give.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// The samples (SCL on channel 0, SDA on channel 1).
    pub words: Vec<u32>,
    /// The expected annotations.
    pub expected: Vec<Expected>,
}

/// Why a fixture was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed(pub String);

/// The most samples a fixture may hold, so that a damaged file cannot ask for gigabytes.
pub const MAX_SAMPLES: u64 = 50_000_000;

/// Reads a fixture from its text.
pub fn parse_fixture(text: &str) -> Result<Fixture, Malformed> {
    let bad = |line: usize, why: &str| Malformed(format!("line {}: {why}", line + 1));
    let mut words: Vec<u32> = Vec::new();
    let mut expected = Vec::new();
    let mut in_expect = false;
    for (number, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "--- expect" {
            in_expect = true;
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if in_expect {
            expected.push(parse_expected(&parts).map_err(|why| bad(number, &why))?);
            continue;
        }
        let [levels, count] = parts.as_slice() else {
            return Err(bad(number, "a run is `<sda><scl> <count>`"));
        };
        let word = match *levels {
            "00" => 0,
            "01" => 1 << SCL,
            "10" => 1 << SDA,
            "11" => (1 << SCL) | (1 << SDA),
            _ => return Err(bad(number, "levels are 00, 01, 10 or 11")),
        };
        let count: u64 = count.parse().map_err(|_| bad(number, "a run length"))?;
        if count == 0 || count > MAX_SAMPLES || words.len() as u64 + count > MAX_SAMPLES {
            return Err(bad(number, "a run length out of range"));
        }
        words.extend(std::iter::repeat_n(word, count as usize));
    }
    if !in_expect {
        return Err(Malformed("the `--- expect` section is missing".into()));
    }
    if words.is_empty() {
        return Err(Malformed("no samples".into()));
    }
    Ok(Fixture { words, expected })
}

fn parse_expected(parts: &[&str]) -> Result<Expected, String> {
    let mut it = parts.iter().copied();
    let target = it.next().ok_or("empty line")?;
    let channel = match target {
        "lane" => None,
        t => Some(
            t.strip_prefix("ch")
                .and_then(|n| n.parse::<u8>().ok())
                .ok_or("the target is `lane` or `chN`")?,
        ),
    };
    let kind = it.next().ok_or("a kind")?.to_owned();
    let mut number = |what: &str| -> Result<u64, String> {
        it.next()
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| format!("a position for {what}"))
    };
    let (positions, rising) = match kind.as_str() {
        "range" => (vec![number("start")?, number("end")?], None),
        "point" => (vec![number("at")?], None),
        "edge" => {
            let at = number("at")?;
            let rising = match it.next() {
                Some("rising") => true,
                Some("falling") => false,
                _ => return Err("`rising` or `falling`".into()),
            };
            (vec![at], Some(rising))
        }
        _ => return Err("the kind is range, point or edge".into()),
    };
    let rest: Vec<&str> = it.collect();
    let (error, rest) = match rest.split_last() {
        Some((&"error", head)) => (true, head.to_vec()),
        _ => (false, rest),
    };
    let (key, args) = match rest.split_first() {
        Some((key, args)) => (
            (*key).to_owned(),
            args.iter().map(|a| (*a).to_owned()).collect(),
        ),
        None => (String::new(), Vec::new()),
    };
    Ok(Expected {
        channel,
        kind,
        positions,
        rising,
        key,
        args,
        error,
    })
}

/// Reads `tests/fixtures/i2c/<name>.txt`.
pub fn load_fixture(name: &str) -> Fixture {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/i2c")
        .join(format!("{name}.txt"));
    let text = std::fs::read_to_string(&path).unwrap();
    parse_fixture(&text).unwrap_or_else(|e| panic!("{name}: {}", e.0))
}
