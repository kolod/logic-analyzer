//! Capture request 2 (command 7): the 58-byte payload and its checks.

use crate::info::DeviceInfo;
use crate::samples::bytes_per_sample;

/// The length of the payload of capture request 2.
pub const REQUEST2_LEN: usize = 58;

/// The most channels of one board.
pub const MAX_CHANNELS: usize = 32;

/// The kind of trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    /// An edge on one channel.
    Edge,
    /// A pattern of up to 16 adjacent channels.
    Pattern,
    /// A pattern of up to 5 adjacent channels, evaluated in the fast path.
    FastPattern,
    /// Blast: capture without waiting for a trigger.
    Blast,
}

impl TriggerType {
    fn from_byte(byte: u8) -> Option<Self> {
        Some(match byte {
            0 => Self::Edge,
            1 => Self::Pattern,
            2 => Self::FastPattern,
            3 => Self::Blast,
            _ => return None,
        })
    }

    fn to_byte(self) -> u8 {
        match self {
            Self::Edge => 0,
            Self::Pattern => 1,
            Self::FastPattern => 2,
            Self::Blast => 3,
        }
    }
}

/// The role of a board in a chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A single board.
    Single,
    /// The first board of a chain.
    Master,
    /// A later board of a chain.
    Slave,
}

/// The encoding asked for the samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Every sample, as bytes.
    Raw,
    /// Only the changes.
    Compact,
}

/// The memory a capture goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Memory {
    /// The board chooses.
    Auto,
    /// The internal memory.
    Internal,
    /// The external memory.
    External,
}

/// The field of a request that is wrong; [`RequestField::name`] is what the board names after
/// `CAPTURE_ERROR`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestField {
    /// The trigger type.
    TriggerType,
    /// The trigger channel.
    TriggerChannel,
    /// The edge direction or the pattern length.
    TriggerLength,
    /// The pattern bits.
    TriggerValue,
    /// The number and the list of channels.
    Channels,
    /// The capture mode.
    Mode,
    /// The sample rate.
    Frequency,
    /// The samples before the trigger.
    SamplesBefore,
    /// The samples after the trigger.
    SamplesAfter,
    /// The bursts.
    Bursts,
    /// The flag for the burst times.
    BurstTimes,
    /// The role in a chain.
    Role,
    /// The encoding.
    Encoding,
    /// The memory.
    Memory,
}

impl RequestField {
    /// The name of the field, as in the protocol contract.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::TriggerType => "trigger_type",
            Self::TriggerChannel => "trigger_channel",
            Self::TriggerLength => "trigger_length",
            Self::TriggerValue => "trigger_value",
            Self::Channels => "channels",
            Self::Mode => "mode",
            Self::Frequency => "frequency",
            Self::SamplesBefore => "samples_before",
            Self::SamplesAfter => "samples_after",
            Self::Bursts => "bursts",
            Self::BurstTimes => "burst_times",
            Self::Role => "role",
            Self::Encoding => "encoding",
            Self::Memory => "memory",
        }
    }
}

/// Why a request could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestError {
    /// The payload is not [`REQUEST2_LEN`] bytes.
    WrongLength,
    /// A field has a value no request can have.
    Field(RequestField),
}

impl core::fmt::Display for RequestError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::WrongLength => f.write_str("the request is not 58 bytes long"),
            Self::Field(field) => write!(f, "the field {} is wrong", field.name()),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RequestError {}

/// Capture request 2, as the 58 bytes of the contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureRequest2 {
    /// The trigger type.
    pub trigger_type: TriggerType,
    /// The trigger channel (the base channel of a pattern), 0 to 31.
    pub trigger_channel: u8,
    /// For an edge, 1 for falling and 0 for rising; for a pattern, its length.
    pub trigger_length: u8,
    /// The pattern bits, bit 0 for the base channel.
    pub trigger_value: u32,
    /// How many of `channels` are used, 1 to 32.
    pub channel_count: u8,
    /// The channel numbers captured, in output order; the first `channel_count` are used.
    pub channels: [u8; MAX_CHANNELS],
    /// The capture mode, 0 to 3.
    pub mode: u8,
    /// The sample rate in hertz.
    pub frequency: u32,
    /// The samples before the trigger.
    pub samples_before: u32,
    /// The samples after the trigger.
    pub samples_after: u32,
    /// The bursts after the trigger, 0 for none.
    pub bursts: u8,
    /// Whether to return the time of each burst.
    pub burst_times: bool,
    /// The role in a chain.
    pub role: Role,
    /// The encoding asked for the samples.
    pub encoding: Encoding,
    /// The memory.
    pub memory: Memory,
}

impl CaptureRequest2 {
    /// The channels in use, in output order.
    #[must_use]
    pub fn used_channels(&self) -> &[u8] {
        self.channels
            .get(..usize::from(self.channel_count))
            .unwrap_or(&[])
    }

    /// The mode the highest channel in use needs: 0 up to channel 7, 1 up to 15, 2 up to 23, 3 above.
    #[must_use]
    pub fn mode_needed(&self) -> u8 {
        let highest = self.used_channels().iter().copied().max().unwrap_or(0);
        highest / 8
    }

    /// The bytes of one raw sample of this request.
    #[must_use]
    pub fn bytes_per_sample(&self) -> usize {
        bytes_per_sample(self.channel_count)
    }

    /// The 58 bytes of the payload.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; REQUEST2_LEN] {
        let mut out = [0_u8; REQUEST2_LEN];
        let mut put = |at: usize, bytes: &[u8]| {
            if let Some(slot) = out.get_mut(at..at.saturating_add(bytes.len())) {
                slot.copy_from_slice(bytes);
            }
        };
        put(
            0,
            &[
                self.trigger_type.to_byte(),
                self.trigger_channel,
                self.trigger_length,
            ],
        );
        put(3, &self.trigger_value.to_le_bytes());
        put(7, &[self.channel_count]);
        put(8, &self.channels);
        put(40, &[self.mode]);
        put(41, &self.frequency.to_le_bytes());
        put(45, &self.samples_before.to_le_bytes());
        put(49, &self.samples_after.to_le_bytes());
        put(53, &[self.bursts, u8::from(self.burst_times)]);
        put(
            55,
            &[
                match self.role {
                    Role::Single => 0,
                    Role::Master => 1,
                    Role::Slave => 2,
                },
                match self.encoding {
                    Encoding::Raw => 0,
                    Encoding::Compact => 1,
                },
                match self.memory {
                    Memory::Auto => 0,
                    Memory::Internal => 1,
                    Memory::External => 2,
                },
            ],
        );
        out
    }

    /// Reads a payload. Only what no request can be is refused here (a length that is not 58, an unknown
    /// enumeration value, a channel number of 32 or more, a duplicate channel, a count outside 1 to 32, a mode
    /// above 3); the limits of a device are checked by [`DeviceInfo::check_request`].
    ///
    /// # Errors
    ///
    /// [`RequestError::WrongLength`] or [`RequestError::Field`] naming the first field that is wrong.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RequestError> {
        if bytes.len() != REQUEST2_LEN {
            return Err(RequestError::WrongLength);
        }
        let at = |i: usize| bytes.get(i).copied().unwrap_or(0);
        let word = |i: usize| {
            u32::from_le_bytes([
                at(i),
                at(i.saturating_add(1)),
                at(i.saturating_add(2)),
                at(i.saturating_add(3)),
            ])
        };
        let trigger_type =
            TriggerType::from_byte(at(0)).ok_or(RequestError::Field(RequestField::TriggerType))?;
        if usize::from(at(1)) >= MAX_CHANNELS {
            return Err(RequestError::Field(RequestField::TriggerChannel));
        }
        let channel_count = at(7);
        if channel_count == 0 || usize::from(channel_count) > MAX_CHANNELS {
            return Err(RequestError::Field(RequestField::Channels));
        }
        let mut channels = [0_u8; MAX_CHANNELS];
        if let Some(list) = bytes.get(8..40) {
            channels.copy_from_slice(list);
        }
        let used = channels.get(..usize::from(channel_count)).unwrap_or(&[]);
        let mut seen = 0_u32;
        for &channel in used {
            if usize::from(channel) >= MAX_CHANNELS || seen >> channel & 1 == 1 {
                return Err(RequestError::Field(RequestField::Channels));
            }
            seen |= 1 << channel;
        }
        if at(40) > 3 {
            return Err(RequestError::Field(RequestField::Mode));
        }
        let role = match at(55) {
            0 => Role::Single,
            1 => Role::Master,
            2 => Role::Slave,
            _ => return Err(RequestError::Field(RequestField::Role)),
        };
        let encoding = match at(56) {
            0 => Encoding::Raw,
            1 => Encoding::Compact,
            _ => return Err(RequestError::Field(RequestField::Encoding)),
        };
        let memory = match at(57) {
            0 => Memory::Auto,
            1 => Memory::Internal,
            2 => Memory::External,
            _ => return Err(RequestError::Field(RequestField::Memory)),
        };
        if at(54) > 1 {
            return Err(RequestError::Field(RequestField::BurstTimes));
        }
        Ok(Self {
            trigger_type,
            trigger_channel: at(1),
            trigger_length: at(2),
            trigger_value: word(3),
            channel_count,
            channels,
            mode: at(40),
            frequency: word(41),
            samples_before: word(45),
            samples_after: word(49),
            bursts: at(53),
            burst_times: at(54) == 1,
            role,
            encoding,
            memory,
        })
    }
}

impl DeviceInfo {
    /// Checks `request` against the limits of this device, as the firmware does before it starts a capture.
    ///
    /// # Errors
    ///
    /// The first field that is outside what the device reported.
    pub fn check_request(&self, request: &CaptureRequest2) -> Result<(), RequestField> {
        let triggers = &self.triggers;
        match request.trigger_type {
            TriggerType::Edge => {
                if !triggers.edge {
                    return Err(RequestField::TriggerType);
                }
                if request.trigger_length > 1 {
                    return Err(RequestField::TriggerLength);
                }
                if request.trigger_value != 0 {
                    return Err(RequestField::TriggerValue);
                }
            }
            TriggerType::Pattern | TriggerType::FastPattern => {
                let most = if request.trigger_type == TriggerType::Pattern {
                    triggers.pattern
                } else {
                    triggers.fast
                };
                if most == 0 {
                    return Err(RequestField::TriggerType);
                }
                if request.trigger_length == 0 || request.trigger_length > most {
                    return Err(RequestField::TriggerLength);
                }
                if u16::from(request.trigger_channel)
                    .saturating_add(u16::from(request.trigger_length))
                    > 32
                {
                    return Err(RequestField::TriggerChannel);
                }
                if request.trigger_value >> request.trigger_length != 0 {
                    return Err(RequestField::TriggerValue);
                }
            }
            TriggerType::Blast => {
                if !triggers.blast {
                    return Err(RequestField::TriggerType);
                }
            }
        }
        // The mode must reach the highest channel, and mode 3 needs the 32-channel feature.
        if request.mode < request.mode_needed() {
            return Err(RequestField::Mode);
        }
        if request.mode == 3 && !self.features.mode32 {
            return Err(RequestField::Mode);
        }
        let highest_rate = if request.trigger_type == TriggerType::Blast {
            self.blast_frequency
        } else {
            self.frequency
        };
        if request.frequency == 0 || request.frequency > highest_rate {
            return Err(RequestField::Frequency);
        }
        let mode = usize::from(request.mode);
        if request.samples_before > self.max_before.get(mode).copied().unwrap_or(0) {
            return Err(RequestField::SamplesBefore);
        }
        if request.samples_after == 0
            || request.samples_after > self.max_after.get(mode).copied().unwrap_or(0)
        {
            return Err(RequestField::SamplesAfter);
        }
        if request.bursts > triggers.burst {
            return Err(RequestField::Bursts);
        }
        if request.burst_times && request.bursts == 0 {
            return Err(RequestField::BurstTimes);
        }
        if request.encoding == Encoding::Compact && !self.features.compact {
            return Err(RequestField::Encoding);
        }
        if request.memory == Memory::External && self.psram_bytes == 0 {
            return Err(RequestField::Memory);
        }
        Ok(())
    }
}
