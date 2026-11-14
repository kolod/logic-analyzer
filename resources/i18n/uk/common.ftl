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
