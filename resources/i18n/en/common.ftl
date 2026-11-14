### status: draft
### Shared messages: counted messages, units, formats and language names.
### Messages marked `# @reserved` are defined for later features and are not used yet.

### Counts
### A counted message selects a form with the plural rules of its language and writes the number with
### FORMAT_NUMBER so the separators match the language.

# @reserved
# A number of samples ("12 samples", "1 sample").
count-samples =
    { $count ->
        [one] { FORMAT_NUMBER($count) } sample
       *[other] { FORMAT_NUMBER($count) } samples
    }

# @reserved
# A number of channels.
count-channels =
    { $count ->
        [one] { FORMAT_NUMBER($count) } channel
       *[other] { FORMAT_NUMBER($count) } channels
    }

# @reserved
# A number of devices.
count-devices =
    { $count ->
        [one] { FORMAT_NUMBER($count) } device
       *[other] { FORMAT_NUMBER($count) } devices
    }

# @reserved
# A number of signal transitions (changes of level).
count-transitions =
    { $count ->
        [one] { FORMAT_NUMBER($count) } transition
       *[other] { FORMAT_NUMBER($count) } transitions
    }

# @reserved
# A number of gaps (stretches of lost samples).
count-gaps =
    { $count ->
        [one] { FORMAT_NUMBER($count) } gap
       *[other] { FORMAT_NUMBER($count) } gaps
    }
