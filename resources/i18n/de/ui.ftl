### status: draft
### Maschinell übersetzter Entwurf; muss von einer Person mit Deutsch als Muttersprache geprüft werden.

### Anwendung

app-title = Logic Analyzer

### Status und Aufnahmeergebnisse

status-idle = Bereit
status-capturing = Aufnahme läuft
status-saving = Wird gespeichert
status-loading = Wird geladen
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
toolbar-length = Länge (Samples)
toolbar-seed = Startwert
toolbar-seed-random = zufällig
toolbar-profile = Signal
profile-sparse = Spärlich
profile-normal = Normal
profile-dense = Dicht
toolbar-realtime = Echtzeit
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
panel-channels = Kanäle
channel-move-up = Nach oben
channel-move-down = Nach unten
channel-colour = Farbe
channel-reset = Kanäle zurücksetzen

### Meldungen

notice-dismiss = Schließen
notice-dismiss-all = Alle schließen
notice-log = Protokollordner anzeigen
notice-list = Meldungen
menu-help = Hilfe
menu-help-log = Protokollordner anzeigen

### Markierungen und Messungen

menu-markers = Markierungen
menu-markers-place = Markierung setzen
menu-markers-previous = Vorherige Markierung
menu-markers-next = Nächste Markierung
menu-markers-remove = Markierung entfernen
menu-markers-remove-all = Alle Markierungen entfernen
menu-markers-snap = An Flanken einrasten
panel-markers = Markierungen
marker-default = Markierung { $number }
marker-go = Zur Markierung
marker-remove = Entfernen
marker-measure-first = Erste Messmarke
marker-measure-second = Zweite Messmarke
measure-distance = Abstand
measure-frequency = Frequenz
measure-pick = Wählen Sie zwei Markierungen, um die Zeit dazwischen zu messen.

### Sitzungsdateien

menu-file-open = Öffnen…
menu-file-save = Speichern
menu-file-save-as = Speichern unter…
menu-file-recent = Zuletzt verwendet
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
decoder-remove = Decoder entfernen
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
settings-save-decoder-results = Decodierte Ergebnisse mit der Sitzung speichern
settings-save-decoder-results-note = Die Datei wird größer. Ohne dies laufen die Decoder beim Öffnen der Datei erneut.
