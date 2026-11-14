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

### Units
### A value followed by its unit. $value is the number already written for the language; the space between the value and the unit belongs to the translation (a no-break space, a narrow no-break space in French, none in Japanese).

# @reserved
unit-hz = { $value }{ "\u00A0" }Hz

# @reserved
unit-khz = { $value }{ "\u00A0" }kHz

# @reserved
unit-mhz = { $value }{ "\u00A0" }MHz

# @reserved
unit-ghz = { $value }{ "\u00A0" }GHz

# @reserved
unit-b = { $value }{ "\u00A0" }B

# @reserved
unit-kib = { $value }{ "\u00A0" }KiB

# @reserved
unit-mib = { $value }{ "\u00A0" }MiB

# @reserved
unit-gib = { $value }{ "\u00A0" }GiB

# @reserved
unit-tib = { $value }{ "\u00A0" }TiB

# @reserved
unit-ns = { $value }{ "\u00A0" }ns

# @reserved
unit-us = { $value }{ "\u00A0" }µs

# @reserved
unit-ms = { $value }{ "\u00A0" }ms

# @reserved
unit-s = { $value }{ "\u00A0" }s

# @reserved
unit-min = { $value }{ "\u00A0" }min

# @reserved
unit-h = { $value }{ "\u00A0" }h

# @reserved
# Joins the two parts of a long duration, for example "2 min 5 s". $first and $second already include their units.
format-duration-parts = { $first } { $second }

### Language names (the same in every language, so the language chooser reads the same everywhere)

# @reserved
language-name-en = English

# @reserved
language-name-de = Deutsch

# @reserved
language-name-fr = Français

# @reserved
language-name-es = Español

# @reserved
language-name-ja = 日本語

# @reserved
language-name-uk = Українська
