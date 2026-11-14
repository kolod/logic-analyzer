### status: draft
### Borrador traducido automáticamente; debe revisarlo una persona con el español como lengua materna.
### Las claves coinciden con los textos en inglés (`en/errors.ftl`).

error-unexpected = Ocurrió un problema inesperado (código { $code }). Inténtelo de nuevo; si sigue ocurriendo, infórmelo adjuntando el archivo de registro.

### Dispositivos

error-device-busy = Este dispositivo ya está en uso por otra captura. Detenga esa captura o elija otro dispositivo.
error-device-disconnected = El dispositivo se desconectó. Todo lo capturado hasta ahora se conserva. Vuelva a conectar el dispositivo e inicie una nueva captura.
error-device-driver_error = El dispositivo notificó un error y la captura se detuvo. Todo lo capturado hasta ahora se conserva. Desconecte y vuelva a conectar el dispositivo, y pruebe de nuevo.
error-device-driver_panic = El controlador de este dispositivo se detuvo de forma inesperada. Todo lo capturado hasta ahora se conserva. Vuelva a conectar el dispositivo y pruebe de nuevo; si sigue ocurriendo, infórmelo adjuntando el archivo de registro.
error-device-invalid_capabilities = El dispositivo se describió de una forma que el programa no puede usar, por lo que no está disponible. Vuelva a conectarlo y, si el problema continúa, infórmelo adjuntando el archivo de registro.
error-device-invalid_capture_length = La duración de captura solicitada no es posible con este dispositivo. Elija una duración más corta u otro dispositivo.
error-device-invalid_channel = Los canales elegidos no existen en este dispositivo. Elija canales que el dispositivo tenga.
error-device-invalid_state = Esa acción no es posible en este momento. Espere a que termine la operación en curso y pruebe de nuevo.
error-device-memory_budget = Es posible que una captura de esta duración no quepa en la memoria. Elija una captura más corta o permita que el programa use más memoria.
error-device-not_found = El dispositivo no está conectado o ya no está disponible. Compruebe la conexión y actualice la lista de dispositivos.
error-device-transport = Falló la comunicación con el dispositivo. Todo lo capturado hasta ahora se conserva. Compruebe el cable y el puerto USB y pruebe de nuevo.
error-device-trigger_timeout = La condición del disparador no se produjo a tiempo, por lo que la captura no comenzó. Compruebe la configuración del disparador y la señal, o capture sin disparador.
error-device-unresponsive = El dispositivo dejó de responder. Todo lo capturado hasta ahora se conserva. Desconéctelo, vuelva a conectarlo y pruebe de nuevo.
error-device-unsupported_sample_rate = Este dispositivo no puede capturar con la frecuencia de muestreo elegida. Elija una de las frecuencias que ofrece el dispositivo.
error-device-unsupported_trigger = Este dispositivo no admite el disparador elegido. Elija un disparador que ofrezca el dispositivo o capture sin disparador.

### Archivos de sesión

error-session-cancelled = Se canceló la operación. No se cambió nada.
error-session-damaged = El archivo de sesión está dañado y no se puede abrir. Si tiene una copia de seguridad del archivo, ábrala en su lugar.
error-session-inconsistent = El archivo de sesión contiene datos contradictorios y no se puede abrir. Si tiene una copia de seguridad del archivo, ábrala en su lugar.
error-session-internal = El programa encontró un problema inesperado al procesar el archivo de sesión. Inténtelo de nuevo; si sigue ocurriendo, infórmelo adjuntando el archivo de registro.
error-session-invalid_input = No se puede usar el nombre o la ubicación del archivo. Elija otro nombre de archivo u otra carpeta.
error-session-io_read = No se pudo leer el archivo de sesión. Compruebe que el archivo existe y que tiene permiso para abrirlo, y pruebe de nuevo.
error-session-io_write = No se pudo guardar el archivo de sesión. Su archivo existente no se modificó. Compruebe que hay espacio libre en el disco y que puede escribir en la carpeta, o elija otra ubicación.
error-session-memory_budget = Esta sesión es demasiado grande para abrirla con el límite de memoria actual. Permita que el programa use más memoria o abra un archivo más pequeño.
error-session-not_a_session = Este archivo no es un archivo de sesión de este programa. Elija un archivo guardado por él.
error-session-too_large = Este archivo de sesión es más grande de lo que permite el programa. Abra un archivo más pequeño.
error-session-truncated = El archivo de sesión está incompleto, probablemente porque se cortó al copiarlo o guardarlo. Si tiene una copia de seguridad, ábrala en su lugar.
error-session-unknown_critical_section = Este archivo de sesión necesita una versión más reciente del programa. Actualice el programa para abrirlo.
error-session-unsupported_version = Este archivo de sesión fue creado por una versión más reciente del programa. Actualice el programa para abrirlo.
error-session-worker_failed = El guardado o la apertura de la sesión se detuvo de forma inesperada. Sus datos en el programa no se modificaron. Inténtelo de nuevo; si sigue ocurriendo, infórmelo adjuntando el archivo de registro.

### Almacenamiento de las señales capturadas

error-storage-channel_out_of_range = Ese canal no existe en esta captura. Elija un canal que la captura tenga.
error-storage-chunk_empty = El dispositivo entregó un bloque de datos vacío. Pruebe de nuevo; si sigue ocurriendo, infórmelo adjuntando el archivo de registro.
error-storage-chunk_out_of_order = Los datos llegaron en desorden, por lo que la captura se detuvo. Todo lo capturado hasta ahora se conserva. Pruebe de nuevo; si sigue ocurriendo, infórmelo adjuntando el archivo de registro.
error-storage-chunk_too_large = El dispositivo entregó de una vez más datos de los que el programa acepta. Pruebe de nuevo con una frecuencia de muestreo menor.
error-storage-closed = La captura ya terminó y no admite más datos. Inicie una nueva captura.
error-storage-internal = El programa encontró un problema inesperado al almacenar la captura. Todo lo capturado hasta ahora se conserva. Infórmelo adjuntando el archivo de registro.
error-storage-invalid_channel_count = No se admite esa cantidad de canales. Elija entre 1 y 32 canales.
error-storage-invalid_gap = Se describió incorrectamente un hueco de la captura. Todo lo capturado hasta ahora se conserva. Infórmelo adjuntando el archivo de registro.
error-storage-invalid_limits = Los límites de memoria o de tamaño no son válidos. Abra la configuración e introduzca valores válidos.
error-storage-invalid_pixels = No se pudo dibujar la vista con ese ancho. Cambie el tamaño de la ventana o el zoom y pruebe de nuevo.
error-storage-invalid_restore = La captura guardada no cumple las reglas de las capturas y no se puede abrir. Si tiene una copia de seguridad del archivo, ábrala en su lugar.
error-storage-invalid_sample_rate = La frecuencia de muestreo debe ser mayor que cero. Introduzca una frecuencia de muestreo válida.
error-storage-invalid_window = La parte de la captura que se debe mostrar no es válida. Seleccione un rango que empiece antes de terminar.
error-storage-memory_limit = La captura alcanzó el límite de memoria y se detuvo. Todo lo capturado hasta ahora se conserva. Guárdela y luego aumente el límite de memoria o capture menos datos.
error-storage-position_overflow = La captura se hizo más larga de lo que el programa puede manejar y se detuvo. Todo lo capturado hasta ahora se conserva. Guárdela y capture en partes más cortas.
