### status: draft
### Borrador traducido automáticamente; debe revisarlo una persona con el español como lengua materna.

### Recuentos

# @reserved
count-samples =
    { $count ->
        [one] { FORMAT_NUMBER($count) } muestra
        [many] { FORMAT_NUMBER($count) } de muestras
       *[other] { FORMAT_NUMBER($count) } muestras
    }

# @reserved
count-channels =
    { $count ->
        [one] { FORMAT_NUMBER($count) } canal
        [many] { FORMAT_NUMBER($count) } de canales
       *[other] { FORMAT_NUMBER($count) } canales
    }

# @reserved
count-devices =
    { $count ->
        [one] { FORMAT_NUMBER($count) } dispositivo
        [many] { FORMAT_NUMBER($count) } de dispositivos
       *[other] { FORMAT_NUMBER($count) } dispositivos
    }

# @reserved
count-transitions =
    { $count ->
        [one] { FORMAT_NUMBER($count) } transición
        [many] { FORMAT_NUMBER($count) } de transiciones
       *[other] { FORMAT_NUMBER($count) } transiciones
    }

# @reserved
count-gaps =
    { $count ->
        [one] { FORMAT_NUMBER($count) } hueco
        [many] { FORMAT_NUMBER($count) } de huecos
       *[other] { FORMAT_NUMBER($count) } huecos
    }

### Unidades

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

### Nombres de los idiomas

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
