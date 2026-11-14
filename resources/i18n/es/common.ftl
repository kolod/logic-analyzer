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
