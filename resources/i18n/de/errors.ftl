### status: draft
### Maschinell übersetzter Entwurf; muss von einer Person mit Deutsch als Muttersprache geprüft werden.
### Die Schlüssel entsprechen den englischen Texten (`en/errors.ftl`).

error-unexpected = Etwas Unerwartetes ist schiefgegangen (Code { $code }). Bitte versuchen Sie es erneut; wenn es weiterhin passiert, melden Sie es mit der Protokolldatei.

### Geräte

error-device-busy = Dieses Gerät wird bereits von einer anderen Aufnahme verwendet. Beenden Sie diese Aufnahme oder wählen Sie ein anderes Gerät.
error-device-disconnected = Die Verbindung zum Gerät wurde getrennt. Alles bisher Aufgenommene bleibt erhalten. Verbinden Sie das Gerät erneut und starten Sie eine neue Aufnahme.
error-device-driver_error = Das Gerät hat einen Fehler gemeldet und die Aufnahme wurde beendet. Alles bisher Aufgenommene bleibt erhalten. Trennen Sie das Gerät, verbinden Sie es erneut und versuchen Sie es noch einmal.
error-device-driver_panic = Der Treiber dieses Geräts wurde unerwartet beendet. Alles bisher Aufgenommene bleibt erhalten. Verbinden Sie das Gerät erneut und versuchen Sie es noch einmal; wenn es weiterhin passiert, melden Sie es mit der Protokolldatei.
error-device-invalid_capabilities = Das Gerät hat sich so beschrieben, dass das Programm es nicht verwenden kann; es steht daher nicht zur Verfügung. Verbinden Sie es erneut; besteht das Problem weiter, melden Sie es mit der Protokolldatei.
error-device-invalid_capture_length = Die gewünschte Aufnahmelänge ist mit diesem Gerät nicht möglich. Wählen Sie eine kürzere Länge oder ein anderes Gerät.
error-device-invalid_channel = Die gewählten Kanäle sind an diesem Gerät nicht vorhanden. Wählen Sie Kanäle, die das Gerät hat.
error-device-invalid_state = Diese Aktion ist im Moment nicht möglich. Warten Sie, bis der laufende Vorgang beendet ist, und versuchen Sie es erneut.
error-device-memory_budget = Eine Aufnahme dieser Länge passt möglicherweise nicht in den Speicher. Wählen Sie eine kürzere Aufnahme oder erlauben Sie dem Programm, mehr Speicher zu verwenden.
error-device-not_found = Das Gerät ist nicht angeschlossen oder nicht mehr verfügbar. Prüfen Sie die Verbindung und aktualisieren Sie die Geräteliste.
error-device-transport = Die Kommunikation mit dem Gerät ist fehlgeschlagen. Alles bisher Aufgenommene bleibt erhalten. Prüfen Sie Kabel und USB-Anschluss und versuchen Sie es dann erneut.
error-device-trigger_timeout = Die Triggerbedingung ist nicht rechtzeitig eingetreten, deshalb wurde die Aufnahme nicht gestartet. Prüfen Sie die Triggereinstellungen und das Signal oder nehmen Sie ohne Trigger auf.
error-device-unresponsive = Das Gerät antwortet nicht mehr. Alles bisher Aufgenommene bleibt erhalten. Trennen Sie es, verbinden Sie es erneut und versuchen Sie es noch einmal.
error-device-unsupported_sample_rate = Dieses Gerät kann nicht mit der gewählten Abtastrate aufnehmen. Wählen Sie eine der Raten, die das Gerät anbietet.
error-device-unsupported_trigger = Dieses Gerät unterstützt den gewählten Trigger nicht. Wählen Sie einen Trigger, den das Gerät anbietet, oder nehmen Sie ohne Trigger auf.

### Sitzungsdateien

error-session-cancelled = Der Vorgang wurde abgebrochen. Es wurde nichts geändert.
error-session-damaged = Die Sitzungsdatei ist beschädigt und kann nicht geöffnet werden. Wenn Sie eine Sicherungskopie der Datei haben, öffnen Sie stattdessen diese.
error-session-inconsistent = Die Sitzungsdatei enthält widersprüchliche Daten und kann nicht geöffnet werden. Wenn Sie eine Sicherungskopie der Datei haben, öffnen Sie stattdessen diese.
error-session-internal = Beim Verarbeiten der Sitzungsdatei ist im Programm ein unerwartetes Problem aufgetreten. Bitte versuchen Sie es erneut; wenn es weiterhin passiert, melden Sie es mit der Protokolldatei.
error-session-invalid_input = Der Dateiname oder der Speicherort kann nicht verwendet werden. Wählen Sie einen anderen Dateinamen oder Ordner.
error-session-io_read = Die Sitzungsdatei konnte nicht gelesen werden. Prüfen Sie, ob die Datei existiert und ob Sie sie öffnen dürfen, und versuchen Sie es dann erneut.
error-session-io_write = Die Sitzungsdatei konnte nicht gespeichert werden. Ihre vorhandene Datei wurde nicht verändert. Prüfen Sie, ob genügend Speicherplatz frei ist und ob Sie in den Ordner schreiben dürfen, oder wählen Sie einen anderen Speicherort.
error-session-memory_budget = Diese Sitzung ist zu groß, um sie mit der aktuellen Speichergrenze zu öffnen. Erlauben Sie dem Programm, mehr Speicher zu verwenden, oder öffnen Sie eine kleinere Datei.
error-session-not_a_session = Diese Datei ist keine Sitzungsdatei dieses Programms. Wählen Sie eine Datei, die mit ihm gespeichert wurde.
error-session-too_large = Diese Sitzungsdatei ist größer, als das Programm zulässt. Öffnen Sie eine kleinere Datei.
error-session-truncated = Die Sitzungsdatei ist unvollständig, wahrscheinlich weil sie beim Kopieren oder Speichern abgeschnitten wurde. Wenn Sie eine Sicherungskopie haben, öffnen Sie stattdessen diese.
error-session-unknown_critical_section = Diese Sitzungsdatei benötigt eine neuere Version des Programms. Aktualisieren Sie das Programm, um sie zu öffnen.
error-session-unsupported_version = Diese Sitzungsdatei wurde mit einer neueren Version des Programms erstellt. Aktualisieren Sie das Programm, um sie zu öffnen.
error-session-worker_failed = Das Speichern oder Öffnen der Sitzung wurde unerwartet beendet. Ihre Daten im Programm wurden nicht verändert. Bitte versuchen Sie es erneut; wenn es weiterhin passiert, melden Sie es mit der Protokolldatei.

### Speicherung der aufgenommenen Signale

error-storage-channel_out_of_range = Dieser Kanal existiert in dieser Aufnahme nicht. Wählen Sie einen Kanal, den die Aufnahme hat.
error-storage-chunk_empty = Das Gerät hat einen leeren Datenblock geliefert. Versuchen Sie es erneut; wenn es weiterhin passiert, melden Sie es mit der Protokolldatei.
error-storage-chunk_out_of_order = Daten sind in falscher Reihenfolge eingetroffen, deshalb wurde die Aufnahme beendet. Alles bisher Aufgenommene bleibt erhalten. Versuchen Sie es erneut; wenn es weiterhin passiert, melden Sie es mit der Protokolldatei.
error-storage-chunk_too_large = Das Gerät hat auf einmal mehr Daten geliefert, als das Programm annimmt. Versuchen Sie es mit einer niedrigeren Abtastrate erneut.
error-storage-closed = Die Aufnahme ist bereits beendet und kann keine weiteren Daten aufnehmen. Starten Sie eine neue Aufnahme.
error-storage-internal = Beim Speichern der Aufnahme ist im Programm ein unerwartetes Problem aufgetreten. Alles bisher Aufgenommene bleibt erhalten. Bitte melden Sie es mit der Protokolldatei.
error-storage-invalid_channel_count = Die Anzahl der Kanäle wird nicht unterstützt. Wählen Sie zwischen 1 und 32 Kanälen.
error-storage-invalid_gap = Eine Lücke in der Aufnahme wurde fehlerhaft beschrieben. Alles bisher Aufgenommene bleibt erhalten. Bitte melden Sie es mit der Protokolldatei.
error-storage-invalid_limits = Die Speicher- oder Größenbegrenzungen sind ungültig. Öffnen Sie die Einstellungen und geben Sie gültige Werte ein.
error-storage-invalid_pixels = Die Ansicht konnte in dieser Breite nicht gezeichnet werden. Ändern Sie die Fenstergröße oder den Zoom und versuchen Sie es erneut.
error-storage-invalid_restore = Die gespeicherte Aufnahme hält die Regeln für Aufnahmen nicht ein und kann nicht geöffnet werden. Wenn Sie eine Sicherungskopie der Datei haben, öffnen Sie stattdessen diese.
error-storage-invalid_sample_rate = Die Abtastrate muss größer als null sein. Geben Sie eine gültige Abtastrate ein.
error-storage-invalid_window = Der anzuzeigende Teil der Aufnahme ist ungültig. Wählen Sie einen Bereich, dessen Anfang vor seinem Ende liegt.
error-storage-memory_limit = Die Aufnahme hat die Speichergrenze erreicht und wurde beendet. Alles bisher Aufgenommene bleibt erhalten. Speichern Sie sie und erhöhen Sie dann die Speichergrenze oder nehmen Sie weniger Daten auf.
error-storage-position_overflow = Die Aufnahme wurde länger, als das Programm verarbeiten kann, und wurde beendet. Alles bisher Aufgenommene bleibt erhalten. Speichern Sie sie und nehmen Sie in kürzeren Abschnitten auf.

### Anwendung und Einstellungen

error-host-settings_unreadable = Die Einstellungsdatei konnte nicht gelesen werden, daher gelten die Standardeinstellungen. Prüfen Sie, ob Sie die Datei lesen dürfen; Ihre Auswahl wird wieder gespeichert, sobald Sie etwas ändern.
error-host-settings_invalid = Die Einstellungsdatei ist beschädigt oder stammt aus einer anderen Version, daher gelten die Standardeinstellungen. Wählen Sie Ihre Einstellungen erneut; sie werden gespeichert, sobald Sie etwas ändern.
error-host-settings_write = Ihre Einstellungen konnten nicht gespeichert werden und gehen beim Schließen des Programms verloren. Prüfen Sie, ob der Einstellungsordner existiert und ob Sie darin schreiben dürfen.
error-host-view_invalid = Die gespeicherte Ansicht dieser Sitzung (Kanalnamen, Farben und Markierungen) konnte nicht wiederhergestellt werden, daher werden die Standardwerte angezeigt. Die aufgenommenen Signale sind nicht betroffen. Wählen Sie Namen und Farben erneut, wenn Sie sie brauchen.
error-host-render_failed = Die Signale konnten nicht gezeichnet werden. Die Aufnahme ist nicht betroffen und lässt sich weiterhin speichern. Ändern Sie den Zoom oder die Fenstergröße; wenn das Problem bestehen bleibt, melden Sie es mit der Protokolldatei.
error-host-internal = Beim Ausführen Ihres Befehls ist ein unerwartetes Problem aufgetreten, und der Befehl wurde nicht ausgeführt. Ihre Aufnahme ist nicht betroffen. Versuchen Sie es erneut; wenn das Problem bestehen bleibt, melden Sie es mit der Protokolldatei.
error-host-invalid_command = Diese Aktion ist im Moment nicht möglich. Warten Sie, bis der aktuelle Vorgang beendet ist, und versuchen Sie es erneut.

# Decoders: faults, refused choices and the saved decoder setup.
error-decoder-failed = Ein Decoder wurde mit einem Fehler beendet. Die bis dahin erzeugten Annotationen bleiben erhalten und sind als unvollständig markiert. Ändern Sie seine Einstellungen oder starten Sie ihn erneut; die Aufnahme ist nicht betroffen.
error-decoder-panicked = Ein Decoder ist unerwartet ausgefallen. Die bis dahin erzeugten Annotationen bleiben erhalten und sind als unvollständig markiert. Starten Sie ihn erneut; wenn das Problem bestehen bleibt, melden Sie es mit der Protokolldatei. Die Aufnahme ist nicht betroffen.
error-decoder-not_responding = Ein Decoder antwortet nicht mehr und wurde aufgegeben. Was er zuvor erzeugt hat, bleibt erhalten und ist als unvollständig markiert. Starten Sie ihn erneut oder entfernen Sie ihn; die Aufnahme ist nicht betroffen.
error-decoder-memory_limit = Die Decoder-Ergebnisse haben das Speicherlimit erreicht, deshalb wurde das Decodieren beendet. Alles bisher Gefundene bleibt erhalten und ist als unvollständig markiert. Entfernen Sie andere Decoder oder erhöhen Sie das Limit in den Einstellungen.
error-decoder-annotation_limit = Ein Decoder hat das Limit für die Anzahl der Annotationen erreicht und wurde beendet. Alles bisher Gefundene bleibt erhalten und ist als unvollständig markiert. Decodieren Sie einen kürzeren Teil der Aufnahme oder wählen Sie strengere Einstellungen.
error-decoder-invalid_output = Ein Decoder hat einige ungültige Ergebnisse erzeugt. Sie wurden übersprungen; die gültigen Ergebnisse werden angezeigt. Wenn das Problem bestehen bleibt, melden Sie es mit der Protokolldatei.
error-decoder-invalid_setup = Diese Einstellung ist für diesen Decoder nicht gültig, deshalb wurde nichts geändert. Wählen Sie einen anderen Kanal oder Wert.
error-decoder-stack_cycle = Diese Auswahl würde Decoder im Kreis voneinander abhängig machen, deshalb wurde nichts geändert. Wählen Sie eine andere Quelle.
error-decoder-stack_incompatible = Dieser Decoder kann die Ergebnisse des gewählten Decoders nicht lesen, deshalb wurde nichts geändert. Wählen Sie eine andere Quelle.
error-decoder-stack_too_deep = Diese Decoder-Kette wäre zu lang, deshalb wurde nichts geändert. Verwenden Sie eine kürzere Kette.
error-decoder-too_many_instances = Das Limit für Decoder pro Aufnahme ist erreicht, deshalb wurde kein Decoder hinzugefügt. Entfernen Sie zuerst einen.
error-decoder-too_many_stuck = Mehrere Decoder antworten nicht mehr und hängen noch, deshalb wird kein neues Decodieren gestartet. Starten Sie die Anwendung neu, um sie freizugeben; die Aufnahme ist nicht betroffen.
error-decoder-section_invalid = Der Decoder-Teil dieser Sitzungsdatei ist beschädigt. Die lesbaren Decoder wurden wiederhergestellt; die Aufnahme und der Rest der Sitzung sind nicht betroffen.
error-decoder-unknown_decoder = Diese Sitzung verwendet einen Decoder, den diese Version der Anwendung nicht hat. Er wurde übersprungen und der Rest der Sitzung wurde wiederhergestellt.
error-decoder-options_skipped = Einige gespeicherte Decoder-Optionen wurden nicht verstanden und übersprungen, deshalb gelten dafür die Standardwerte. Prüfen Sie die Decoder-Einstellungen.
