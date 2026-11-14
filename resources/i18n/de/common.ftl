### status: draft
### Maschinell übersetzter Entwurf; muss von einer Person mit Deutsch als Muttersprache geprüft werden.

### Zählungen

# @reserved
count-samples =
    { $count ->
        [one] { FORMAT_NUMBER($count) } Sample
       *[other] { FORMAT_NUMBER($count) } Samples
    }

# @reserved
count-channels =
    { $count ->
        [one] { FORMAT_NUMBER($count) } Kanal
       *[other] { FORMAT_NUMBER($count) } Kanäle
    }

# @reserved
count-devices =
    { $count ->
        [one] { FORMAT_NUMBER($count) } Gerät
       *[other] { FORMAT_NUMBER($count) } Geräte
    }

# @reserved
count-transitions =
    { $count ->
        [one] { FORMAT_NUMBER($count) } Flanke
       *[other] { FORMAT_NUMBER($count) } Flanken
    }

# @reserved
count-gaps =
    { $count ->
        [one] { FORMAT_NUMBER($count) } Lücke
       *[other] { FORMAT_NUMBER($count) } Lücken
    }

### Einheiten

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
format-duration-parts = { $first } { $second }

### Sprachnamen

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
