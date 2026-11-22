# Contract: the `LAYT` section, version 2

Version 1 is `specs/014-gui-improvements/contracts/layt-section.md`: a version byte, a count, and
(channel, height) pairs. Version 2 adds the heights of the lanes of decoder instances.

```text
2                      version
n                      count of channels with a height (0 to 32), as in version 1
n × (channel, height)  ascending channels, below the channel count, heights 24 to 240
m                      count of decoder lanes with a height (1 to 64)
m × (instance, height) strictly ascending instances, heights 24 to 240
```

Every number is a canonical varint (no more bytes than needed). Nothing may follow the last height.

- The writer writes version 2 only when a decoder lane has a height of its own; with none, the payload is
  version 1, byte for byte what it was, and with nothing at all the section is not written.
- The reader accepts versions 1 and 2; a higher version is kept as it is and not interpreted (a notice is
  shown); anything else that does not follow this contract is `host.layout_invalid`, the section is dropped
  and the capture opens with automatic heights.
- An instance that is not among the instances restored from the `DECO` section is ignored.
- The place of a decoder lane among the other lanes is not saved, as the order of the channels is not.
