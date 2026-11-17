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

### Aplicación y configuración

error-host-settings_unreadable = No se pudo leer el archivo de configuración, por lo que se usa la configuración predeterminada. Compruebe que tiene permiso para leer el archivo; sus opciones se guardarán de nuevo cuando las cambie.
error-host-settings_invalid = El archivo de configuración está dañado o es de otra versión, por lo que se usa la configuración predeterminada. Vuelva a elegir sus opciones; se guardarán cuando las cambie.
error-host-settings_write = No se pudo guardar su configuración y se perderá al cerrar el programa. Compruebe que la carpeta de configuración existe y que puede escribir en ella.
error-host-view_invalid = No se pudo restaurar la vista guardada de esta sesión (nombres de canales, colores y marcadores), por lo que se muestran los valores predeterminados. Las señales capturadas no se ven afectadas. Vuelva a elegir los nombres y los colores si los necesita.
error-host-layout_invalid = No se pudieron restaurar las alturas guardadas de las señales de esta sesión, así que se muestran las alturas automáticas. Las señales capturadas no se ven afectadas. Elija de nuevo las alturas si lo desea.
error-host-render_failed = No se pudieron dibujar las señales. La captura no se ve afectada y aún puede guardarse. Cambie el zoom o el tamaño de la ventana; si sigue ocurriendo, infórmelo adjuntando el archivo de registro.
error-host-internal = Ocurrió un problema inesperado al ejecutar su orden, que no se ejecutó. Su captura no se ve afectada. Inténtelo de nuevo; si sigue ocurriendo, infórmelo adjuntando el archivo de registro.
error-host-invalid_command = Esa acción no es posible en este momento. Espere a que termine la operación en curso y pruebe de nuevo.

# Decoders: faults, refused choices and the saved decoder setup.
error-decoder-failed = Un decodificador se detuvo con un error. Las anotaciones producidas antes se conservan y se marcan como incompletas. Cambie su configuración o ejecútelo de nuevo; la captura no se ve afectada.
error-decoder-panicked = Un decodificador falló de forma inesperada. Las anotaciones producidas antes se conservan y se marcan como incompletas. Ejecútelo de nuevo; si sigue ocurriendo, infórmelo adjuntando el archivo de registro. La captura no se ve afectada.
error-decoder-not_responding = Un decodificador dejó de responder y se abandonó. Lo que produjo antes se conserva y se marca como incompleto. Ejecútelo de nuevo o quítelo; la captura no se ve afectada.
error-decoder-memory_limit = Los resultados de los decodificadores alcanzaron el límite de memoria, por lo que la decodificación se detuvo. Todo lo encontrado hasta ahora se conserva y se marca como incompleto. Quite otros decodificadores o aumente el límite en la configuración.
error-decoder-annotation_limit = Un decodificador alcanzó el límite de anotaciones y se detuvo. Todo lo encontrado hasta ahora se conserva y se marca como incompleto. Decodifique una parte más corta de la captura o elija ajustes más estrictos.
error-decoder-invalid_output = Un decodificador produjo algunos resultados no válidos. Se omitieron; se muestran los resultados válidos. Si sigue ocurriendo, infórmelo adjuntando el archivo de registro.
error-decoder-invalid_setup = Esa opción de configuración no es válida para este decodificador, por lo que no se cambió nada. Elija otro canal o valor.
error-decoder-stack_cycle = Esa elección haría que los decodificadores dependieran unos de otros en círculo, por lo que no se cambió nada. Elija otra fuente.
error-decoder-stack_incompatible = Ese decodificador no puede leer los resultados del decodificador elegido, por lo que no se cambió nada. Elija otra fuente.
error-decoder-stack_too_deep = Esa cadena de decodificadores sería demasiado larga, por lo que no se cambió nada. Use una cadena más corta.
error-decoder-too_many_instances = Se alcanzó el límite de decodificadores por captura, por lo que no se añadió ninguno. Quite uno primero.
error-decoder-too_many_stuck = Varios decodificadores dejaron de responder y siguen bloqueados, por lo que no se inicia una nueva decodificación. Reinicie la aplicación para liberarlos; la captura no se ve afectada.
error-decoder-section_invalid = La parte de decodificadores de este archivo de sesión está dañada. Se restauraron los decodificadores que se pudieron leer; la captura y el resto de la sesión no se ven afectados.
error-decoder-unknown_decoder = Esta sesión usa un decodificador que esta versión de la aplicación no tiene. Se omitió y se restauró el resto de la sesión.
error-decoder-options_skipped = Algunas opciones guardadas de un decodificador no se entendieron y se omitieron, por lo que se usan los valores predeterminados. Revise la configuración del decodificador.

### WeAct LogicAnalyzer

error-weact-access_denied = Este programa no tiene permiso para abrir el WeAct LogicAnalyzer. Compruebe que la regla de acceso (udev) de la documentación está instalada y luego desconecte y vuelva a conectar el dispositivo.
error-weact-driver_missing = El WeAct LogicAnalyzer no usa el controlador WinUSB, por lo que no se puede abrir. Consulte la documentación para instalar WinUSB con Zadig y vuelva a conectar el dispositivo.
error-weact-in_use = El WeAct LogicAnalyzer está siendo usado por otro programa. Cierre el otro programa (por ejemplo PulseView) e inténtelo de nuevo.
error-weact-firmware_rejected = El WeAct LogicAnalyzer no aceptó el firmware o indica una versión que este programa no conoce. Desconecte el dispositivo, vuelva a conectarlo e inténtelo de nuevo.
error-weact-not_renumerated = El WeAct LogicAnalyzer no volvió tras cargar su firmware. Desconéctelo, espere unos segundos y vuelva a conectarlo.
error-weact-preparing = El WeAct LogicAnalyzer todavía se está preparando. Espere un momento y vuelva a iniciar.
error-weact-no_data = El WeAct LogicAnalyzer no envió ningún dato. Compruebe el cable USB, pruebe con otro puerto USB y vuelva a iniciar la captura.
error-weact-stream_stopped = La captura terminó antes de tiempo porque el ordenador no pudo seguir el ritmo del WeAct LogicAnalyzer. Todo lo capturado hasta ahora se conserva. Pruebe con una frecuencia de muestreo más baja y evite los concentradores y las estaciones de acoplamiento USB.
error-weact-firmware_invalid = El firmware del WeAct LogicAnalyzer incluido con este programa está dañado. Reinstale la aplicación.
