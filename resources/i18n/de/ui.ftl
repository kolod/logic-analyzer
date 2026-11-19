### status: draft
### Maschinell übersetzter Entwurf; muss von einer Person mit Deutsch als Muttersprache geprüft werden.

### Anwendung

app-title = Logic Analyzer

### Status und Aufnahmeergebnisse

status-idle = Bereit
status-capturing = Aufnahme läuft
status-saving = Wird gespeichert
status-loading = Wird geladen
status-cropping = Daten werden gelöscht
status-memory = { $used } / { $limit }
end-reason-completed = Aufnahme abgeschlossen
end-reason-stopped = Von Ihnen angehalten
end-reason-memory-limit = Speichergrenze erreicht
end-reason-disconnected = Gerät getrennt
end-reason-device-error = Gerätefehler
end-reason-samples-lost = Mit verlorenen Samples abgeschlossen
end-reason-loaded = Aus Datei geladen
notice-device-removed = Das ausgewählte Gerät „{ $text }“ ist nicht mehr verfügbar; das Standardgerät wurde ausgewählt.

### Menüs

menu-file = Datei
menu-file-new = Neue Aufnahme
menu-file-quit = Beenden
menu-capture = Aufnahme
menu-capture-start = Starten
menu-capture-stop = Anhalten

### Werkzeugleiste, Ansicht und Dialoge

toolbar-device = Gerät
toolbar-rate = Abtastrate (Hz)
toolbar-rate-choice = Abtastrate
toolbar-seed = Startwert
toolbar-seed-random = zufällig
toolbar-profile = Signal
profile-sparse = Spärlich
profile-normal = Normal
profile-dense = Dicht
toolbar-start = Starten
toolbar-stop = Anhalten
status-simulated = Simuliert
status-incomplete = Unvollständig
status-unsaved = Nicht gespeicherte Änderungen
view-no-capture = Noch keine Aufnahme. Wählen Sie ein Gerät und drücken Sie Starten.
view-empty = Die Aufnahme enthält keine Samples.
view-cannot-draw = Die Signale können nicht gezeichnet werden.
channel-default = Kanal { $number }
dialog-unsaved-title = Nicht gespeicherte Aufnahme
dialog-unsaved-body = Die Aufnahme wurde nicht gespeichert. Was möchten Sie damit tun?
dialog-save = Speichern
dialog-discard = Verwerfen
dialog-cancel = Abbrechen

### Ansicht und Kanäle

menu-view = Ansicht
menu-view-zoom-in = Vergrößern
menu-view-zoom-out = Verkleinern
menu-view-fit = Alles anzeigen
menu-view-start = Zum Anfang
menu-view-end = Zum Ende
menu-view-pan-left = Nach links
menu-view-pan-right = Nach rechts
menu-view-page-left = Seite zurück
menu-view-page-right = Seite vor
menu-capture-follow = Neueste Daten folgen
channel-reset = Zurücksetzen

### Meldungen

notice-dismiss = Schließen
notice-dismiss-all = Alle schließen
notice-log = Protokollordner anzeigen
notice-list = Meldungen
menu-help = Hilfe
menu-help-log = Protokollordner anzeigen

### Markierungen und Messungen

menu-markers = Markierungen
menu-markers-place-single = Einzelmarkierung setzen
menu-markers-place-pair = Markierungspaar setzen
menu-markers-cancel = Platzieren abbrechen
menu-markers-previous = Vorherige Markierung
menu-markers-next = Nächste Markierung
menu-markers-remove = Markierung entfernen
menu-markers-remove-all = Alle Markierungen entfernen
menu-markers-snap = An Flanken einrasten
panel-markers = Markierungen
marker-single-add = Einzelmarkierung
marker-pair-add = Markierungspaar
marker-empty = Keine Markierungen. Mit den Schaltflächen oben fügen Sie eine hinzu.
marker-placing-single = Klicken, um die Markierung zu setzen. Esc bricht ab.
marker-placing-pair-first = Klicken, um das erste Ende des Paars zu setzen. Esc bricht ab.
marker-placing-pair-second = Klicken, um das zweite Ende zu setzen. Esc bricht ab.
marker-zoomed-in = Hineingezoomt, um die Flanke zu finden. Die Ansicht kehrt nach einem Klick oder mit Esc zurück.
marker-limit = Das Limit von 64 Markierungen ist erreicht. Löschen Sie eine, um eine neue hinzuzufügen.
marker-actions = Aktionen
marker-delete = Löschen
marker-comment = Kommentar
marker-make-pair = Paar bilden
marker-action-origin = Zeit ab hier zählen
marker-action-origin-reset = Zeit zählt ab dem Anfang
marker-action-cut-left = Daten links löschen
marker-action-cut-right = Daten rechts löschen
marker-action-scroll = Dorthin scrollen
marker-action-trim = Auf das Paar zuschneiden
marker-action-fit = Ansicht an das Paar anpassen
marker-action-split = In zwei Einzelmarkierungen umwandeln
marker-cut-disabled-running = Nicht möglich, solange eine Aufnahme läuft
marker-cut-disabled-nothing = Auf dieser Seite gibt es nichts zu löschen
marker-cut-pair-removed = Paare mit nur einem verbleibenden Ende wurden entfernt: { $text }.
dialog-crop-title = Daten löschen?
dialog-crop-left-body = Die Daten links von der Markierung werden gelöscht: { $lost } ({ $percent } %). Es bleiben { $kept }. Das kann nicht rückgängig gemacht werden.
dialog-crop-right-body = Die Daten rechts von der Markierung werden gelöscht: { $lost } ({ $percent } %). Es bleiben { $kept }. Das kann nicht rückgängig gemacht werden.
dialog-crop-pair-body = Nur die Daten zwischen den Enden des Paars bleiben erhalten: { $kept }. { $lost } ({ $percent } %) werden gelöscht. Das kann nicht rückgängig gemacht werden.
dialog-crop-confirm = Löschen
dialog-cropping = Daten werden gelöscht
marker-colour-0 = Rot
marker-colour-1 = Bernstein
marker-colour-2 = Grün
marker-colour-3 = Türkis
marker-colour-4 = Blau
marker-colour-5 = Violett
marker-colour-6 = Rosa
marker-colour-7 = Grau
panel-capture = Aufnahme
panel-view-capture = Aufnahmeeinstellungen
panel-view-decoders = Decoder
panel-view-markers = Marker und Messen
capture-length-seconds = Länge (Sekunden)
capture-length-samples = Abtastwerte: { $count }
capture-length-invalid = Geben Sie die Länge als Zahl in Sekunden ein, zum Beispiel 2,5.
capture-length-decimals = Höchstens neun Stellen nach dem Komma.
capture-length-small = Das ist bei dieser Abtastrate weniger als ein Abtastwert.
capture-length-large = Das ist länger, als eine Aufnahme sein kann.
capture-length-no-rate = Stellen Sie zuerst die Abtastrate ein.
capture-channels = Aufzunehmende Kanäle
capture-channels-all = Alle
capture-channels-none = Keine
capture-no-channel = Kein Kanal gewählt: Wählen Sie mindestens einen zum Starten.

### Sitzungsdateien

menu-file-open = Öffnen…
menu-file-save = Speichern
menu-file-save-as = Speichern unter…
menu-file-recent = Zuletzt verwendet
menu-file-recent-empty = Keine letzten Dateien
menu-file-recent-missing = (fehlt)
menu-file-recent-remove-missing = Fehlende Einträge entfernen
menu-file-recent-clear = Liste leeren
dialog-saving = Sitzung wird gespeichert…
dialog-opening = Sitzung wird geöffnet…
dialog-open-title = Sitzung öffnen
dialog-save-title = Sitzung speichern
dialog-filter-session = Logikanalysator-Sitzungen
title-untitled = Unbenannte Aufnahme
notice-view-newer = Die Ansichtseinstellungen dieser Datei stammen aus einer neueren Version und wurden nicht angewendet. Sie bleiben unverändert in der Datei.

### Einstellungen und Info

menu-file-settings = Einstellungen…
menu-view-scale-up = Größere Oberfläche
menu-view-scale-down = Kleinere Oberfläche
menu-help-about = Über
settings-title = Einstellungen
settings-unsaved-title = Nicht gespeicherte Aufnahme
settings-remember-choice = Auswahl merken
settings-language = Sprache
settings-theme = Farbschema
theme-system = Wie das System
theme-light = Hell
theme-dark = Dunkel
theme-high-contrast = Hoher Kontrast
settings-scale = Skalierung
settings-memory = Speichergrenze einer Aufnahme
settings-memory-unit = MiB
settings-memory-note = Gilt für die nächste Aufnahme.
settings-memory-invalid = Geben Sie eine ganze Zahl in MiB von { $min } bis { $max } ein.
dialog-close = Schließen
about-title = Über
about-body = Logic Analyzer { $version }. Zeichnet digitale Signale auf und zeigt sie auf einer Zeitleiste; ein simuliertes Gerät erlaubt das Ausprobieren ohne Hardware.
about-license = Freie Software unter der GNU General Public License, Version 3.

### Decoders: the sample decoder
decoder-sample-clocked-bits-name = Getaktete Bits
decoder-sample-clocked-bits-description = Liest Bits von einer Datenleitung bei den Flanken einer Taktleitung und fasst sie zu Bytes zusammen. Ein Beispiel-Decoder, der zeigt, was Decoder können.
decoder-sample-clocked-bits-signal-clock = Takt
decoder-sample-clocked-bits-signal-data = Daten
decoder-sample-clocked-bits-option-clock_edge = Taktflanke
decoder-sample-clocked-bits-choice-clock_edge-rising = Steigend
decoder-sample-clocked-bits-choice-clock_edge-falling = Fallend
decoder-sample-clocked-bits-option-bit_order = Bitreihenfolge
decoder-sample-clocked-bits-choice-bit_order-msb = Höchstwertiges Bit zuerst
decoder-sample-clocked-bits-choice-bit_order-lsb = Niedrigstwertiges Bit zuerst
decoder-sample-clocked-bits-option-bits = Bits pro Byte
decoder-sample-clocked-bits-option-idle = Leerlaufgrenze (Abtastwerte)
ann-sample-clocked-bits-byte = Byte { $value }
ann-sample-clocked-bits-bit = Bit { $value }
ann-sample-clocked-bits-incomplete = Unvollständiges Byte: { $value } Bits
ann-sample-clocked-bits-gap = Hier gingen Abtastwerte verloren

### Decoders: I2C
decoder-i2c-name = I2C
decoder-i2c-description = Liest den I2C-Bus (zwei Leitungen, Takt und Daten): Adresse, Bytes und Bestätigungen jeder Übertragung.
decoder-i2c-signal-scl = Takt (SCL)
decoder-i2c-signal-sda = Daten (SDA)
decoder-i2c-option-address = Adressschreibweise
decoder-i2c-choice-address-addr7 = 7-Bit-Adresse (ohne Richtungsbit)
decoder-i2c-choice-address-addr8 = 8-Bit-Wert (mit Richtungsbit)
decoder-i2c-option-radix = Zahlenbasis
decoder-i2c-choice-radix-hex = Hexadezimal
decoder-i2c-choice-radix-dec = Dezimal
decoder-i2c-choice-radix-bin = Binär
ann-i2c-start = Start
ann-i2c-repeated-start = Wiederholter Start
ann-i2c-stop = Stopp
ann-i2c-transaction-write = Schreiben an { $value }
ann-i2c-transaction-read = Lesen von { $value }
ann-i2c-address-write = Adresse { $value } (Schreiben)
ann-i2c-address-read = Adresse { $value } (Lesen)
ann-i2c-data = Daten { $value }
ann-i2c-ack = ACK
ann-i2c-nak = NAK
ann-i2c-incomplete = Unvollständige Übertragung
ann-i2c-error-byte-cut = Byte nach { $value } Bits abgebrochen
ann-i2c-error-address-nak = Adresse { $value } nicht bestätigt
ann-i2c-error-data-nak = Daten { $value } nicht bestätigt
ann-i2c-error-empty = Leere Übertragung: keine Adresse
ann-i2c-error-gap = Hier gingen Abtastwerte verloren

### Decoders: menu, panel and the selected annotation
menu-decoders = Decoder
menu-decoders-add = Decoder hinzufügen
menu-decoders-previous = Vorherige Annotation
menu-decoders-next = Nächste Annotation
menu-decoders-goto = Zur ausgewählten Annotation
panel-decoders = Decoder
decoders-add = Decoder hinzufügen
decoders-empty = Noch keine Decoder. Fügen Sie einen hinzu, um zu sehen, was die Signale sagen.
decoders-need-capture = Nehmen Sie zuerst Signale auf oder öffnen Sie welche.
decoder-state-waiting = Wartet
decoder-state-running = Läuft
decoder-state-finished = Fertig
decoder-state-cancelled = Abgebrochen
decoder-state-failed = Fehlgeschlagen
decoder-needs-channels = Wählen Sie die Kanäle, um das Decodieren zu starten.
decoder-channel-none = (keiner)
decoder-lane-show = Spur anzeigen
decoder-annotations = Annotationen: { $number }
decoder-incomplete = Unvollständige Ergebnisse
decoder-selected = Ausgewählte Annotation
decoder-detail-length = Dauer: { $value }
decoder-detail-error = Meldet ein Problem im Signal
decoder-detail-position = Position: { $first } bis { $second }

### Decoders: the simulation of signals that are valid for a decoder
decoder-generator-errors = Absichtlich Fehler in die simulierten Signale einbauen
decoder-generator-rate = Etwa jedes so viele Byte wird abgebrochen

### Decoders: running, cancelling and the memory limit
decoder-run = Erneut ausführen
decoder-cancel = Abbrechen
settings-decoder-memory = Speicherlimit der Decoder-Ergebnisse
settings-decoder-memory-note = Gilt für alle Decoder zusammen und für den nächsten Durchlauf.

### Decoders: stacking
decoder-source = Liest von
decoder-source-capture = Die Aufnahme
dialog-remove-decoder-title = Decoder entfernen
dialog-remove-decoder-body = Andere Decoder lesen von diesem. Wenn Sie ihn entfernen, werden auch diese entfernt. Die Aufnahme ist nicht betroffen.
dialog-remove-decoder-confirm = Alle entfernen

### Decoders: saving and opening
notice-decoders-newer = Diese Datei enthält Decoder einer neueren Version der Anwendung. Sie bleiben unverändert erhalten und werden nicht angezeigt; die Aufnahme wird normal geöffnet.
notice-layout-newer = Diese Datei enthält Signalhöhen, die von einer neueren Version der Anwendung gespeichert wurden. Sie bleiben unverändert erhalten und werden nicht angezeigt; die Aufnahme wird mit automatischen Höhen geöffnet.
settings-save-decoder-results = Decodierte Ergebnisse mit der Sitzung speichern
settings-save-decoder-results-note = Die Datei wird größer. Ohne dies laufen die Decoder beim Öffnen der Datei erneut.
