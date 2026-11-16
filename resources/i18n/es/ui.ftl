### status: draft
### Borrador traducido automáticamente; debe revisarlo una persona con el español como lengua materna.

### Aplicación

app-title = Logic Analyzer

### Estado y resultados de la captura

status-idle = En espera
status-capturing = Capturando
status-saving = Guardando
status-loading = Cargando
status-memory = { $used } / { $limit }
end-reason-completed = Captura completada
end-reason-stopped = Detenida por usted
end-reason-memory-limit = Límite de memoria alcanzado
end-reason-disconnected = Dispositivo desconectado
end-reason-device-error = Error del dispositivo
end-reason-samples-lost = Completada con muestras perdidas
end-reason-loaded = Cargada desde un archivo
notice-device-removed = El dispositivo seleccionado «{ $text }» ya no está disponible; se seleccionó el dispositivo predeterminado.

### Menús

menu-file = Archivo
menu-file-new = Nueva captura
menu-file-quit = Salir
menu-capture = Captura
menu-capture-start = Iniciar
menu-capture-stop = Detener

### Barra de herramientas, vista y diálogos

toolbar-device = Dispositivo
toolbar-rate = Frecuencia de muestreo (Hz)
toolbar-seed = Semilla
toolbar-seed-random = aleatoria
toolbar-profile = Señal
profile-sparse = Escasa
profile-normal = Normal
profile-dense = Densa
toolbar-realtime = Tiempo real
toolbar-start = Iniciar
toolbar-stop = Detener
status-simulated = Simulado
status-incomplete = Incompleta
status-unsaved = Cambios sin guardar
view-no-capture = Aún no hay captura. Elija un dispositivo y pulse Iniciar.
view-empty = La captura no tiene muestras.
view-cannot-draw = No se pueden dibujar las señales.
channel-default = Canal { $number }
dialog-unsaved-title = Captura sin guardar
dialog-unsaved-body = La captura no se ha guardado. ¿Qué desea hacer con ella?
dialog-save = Guardar
dialog-discard = Descartar
dialog-cancel = Cancelar

### Vista y canales

menu-view = Vista
menu-view-zoom-in = Acercar
menu-view-zoom-out = Alejar
menu-view-fit = Ver todo
menu-view-start = Ir al inicio
menu-view-end = Ir al final
menu-view-pan-left = Desplazar a la izquierda
menu-view-pan-right = Desplazar a la derecha
menu-view-page-left = Página anterior
menu-view-page-right = Página siguiente
menu-capture-follow = Seguir los datos más recientes
channel-reset = Restablecer canales

### Mensajes

notice-dismiss = Cerrar
notice-dismiss-all = Cerrar todos
notice-log = Mostrar la carpeta del registro
notice-list = Mensajes
menu-help = Ayuda
menu-help-log = Mostrar la carpeta del registro

### Marcadores y mediciones

menu-markers = Marcadores
menu-markers-place = Colocar marcador
menu-markers-previous = Marcador anterior
menu-markers-next = Marcador siguiente
menu-markers-remove = Quitar marcador
menu-markers-remove-all = Quitar todos los marcadores
menu-markers-snap = Ajustar a las transiciones
panel-markers = Marcadores
panel-capture = Captura
panel-view-capture = Ajustes de la captura
panel-view-decoders = Decodificadores
panel-view-markers = Marcadores y medidas
capture-length-seconds = Duración (segundos)
capture-length-samples = Muestras: { $count }
capture-length-invalid = Escriba la duración en segundos, por ejemplo 2,5.
capture-length-decimals = Use como máximo nueve cifras decimales.
capture-length-small = Es menos de una muestra con esta frecuencia de muestreo.
capture-length-large = Es más larga de lo que puede ser una captura.
capture-length-no-rate = Primero fije la frecuencia de muestreo.
capture-channels = Canales a grabar
capture-no-channel = No hay ningún canal elegido: elija al menos uno para empezar.
marker-default = Marcador { $number }
marker-go = Ir al marcador
marker-remove = Quitar
marker-measure-first = Primero de la medición
marker-measure-second = Segundo de la medición
measure-distance = Distancia
measure-frequency = Frecuencia
measure-pick = Elija dos marcadores para medir el tiempo entre ellos.

### Archivos de sesión

menu-file-open = Abrir…
menu-file-save = Guardar
menu-file-save-as = Guardar como…
menu-file-recent = Archivos recientes
dialog-saving = Guardando la sesión…
dialog-opening = Abriendo la sesión…
dialog-open-title = Abrir una sesión
dialog-save-title = Guardar la sesión
dialog-filter-session = Sesiones del analizador lógico
title-untitled = Captura sin título
notice-view-newer = Los ajustes de vista de este archivo proceden de una versión más reciente y no se aplicaron. Permanecen tal cual en el archivo.

### Ajustes y acerca de

menu-file-settings = Ajustes…
menu-view-scale-up = Interfaz más grande
menu-view-scale-down = Interfaz más pequeña
menu-help-about = Acerca de
settings-title = Ajustes
settings-language = Idioma
settings-theme = Tema
theme-system = Seguir el sistema
theme-light = Claro
theme-dark = Oscuro
theme-high-contrast = Alto contraste
settings-scale = Escala de la interfaz
settings-memory = Límite de memoria de una captura
settings-memory-unit = MiB
settings-memory-note = Se aplica a la próxima captura.
settings-memory-invalid = Introduzca un número entero de MiB entre { $min } y { $max }.
dialog-close = Cerrar
about-title = Acerca de
about-body = Logic Analyzer { $version }. Registra señales digitales y las muestra en una línea de tiempo; un dispositivo simulado permite probarlo sin hardware.
about-license = Software libre bajo la GNU General Public License, versión 3.

### Decoders: the sample decoder
decoder-sample-clocked-bits-name = Bits con reloj
decoder-sample-clocked-bits-description = Lee bits de una línea de datos en los flancos de una línea de reloj y los agrupa en bytes. Un decodificador de ejemplo que muestra lo que pueden hacer los decodificadores.
decoder-sample-clocked-bits-signal-clock = Reloj
decoder-sample-clocked-bits-signal-data = Datos
decoder-sample-clocked-bits-option-clock_edge = Flanco del reloj
decoder-sample-clocked-bits-choice-clock_edge-rising = Ascendente
decoder-sample-clocked-bits-choice-clock_edge-falling = Descendente
decoder-sample-clocked-bits-option-bit_order = Orden de los bits
decoder-sample-clocked-bits-choice-bit_order-msb = Primero el bit más significativo
decoder-sample-clocked-bits-choice-bit_order-lsb = Primero el bit menos significativo
decoder-sample-clocked-bits-option-bits = Bits por byte
decoder-sample-clocked-bits-option-idle = Límite de inactividad (muestras)
ann-sample-clocked-bits-byte = Byte { $value }
ann-sample-clocked-bits-bit = Bit { $value }
ann-sample-clocked-bits-incomplete = Byte incompleto: { $value } bits
ann-sample-clocked-bits-gap = Aquí se perdieron muestras

### Decoders: I2C
decoder-i2c-name = I2C
decoder-i2c-description = Lee el bus I2C (dos hilos, reloj y datos): la dirección, los bytes y las confirmaciones de cada transacción.
decoder-i2c-signal-scl = Reloj (SCL)
decoder-i2c-signal-sda = Datos (SDA)
decoder-i2c-option-address = Notación de la dirección
decoder-i2c-choice-address-addr7 = Dirección de 7 bits (sin el bit de dirección)
decoder-i2c-choice-address-addr8 = Valor de 8 bits (con el bit de dirección)
decoder-i2c-option-radix = Base numérica
decoder-i2c-choice-radix-hex = Hexadecimal
decoder-i2c-choice-radix-dec = Decimal
decoder-i2c-choice-radix-bin = Binario
ann-i2c-start = Inicio
ann-i2c-repeated-start = Inicio repetido
ann-i2c-stop = Parada
ann-i2c-transaction-write = Escritura en { $value }
ann-i2c-transaction-read = Lectura de { $value }
ann-i2c-address-write = Dirección { $value } (escritura)
ann-i2c-address-read = Dirección { $value } (lectura)
ann-i2c-data = Datos { $value }
ann-i2c-ack = ACK
ann-i2c-nak = NAK
ann-i2c-incomplete = Transacción incompleta
ann-i2c-error-byte-cut = Byte interrumpido tras { $value } bits
ann-i2c-error-address-nak = Dirección { $value } sin confirmar
ann-i2c-error-data-nak = Datos { $value } sin confirmar
ann-i2c-error-empty = Transacción vacía: sin dirección
ann-i2c-error-gap = Aquí se perdieron muestras

### Decoders: menu, panel and the selected annotation
menu-decoders = Decodificadores
menu-decoders-add = Añadir decodificador
menu-decoders-previous = Anotación anterior
menu-decoders-next = Anotación siguiente
menu-decoders-goto = Ir a la anotación seleccionada
panel-decoders = Decodificadores
decoders-add = Añadir decodificador
decoders-empty = Aún no hay decodificadores. Añada uno para ver qué dicen las señales.
decoders-need-capture = Primero capture o abra señales.
decoder-state-waiting = En espera
decoder-state-running = En ejecución
decoder-state-finished = Terminado
decoder-state-cancelled = Cancelado
decoder-state-failed = Falló
decoder-needs-channels = Elija los canales para iniciar la decodificación.
decoder-channel-none = (ninguno)
decoder-lane-show = Mostrar el carril
decoder-remove = Quitar decodificador
decoder-annotations = Anotaciones: { $number }
decoder-incomplete = Resultados incompletos
decoder-selected = Anotación seleccionada
decoder-detail-length = Duración: { $value }
decoder-detail-error = Indica un problema en la señal
decoder-detail-position = Posición: de { $first } a { $second }

### Decoders: the simulation of signals that are valid for a decoder
decoder-generator-errors = Introducir errores a propósito en las señales simuladas
decoder-generator-rate = Aproximadamente uno de cada tantos bytes se corta

### Decoders: running, cancelling and the memory limit
decoder-run = Ejecutar de nuevo
decoder-cancel = Cancelar
settings-decoder-memory = Límite de memoria de los resultados de los decodificadores
settings-decoder-memory-note = Compartido por todos los decodificadores. Se aplica a la próxima pasada.

### Decoders: stacking
decoder-source = Lee de
decoder-source-capture = La captura
dialog-remove-decoder-title = Quitar decodificador
dialog-remove-decoder-body = Otros decodificadores leen de este. Si lo quita, también se quitan ellos. La captura no se ve afectada.
dialog-remove-decoder-confirm = Quitar todos

### Decoders: saving and opening
notice-decoders-newer = Este archivo tiene decodificadores de una versión más reciente de la aplicación. Se conservan tal cual y no se muestran; la captura se abre con normalidad.
notice-layout-newer = Este archivo contiene alturas de señales guardadas por una versión más reciente de la aplicación. Se conservan tal cual y no se muestran; la captura se abre con alturas automáticas.
settings-save-decoder-results = Guardar los resultados decodificados con la sesión
settings-save-decoder-results-note = El archivo es más grande. Sin esto, los decodificadores se ejecutan de nuevo al abrir el archivo.
