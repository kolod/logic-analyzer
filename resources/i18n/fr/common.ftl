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
