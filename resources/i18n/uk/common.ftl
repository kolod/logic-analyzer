### status: draft
### Чернетка, перекладена машинним способом; потребує перевірки носієм української мови.

### Кількості

# @reserved
count-samples =
    { $count ->
        [one] { FORMAT_NUMBER($count) } відлік
        [few] { FORMAT_NUMBER($count) } відліки
        [many] { FORMAT_NUMBER($count) } відліків
       *[other] { FORMAT_NUMBER($count) } відліку
    }

# @reserved
count-channels =
    { $count ->
        [one] { FORMAT_NUMBER($count) } канал
        [few] { FORMAT_NUMBER($count) } канали
        [many] { FORMAT_NUMBER($count) } каналів
       *[other] { FORMAT_NUMBER($count) } каналу
    }

# @reserved
count-devices =
    { $count ->
        [one] { FORMAT_NUMBER($count) } пристрій
        [few] { FORMAT_NUMBER($count) } пристрої
        [many] { FORMAT_NUMBER($count) } пристроїв
       *[other] { FORMAT_NUMBER($count) } пристрою
    }

# @reserved
count-transitions =
    { $count ->
        [one] { FORMAT_NUMBER($count) } перехід
        [few] { FORMAT_NUMBER($count) } переходи
        [many] { FORMAT_NUMBER($count) } переходів
       *[other] { FORMAT_NUMBER($count) } переходу
    }

# @reserved
count-gaps =
    { $count ->
        [one] { FORMAT_NUMBER($count) } пропуск
        [few] { FORMAT_NUMBER($count) } пропуски
        [many] { FORMAT_NUMBER($count) } пропусків
       *[other] { FORMAT_NUMBER($count) } пропуску
    }

### Одиниці

# @reserved
unit-hz = { $value }{ "\u00A0" }Гц

# @reserved
unit-khz = { $value }{ "\u00A0" }кГц

# @reserved
unit-mhz = { $value }{ "\u00A0" }МГц

# @reserved
unit-ghz = { $value }{ "\u00A0" }ГГц

# @reserved
unit-b = { $value }{ "\u00A0" }Б

# @reserved
unit-kib = { $value }{ "\u00A0" }КіБ

# @reserved
unit-mib = { $value }{ "\u00A0" }МіБ

# @reserved
unit-gib = { $value }{ "\u00A0" }ГіБ

# @reserved
unit-tib = { $value }{ "\u00A0" }ТіБ

# @reserved
unit-ns = { $value }{ "\u00A0" }нс

# @reserved
unit-us = { $value }{ "\u00A0" }мкс

# @reserved
unit-ms = { $value }{ "\u00A0" }мс

# @reserved
unit-s = { $value }{ "\u00A0" }с

# @reserved
unit-min = { $value }{ "\u00A0" }хв

# @reserved
unit-h = { $value }{ "\u00A0" }год

# @reserved
format-duration-parts = { $first } { $second }

### Назви мов

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
