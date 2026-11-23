//! The device information reply (command 0): text lines, the original's and the new ones.

/// What a board offers beyond the original, from the `FEATURES:` line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Features {
    /// Capture request 2 (command 7) is understood.
    pub cmd7: bool,
    /// The compact coding of samples is available.
    pub compact: bool,
    /// The board has external memory.
    pub psram: bool,
    /// Capture mode 3 (up to 32 channels) is available.
    pub mode32: bool,
}

/// The triggers a board supports, from the `TRIGGERS:` line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TriggerSupport {
    /// An edge trigger.
    pub edge: bool,
    /// The most channels of a pattern trigger, 0 for none.
    pub pattern: u8,
    /// The most channels of a fast pattern trigger, 0 for none.
    pub fast: u8,
    /// The most bursts, 0 for none.
    pub burst: u8,
    /// Blast.
    pub blast: bool,
}

/// Why the information could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoError {
    /// The first line is not `LOGIC_ANALYZER_<board>_<version>`.
    NotAnalyzer,
    /// A line the reply must have is missing.
    Missing(&'static str),
    /// A line has a value that is not valid.
    BadValue(&'static str),
}

impl core::fmt::Display for InfoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotAnalyzer => {
                f.write_str("the device does not identify itself as a logic analyzer")
            }
            Self::Missing(name) => write!(f, "the line {name} is missing"),
            Self::BadValue(name) => write!(f, "the line {name} has a value that is not valid"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for InfoError {}

/// What a board reports about itself. Lines this crate does not know are ignored, so a newer firmware still
/// reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// The highest sample rate in hertz.
    pub frequency: u32,
    /// The highest rate of blast in hertz.
    pub blast_frequency: u32,
    /// The buffer of the internal memory, in samples.
    pub buffer: u32,
    /// The number of channels.
    pub channels: u8,
    /// The firmware version (`FW:`), `None` for a firmware that does not report one.
    pub firmware: Option<(u8, u8, u8)>,
    /// The protocol extension level (`PROTO:`), 1 for the original.
    pub proto: u8,
    /// What the board offers beyond the original.
    pub features: Features,
    /// The size of the external memory in bytes, 0 for none.
    pub psram_bytes: u64,
    /// The highest rate at which a capture of 32 channels can go to the external memory.
    pub psram_rate: u32,
    /// The most samples before the trigger, by mode.
    pub max_before: [u32; 4],
    /// The most samples after the trigger, by mode.
    pub max_after: [u32; 4],
    /// The triggers.
    pub triggers: TriggerSupport,
}

impl DeviceInfo {
    /// Reads the lines of the reply.
    ///
    /// # Errors
    ///
    /// [`InfoError`] for a reply that is not from an analyzer, lacks `FREQ:` or `CHANNELS:`, or has a value
    /// that is not valid.
    pub fn parse(text: &str) -> Result<Self, InfoError> {
        let mut lines = text
            .lines()
            .map(|l| l.trim_end_matches('\r'))
            .filter(|l| !l.is_empty());
        if !lines
            .next()
            .is_some_and(|l| l.starts_with("LOGIC_ANALYZER_"))
        {
            return Err(InfoError::NotAnalyzer);
        }
        let mut info = Self {
            frequency: 0,
            blast_frequency: 0,
            buffer: 0,
            channels: 0,
            firmware: None,
            proto: 1,
            features: Features::default(),
            psram_bytes: 0,
            psram_rate: 0,
            max_before: [0; 4],
            max_after: [0; 4],
            triggers: TriggerSupport::default(),
        };
        let (mut has_freq, mut has_channels) = (false, false);
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            match name {
                "FREQ" => {
                    info.frequency = number(value, "FREQ")?;
                    has_freq = true;
                }
                "BLASTFREQ" => info.blast_frequency = number(value, "BLASTFREQ")?,
                "BUFFER" => info.buffer = number(value, "BUFFER")?,
                "CHANNELS" => {
                    info.channels = number(value, "CHANNELS")?;
                    has_channels = true;
                }
                "FW" => info.firmware = Some(version(value)?),
                "PROTO" => info.proto = number(value, "PROTO")?,
                "FEATURES" => info.features = features(value),
                "PSRAM" => info.psram_bytes = number(value, "PSRAM")?,
                "PSRAMRATE" => info.psram_rate = number(value, "PSRAMRATE")?,
                "MAXPRE" => info.max_before = four(value, "MAXPRE")?,
                "MAXPOST" => info.max_after = four(value, "MAXPOST")?,
                "TRIGGERS" => info.triggers = triggers(value)?,
                _ => {}
            }
        }
        if !has_freq {
            return Err(InfoError::Missing("FREQ"));
        }
        if !has_channels {
            return Err(InfoError::Missing("CHANNELS"));
        }
        Ok(info)
    }
}

fn number<T: core::str::FromStr>(value: &str, name: &'static str) -> Result<T, InfoError> {
    value.trim().parse().map_err(|_| InfoError::BadValue(name))
}

fn version(value: &str) -> Result<(u8, u8, u8), InfoError> {
    let mut parts = value.trim().split('.');
    let mut next = || -> Result<u8, InfoError> {
        parts
            .next()
            .ok_or(InfoError::BadValue("FW"))?
            .parse()
            .map_err(|_| InfoError::BadValue("FW"))
    };
    let version = (next()?, next()?, next()?);
    if parts.next().is_some() {
        return Err(InfoError::BadValue("FW"));
    }
    Ok(version)
}

fn features(value: &str) -> Features {
    let mut features = Features::default();
    for name in value.split(',').map(str::trim) {
        match name {
            "cmd7" => features.cmd7 = true,
            "compact" => features.compact = true,
            "psram" => features.psram = true,
            "mode32" => features.mode32 = true,
            _ => {}
        }
    }
    features
}

fn four(value: &str, name: &'static str) -> Result<[u32; 4], InfoError> {
    let mut out = [0_u32; 4];
    let mut parts = value.split(',');
    for slot in &mut out {
        *slot = number(parts.next().ok_or(InfoError::BadValue(name))?, name)?;
    }
    if parts.next().is_some() {
        return Err(InfoError::BadValue(name));
    }
    Ok(out)
}

fn triggers(value: &str) -> Result<TriggerSupport, InfoError> {
    let mut support = TriggerSupport::default();
    for item in value.split(',').map(str::trim).filter(|i| !i.is_empty()) {
        let (name, argument) = item.split_once(':').unwrap_or((item, ""));
        match name {
            "edge" => support.edge = true,
            "blast" => support.blast = true,
            "pattern" => support.pattern = number(argument, "TRIGGERS")?,
            "fast" => support.fast = number(argument, "TRIGGERS")?,
            "burst" => support.burst = number(argument, "TRIGGERS")?,
            _ => {}
        }
    }
    Ok(support)
}
