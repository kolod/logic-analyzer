### status: draft
### Version préliminaire traduite automatiquement ; à relire par une personne de langue maternelle française.

### Décomptes

# @reserved
count-samples =
    { $count ->
        [one] { FORMAT_NUMBER($count) } échantillon
        [many] { FORMAT_NUMBER($count) } d'échantillons
       *[other] { FORMAT_NUMBER($count) } échantillons
    }

# @reserved
count-channels =
    { $count ->
        [one] { FORMAT_NUMBER($count) } voie
        [many] { FORMAT_NUMBER($count) } de voies
       *[other] { FORMAT_NUMBER($count) } voies
    }

# @reserved
count-devices =
    { $count ->
        [one] { FORMAT_NUMBER($count) } appareil
        [many] { FORMAT_NUMBER($count) } d'appareils
       *[other] { FORMAT_NUMBER($count) } appareils
    }

# @reserved
count-transitions =
    { $count ->
        [one] { FORMAT_NUMBER($count) } transition
        [many] { FORMAT_NUMBER($count) } de transitions
       *[other] { FORMAT_NUMBER($count) } transitions
    }

# @reserved
count-gaps =
    { $count ->
        [one] { FORMAT_NUMBER($count) } lacune
        [many] { FORMAT_NUMBER($count) } de lacunes
       *[other] { FORMAT_NUMBER($count) } lacunes
    }

### Unités

# @reserved
unit-hz = { $value }{ "\u202F" }Hz

# @reserved
unit-khz = { $value }{ "\u202F" }kHz

# @reserved
unit-mhz = { $value }{ "\u202F" }MHz

# @reserved
unit-ghz = { $value }{ "\u202F" }GHz

# @reserved
unit-b = { $value }{ "\u202F" }o

# @reserved
unit-kib = { $value }{ "\u202F" }Kio

# @reserved
unit-mib = { $value }{ "\u202F" }Mio

# @reserved
unit-gib = { $value }{ "\u202F" }Gio

# @reserved
unit-tib = { $value }{ "\u202F" }Tio

# @reserved
unit-ns = { $value }{ "\u202F" }ns

# @reserved
unit-us = { $value }{ "\u202F" }µs

# @reserved
unit-ms = { $value }{ "\u202F" }ms

# @reserved
unit-s = { $value }{ "\u202F" }s

# @reserved
unit-min = { $value }{ "\u202F" }min

# @reserved
unit-h = { $value }{ "\u202F" }h

# @reserved
format-duration-parts = { $first } { $second }

### Noms des langues

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
