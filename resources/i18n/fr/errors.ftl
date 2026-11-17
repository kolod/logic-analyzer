### status: draft
### Version préliminaire traduite automatiquement ; à relire par une personne de langue maternelle française.
### Les clés correspondent aux textes anglais (`en/errors.ftl`).

error-unexpected = Un problème inattendu est survenu (code { $code }). Veuillez réessayer ; si le problème persiste, signalez-le avec le fichier journal.

### Appareils

error-device-busy = Cet appareil est déjà utilisé par une autre acquisition. Arrêtez cette acquisition ou choisissez un autre appareil.
error-device-disconnected = L'appareil a été déconnecté. Tout ce qui a été acquis jusqu'ici est conservé. Reconnectez l'appareil et lancez une nouvelle acquisition.
error-device-driver_error = L'appareil a signalé une erreur et l'acquisition s'est arrêtée. Tout ce qui a été acquis jusqu'ici est conservé. Débranchez puis rebranchez l'appareil, puis réessayez.
error-device-driver_panic = Le pilote de cet appareil s'est arrêté de façon inattendue. Tout ce qui a été acquis jusqu'ici est conservé. Reconnectez l'appareil et réessayez ; si le problème persiste, signalez-le avec le fichier journal.
error-device-invalid_capabilities = L'appareil s'est décrit d'une façon que le programme ne peut pas utiliser ; il n'est donc pas disponible. Reconnectez-le et, si le problème subsiste, signalez-le avec le fichier journal.
error-device-invalid_capture_length = La durée d'acquisition demandée n'est pas possible avec cet appareil. Choisissez une durée plus courte ou un autre appareil.
error-device-invalid_channel = Les voies choisies n'existent pas sur cet appareil. Choisissez des voies que l'appareil possède.
error-device-invalid_state = Cette action n'est pas possible pour le moment. Attendez la fin de l'opération en cours et réessayez.
error-device-memory_budget = Une acquisition de cette durée risque de ne pas tenir en mémoire. Choisissez une acquisition plus courte ou autorisez le programme à utiliser plus de mémoire.
error-device-not_found = L'appareil n'est pas connecté ou n'est plus disponible. Vérifiez la connexion et actualisez la liste des appareils.
error-device-transport = La communication avec l'appareil a échoué. Tout ce qui a été acquis jusqu'ici est conservé. Vérifiez le câble et le port USB, puis réessayez.
error-device-trigger_timeout = La condition de déclenchement ne s'est pas produite à temps, donc l'acquisition n'a pas démarré. Vérifiez les réglages du déclenchement et le signal, ou acquérez sans déclenchement.
error-device-unresponsive = L'appareil ne répond plus. Tout ce qui a été acquis jusqu'ici est conservé. Débranchez-le puis rebranchez-le, puis réessayez.
error-device-unsupported_sample_rate = Cet appareil ne peut pas acquérir à la fréquence d'échantillonnage choisie. Choisissez l'une des fréquences proposées par l'appareil.
error-device-unsupported_trigger = Cet appareil ne prend pas en charge le déclenchement choisi. Choisissez un déclenchement proposé par l'appareil ou acquérez sans déclenchement.

### Fichiers de session

error-session-cancelled = L'opération a été annulée. Rien n'a été modifié.
error-session-damaged = Le fichier de session est endommagé et ne peut pas être ouvert. Si vous avez une copie de sauvegarde du fichier, ouvrez-la à la place.
error-session-inconsistent = Le fichier de session contient des données incohérentes et ne peut pas être ouvert. Si vous avez une copie de sauvegarde du fichier, ouvrez-la à la place.
error-session-internal = Le programme a rencontré un problème inattendu lors du traitement du fichier de session. Veuillez réessayer ; si le problème persiste, signalez-le avec le fichier journal.
error-session-invalid_input = Le nom ou l'emplacement du fichier ne peut pas être utilisé. Choisissez un autre nom de fichier ou un autre dossier.
error-session-io_read = Le fichier de session n'a pas pu être lu. Vérifiez que le fichier existe et que vous avez le droit de l'ouvrir, puis réessayez.
error-session-io_write = Le fichier de session n'a pas pu être enregistré. Votre fichier existant n'a pas été modifié. Vérifiez qu'il reste de l'espace disque et que vous pouvez écrire dans le dossier, ou choisissez un autre emplacement.
error-session-memory_budget = Cette session est trop volumineuse pour être ouverte avec la limite de mémoire actuelle. Autorisez le programme à utiliser plus de mémoire ou ouvrez un fichier plus petit.
error-session-not_a_session = Ce fichier n'est pas un fichier de session de ce programme. Choisissez un fichier enregistré par celui-ci.
error-session-too_large = Ce fichier de session est plus volumineux que ce que le programme autorise. Ouvrez un fichier plus petit.
error-session-truncated = Le fichier de session est incomplet, probablement parce qu'il a été tronqué lors d'une copie ou d'un enregistrement. Si vous avez une copie de sauvegarde, ouvrez-la à la place.
error-session-unknown_critical_section = Ce fichier de session nécessite une version plus récente du programme. Mettez le programme à jour pour l'ouvrir.
error-session-unsupported_version = Ce fichier de session a été créé par une version plus récente du programme. Mettez le programme à jour pour l'ouvrir.
error-session-worker_failed = L'enregistrement ou l'ouverture de la session s'est arrêté de façon inattendue. Vos données dans le programme n'ont pas été modifiées. Veuillez réessayer ; si le problème persiste, signalez-le avec le fichier journal.

### Stockage des signaux acquis

error-storage-channel_out_of_range = Cette voie n'existe pas dans cette acquisition. Choisissez une voie que l'acquisition possède.
error-storage-chunk_empty = L'appareil a envoyé un bloc de données vide. Réessayez ; si le problème persiste, signalez-le avec le fichier journal.
error-storage-chunk_out_of_order = Des données sont arrivées dans le désordre, donc l'acquisition a été arrêtée. Tout ce qui a été acquis jusqu'ici est conservé. Réessayez ; si le problème persiste, signalez-le avec le fichier journal.
error-storage-chunk_too_large = L'appareil a envoyé en une fois plus de données que le programme n'en accepte. Réessayez avec une fréquence d'échantillonnage plus basse.
error-storage-closed = L'acquisition est déjà terminée et ne peut plus recevoir de données. Lancez une nouvelle acquisition.
error-storage-internal = Le programme a rencontré un problème inattendu lors du stockage de l'acquisition. Tout ce qui a été acquis jusqu'ici est conservé. Veuillez le signaler avec le fichier journal.
error-storage-invalid_channel_count = Le nombre de voies n'est pas pris en charge. Choisissez entre 1 et 32 voies.
error-storage-invalid_gap = Une lacune de l'acquisition a été décrite de façon incorrecte. Tout ce qui a été acquis jusqu'ici est conservé. Veuillez le signaler avec le fichier journal.
error-storage-invalid_limits = Les limites de mémoire ou de taille ne sont pas valides. Ouvrez les paramètres et saisissez des valeurs valides.
error-storage-invalid_pixels = La vue n'a pas pu être dessinée à cette largeur. Redimensionnez la fenêtre ou modifiez le zoom, puis réessayez.
error-storage-invalid_restore = L'acquisition enregistrée ne respecte pas les règles des acquisitions et ne peut pas être ouverte. Si vous avez une copie de sauvegarde du fichier, ouvrez-la à la place.
error-storage-invalid_sample_rate = La fréquence d'échantillonnage doit être supérieure à zéro. Saisissez une fréquence d'échantillonnage valide.
error-storage-invalid_window = La partie de l'acquisition à afficher n'est pas valide. Sélectionnez une plage dont le début précède la fin.
error-storage-memory_limit = L'acquisition a atteint la limite de mémoire et s'est arrêtée. Tout ce qui a été acquis jusqu'ici est conservé. Enregistrez-la, puis augmentez la limite de mémoire ou acquérez moins de données.
error-storage-position_overflow = L'acquisition est devenue plus longue que ce que le programme peut gérer et a été arrêtée. Tout ce qui a été acquis jusqu'ici est conservé. Enregistrez-la et acquérez par parties plus courtes.

### Application et paramètres

error-host-settings_unreadable = Le fichier des paramètres n'a pas pu être lu ; les paramètres par défaut sont utilisés. Vérifiez que vous avez le droit de lire le fichier ; vos choix seront de nouveau enregistrés dès que vous les modifierez.
error-host-settings_invalid = Le fichier des paramètres est endommagé ou provient d'une autre version ; les paramètres par défaut sont utilisés. Choisissez de nouveau vos paramètres ; ils seront enregistrés dès que vous les modifierez.
error-host-settings_write = Vos paramètres n'ont pas pu être enregistrés et seront perdus à la fermeture du programme. Vérifiez que le dossier des paramètres existe et que vous pouvez y écrire.
error-host-view_invalid = L'affichage enregistré de cette session (noms des canaux, couleurs et repères) n'a pas pu être restauré ; les valeurs par défaut sont affichées. Les signaux capturés ne sont pas affectés. Choisissez de nouveau les noms et les couleurs si vous en avez besoin.
error-host-layout_invalid = Les hauteurs enregistrées des signaux de cette session n'ont pas pu être restaurées ; les hauteurs automatiques sont affichées. Les signaux capturés ne sont pas affectés. Choisissez à nouveau les hauteurs si vous le souhaitez.
error-host-render_failed = Les signaux n'ont pas pu être dessinés. La capture n'est pas affectée et peut toujours être enregistrée. Modifiez le zoom ou la taille de la fenêtre ; si le problème persiste, signalez-le en joignant le fichier journal.
error-host-internal = Un problème inattendu est survenu lors de l'exécution de votre commande, qui n'a pas été exécutée. Votre capture n'est pas affectée. Réessayez ; si le problème persiste, signalez-le en joignant le fichier journal.
error-host-invalid_command = Cette action n'est pas possible pour le moment. Attendez la fin de l'opération en cours, puis réessayez.

# Decoders: faults, refused choices and the saved decoder setup.
error-decoder-failed = Un décodeur s'est arrêté avec une erreur. Les annotations produites avant sont conservées et marquées comme incomplètes. Modifiez ses réglages ou relancez-le ; la capture n'est pas affectée.
error-decoder-panicked = Un décodeur a échoué de façon inattendue. Les annotations produites avant sont conservées et marquées comme incomplètes. Relancez-le ; si le problème persiste, signalez-le en joignant le fichier journal. La capture n'est pas affectée.
error-decoder-not_responding = Un décodeur ne répond plus et a été abandonné. Ce qu'il a produit avant est conservé et marqué comme incomplet. Relancez-le ou supprimez-le ; la capture n'est pas affectée.
error-decoder-memory_limit = Les résultats des décodeurs ont atteint la limite de mémoire, le décodage s'est donc arrêté. Tout ce qui a été trouvé jusque-là est conservé et marqué comme incomplet. Supprimez d'autres décodeurs ou augmentez la limite dans les réglages.
error-decoder-annotation_limit = Un décodeur a atteint la limite du nombre d'annotations et s'est arrêté. Tout ce qui a été trouvé jusque-là est conservé et marqué comme incomplet. Décodez une partie plus courte de la capture ou choisissez des réglages plus stricts.
error-decoder-invalid_output = Un décodeur a produit des résultats non valides. Ils ont été ignorés ; les résultats valides sont affichés. Si le problème persiste, signalez-le en joignant le fichier journal.
error-decoder-invalid_setup = Ce choix de réglage n'est pas valide pour ce décodeur, rien n'a donc été modifié. Choisissez un autre canal ou une autre valeur.
error-decoder-stack_cycle = Ce choix ferait dépendre des décodeurs les uns des autres en boucle, rien n'a donc été modifié. Choisissez une autre source.
error-decoder-stack_incompatible = Ce décodeur ne peut pas lire les résultats du décodeur choisi, rien n'a donc été modifié. Choisissez une autre source.
error-decoder-stack_too_deep = Cette chaîne de décodeurs serait trop longue, rien n'a donc été modifié. Utilisez une chaîne plus courte.
error-decoder-too_many_instances = La limite de décodeurs par capture est atteinte, aucun décodeur n'a donc été ajouté. Supprimez-en un d'abord.
error-decoder-too_many_stuck = Plusieurs décodeurs ne répondent plus et sont toujours bloqués, aucun nouveau décodage n'est donc lancé. Redémarrez l'application pour les libérer ; la capture n'est pas affectée.
error-decoder-section_invalid = La partie décodeurs de ce fichier de session est endommagée. Les décodeurs lisibles ont été restaurés ; la capture et le reste de la session ne sont pas affectés.
error-decoder-unknown_decoder = Cette session utilise un décodeur que cette version de l'application ne possède pas. Il a été ignoré et le reste de la session a été restauré.
error-decoder-options_skipped = Certaines options enregistrées d'un décodeur n'ont pas été comprises et ont été ignorées ; les valeurs par défaut sont donc utilisées. Vérifiez les réglages du décodeur.

### WeAct LogicAnalyzer

error-weact-access_denied = Ce programme n'est pas autorisé à ouvrir le WeAct LogicAnalyzer. Vérifiez que la règle d'accès (udev) décrite dans la documentation est installée, puis débranchez et rebranchez l'appareil.
error-weact-driver_missing = Le WeAct LogicAnalyzer n'utilise pas le pilote WinUSB et ne peut donc pas être ouvert. Consultez la documentation pour installer WinUSB avec Zadig, puis rebranchez l'appareil.
error-weact-in_use = Le WeAct LogicAnalyzer est utilisé par un autre programme. Fermez cet autre programme (par exemple PulseView) et réessayez.
error-weact-firmware_rejected = Le WeAct LogicAnalyzer n'a pas accepté le firmware ou signale une version que ce programme ne connaît pas. Débranchez l'appareil, rebranchez-le et réessayez.
error-weact-not_renumerated = Le WeAct LogicAnalyzer n'est pas revenu après le chargement de son firmware. Débranchez-le, attendez quelques secondes et rebranchez-le.
error-weact-preparing = Le WeAct LogicAnalyzer est encore en cours de préparation. Patientez un instant puis relancez.
error-weact-no_data = Le WeAct LogicAnalyzer n'a envoyé aucune donnée. Vérifiez le câble USB, essayez un autre port USB puis relancez la capture.
error-weact-stream_stopped = La capture s'est arrêtée prématurément car l'ordinateur n'a pas pu suivre le WeAct LogicAnalyzer. Tout ce qui a été capturé est conservé. Essayez une fréquence d'échantillonnage plus basse et évitez les concentrateurs et stations d'accueil USB.
error-weact-firmware_invalid = Le firmware du WeAct LogicAnalyzer fourni avec ce programme est endommagé. Veuillez réinstaller l'application.
