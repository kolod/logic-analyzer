# Contract: what the device abstraction gains

The changes to `la-devices` (and the types it uses from `la-storage`). A device that does not use the new things
declares none of them and behaves as before; the tests of every other driver pass unchanged.

## Channels (stage 1)

- `la_storage::MAX_CHANNELS`: 32 → **160**.
- `la_storage::CaptureConfig::words_per_sample`: 1 to 5, `ceil(channels / 32)`, derived; chunks given to
  `Capture::append(start, samples)` hold `n × words_per_sample` words in sample order; word `k` of a sample is
  channels `32k..32k+31` (bit `i` of word `k` is channel `32k + i`). For one word per sample nothing differs.
- `la_devices::ChannelSet`: a set of channel numbers below `MAX_CHANNELS`, used only through its methods:
  `empty`, `all(n)` (the first `n`), `contains`, `insert`, `remove`, `count`, `iter` (ascending), `and`,
  `highest`, and `from_u32` and `to_u32` (the first word, for code that still handles up to 32). It is
  `Copy` and its size follows `MAX_CHANNELS` (a constant; the channel number stays a `u8`, so the bound is at
  most 256). Used for
  `DeviceConfig::enabled_channels`, `ResolvedConfig::enabled_channels`, the description of a capture and the
  presentation's visible channels.
- `Driver::read(&mut self, max_samples, out: &mut Vec<u32>)`: `out` receives `n × words_per_sample` words;
  `max_samples` counts samples, not words. `Capabilities::channels` is 1 to 160.
- The virtual device generates up to 160 channels (`VirtualOptions` is unchanged otherwise).

## Triggers, samples before and after, bursts (stage 2)

```text
TriggerKind  += Pattern, FastPattern          // existing: Level, RisingEdge, FallingEdge, AnyEdge
Pattern      { base_channel, length, value }  // levels of adjacent channels, bit i = channel base + i
Capabilities += trigger_limits: TriggerLimits // widest Pattern and FastPattern, chained, allowed_in_chain
             += pre_post: Vec<ModeLimit>      // by capture mode: channels up to, max before, max after
             += buffer_depths: Vec<DepthBand> // by rate range: from rate, to rate, depth in samples
             += bursts: Option<u32>           // the most bursts, None when unsupported
             += max_rates: Vec<(u8, u64)>     // highest rate by channel count (up to n channels)
             += actions: Vec<DeviceAction>    // Restart, RestartIntoBootloader (stage 3); DeviceProvider::perform
DeviceConfig += pattern: Option<Pattern>      // for the Pattern and FastPattern triggers only
             += pre_trigger_samples: u64      // kept in the capture, part of capture_length; 0 when unused
             += bursts: u32                   // 0 for one capture
```

`validate` checks each new field against the capabilities and fails with a typed `device.*` error that names the
value and the limit: `device.unsupported_trigger` (a pattern wider than allowed, a trigger in a chain),
`device.invalid_capture_length` (more samples before or after than the mode allows, more than the depth at the
rate), `device.unsupported_sample_rate` (a rate above the highest for that channel count), and the new
`device.unsupported_bursts`. `CaptureDescription::samples_before_trigger` keeps its meaning (samples skipped
waiting); a new field `pre_trigger_samples` records the samples kept before the trigger.

## Options and the chain (stage 3, 4)

`DeviceOptions::Rp2350b(Rp2350bOptions)`: the memory to use (auto, internal, external), the compact transfer
(auto, off), the shift in samples of each board of a chain. A chain is a device whose `DeviceId` is
`rp2350b:chain-<name>` and whose capabilities are those of its boards combined (channels the sum, rates and
depths the least of the boards, triggers the pattern ones of the master).

## Compatibility

`ResolvedConfig::enabled_channels()` still returns `u32` for devices of 32 channels or fewer through
`to_u32()` until every caller is moved; each caller that handles more than 32 channels uses `ChannelSet`.
The host API gains a minor version for the types it exposes (`State::config`, `ChannelStyle` unchanged).
