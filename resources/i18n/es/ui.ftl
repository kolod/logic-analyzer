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
toolbar-length = Duración (muestras)
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
panel-channels = Canales
channel-show = Mostrar
channel-move-up = Subir
channel-move-down = Bajar
channel-colour = Color
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

# Tooltip of the flag of a channel that chooses whether the next capture records it.
channel-record = Grabar en la próxima captura
# Short caption above the column of the flags that choose what the next capture records.
channels-header-record = Grab.
# Short caption above the column of the flags that show or hide a channel.
channels-header-show = Ver
