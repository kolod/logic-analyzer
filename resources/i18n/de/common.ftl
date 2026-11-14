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
