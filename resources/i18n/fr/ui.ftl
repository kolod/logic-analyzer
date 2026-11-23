### status: draft
### Brouillon traduit automatiquement ; doit être relu par une personne de langue maternelle française.

### Application

app-title = Logic Analyzer

### État et résultats de la capture

status-idle = Prêt
status-capturing = Capture en cours
status-saving = Enregistrement
status-loading = Chargement
status-cropping = Suppression des données
status-memory = { $used } / { $limit }
end-reason-completed = Capture terminée
end-reason-stopped = Arrêtée par vous
end-reason-memory-limit = Limite de mémoire atteinte
end-reason-disconnected = Appareil déconnecté
end-reason-device-error = Erreur de l'appareil
end-reason-samples-lost = Terminée avec des échantillons perdus
end-reason-loaded = Chargée depuis un fichier
notice-device-removed = L'appareil sélectionné « { $text } » n'est plus disponible ; l'appareil par défaut a été sélectionné.

### Menus

menu-file = Fichier
menu-file-new = Nouvelle capture
menu-file-quit = Quitter
menu-capture = Capture
menu-capture-start = Démarrer
menu-capture-stop = Arrêter
menu-capture-restart-device = Redémarrer l'appareil
menu-capture-bootloader = Redémarrer en bootloader

### Barre d'outils, vue et boîtes de dialogue

toolbar-device = Appareil
toolbar-rate = Fréquence d'échantillonnage (Hz)
toolbar-rate-choice = Fréquence d'échantillonnage
toolbar-seed = Graine
toolbar-seed-random = aléatoire
toolbar-profile = Signal
profile-sparse = Clairsemé
profile-normal = Normal
profile-dense = Dense
toolbar-start = Démarrer
toolbar-stop = Arrêter
status-simulated = Simulé
status-incomplete = Incomplète
status-unsaved = Modifications non enregistrées
view-no-capture = Aucune capture pour le moment. Choisissez un appareil et appuyez sur Démarrer.
view-empty = La capture ne contient aucun échantillon.
view-cannot-draw = Les signaux ne peuvent pas être dessinés.
channel-default = Canal { $number }
dialog-unsaved-title = Capture non enregistrée
dialog-unsaved-body = La capture n'a pas été enregistrée. Que souhaitez-vous en faire ?
dialog-save = Enregistrer
dialog-discard = Abandonner
dialog-cancel = Annuler

### Affichage et canaux

menu-view = Affichage
menu-view-zoom-in = Zoom avant
menu-view-zoom-out = Zoom arrière
menu-view-fit = Tout afficher
menu-view-start = Aller au début
menu-view-end = Aller à la fin
menu-view-pan-left = Défiler à gauche
menu-view-pan-right = Défiler à droite
menu-view-page-left = Page précédente
menu-view-page-right = Page suivante
menu-capture-follow = Suivre les dernières données
channel-reset = Réinitialiser

### Messages

notice-dismiss = Fermer
notice-dismiss-all = Tout fermer
notice-log = Afficher le dossier du journal
notice-list = Messages
menu-help = Aide
menu-help-log = Afficher le dossier du journal

### Repères et mesures

menu-markers = Repères
menu-markers-place-single = Placer un repère simple
menu-markers-place-pair = Placer un repère double
menu-markers-cancel = Annuler le placement
menu-markers-previous = Repère précédent
menu-markers-next = Repère suivant
menu-markers-remove = Supprimer le repère
menu-markers-remove-all = Supprimer tous les repères
menu-markers-snap = Aimanter aux transitions
panel-markers = Repères
marker-single-add = Repère simple
marker-pair-add = Repère double
marker-empty = Aucun repère. Utilisez les boutons ci-dessus pour en ajouter un.
marker-placing-single = Cliquez pour placer le repère. Échap annule.
marker-placing-pair-first = Cliquez pour placer la première extrémité de la paire. Échap annule.
marker-placing-pair-second = Cliquez pour placer la seconde extrémité. Échap annule.
marker-zoomed-in = Zoom avant pour trouver le front. La vue revient après un clic ou avec Échap.
marker-limit = La limite de 64 repères est atteinte. Supprimez-en un pour en ajouter un autre.
marker-delete = Supprimer
marker-comment = Commentaire
marker-make-pair = Former une paire
marker-join-hint = Cliquez sur le second marqueur simple. Échap annule.
marker-action-origin = Compter le temps à partir d'ici
marker-action-origin-reset = Le temps compte depuis le début
marker-action-cut-left = Supprimer les données à gauche
marker-action-cut-right = Supprimer les données à droite
marker-action-scroll = Y aller
marker-action-trim = Réduire à la paire
marker-action-fit = Ajuster la vue à la paire
marker-action-split = Convertir en deux repères simples
marker-cut-disabled-running = Impossible pendant une acquisition
marker-cut-disabled-nothing = Il n'y a rien à supprimer de ce côté
marker-cut-pair-removed = Les paires dont il ne restait qu'une extrémité ont été supprimées : { $text }.
dialog-crop-title = Supprimer des données ?
dialog-crop-left-body = Les données à gauche du repère seront supprimées : { $lost } ({ $percent } %). Il restera { $kept }. Cette action est irréversible.
dialog-crop-right-body = Les données à droite du repère seront supprimées : { $lost } ({ $percent } %). Il restera { $kept }. Cette action est irréversible.
dialog-crop-pair-body = Seules les données entre les extrémités de la paire seront conservées : { $kept }. { $lost } ({ $percent } %) seront supprimées. Cette action est irréversible.
dialog-crop-confirm = Supprimer
dialog-cropping = Suppression des données
marker-colour-0 = Rouge
marker-colour-1 = Ambre
marker-colour-2 = Vert
marker-colour-3 = Sarcelle
marker-colour-4 = Bleu
marker-colour-5 = Violet
marker-colour-6 = Rose
marker-colour-7 = Gris
panel-capture = Capture
panel-view-capture = Réglages de la capture
panel-view-decoders = Décodeurs
panel-view-markers = Marqueurs et mesures
capture-length-seconds = Durée (secondes)
capture-length-samples = Échantillons : { $count }
capture-length-invalid = Saisissez la durée en secondes, par exemple 2,5.
capture-length-decimals = Au plus neuf chiffres après la virgule.
capture-length-small = C'est moins d'un échantillon à cette fréquence d'échantillonnage.
capture-length-large = C'est plus long qu'une capture ne peut l'être.
capture-length-no-rate = Réglez d'abord la fréquence d'échantillonnage.
# Le déclenchement d'une acquisition (affiché si l'appareil en a un).
trigger-type = Déclenchement
trigger-none = Sans déclenchement
trigger-level-low = Niveau : bas
trigger-level-high = Niveau : haut
trigger-rising = Front montant
trigger-falling = Front descendant
trigger-any = N'importe quel front
trigger-pattern = Motif
trigger-fast-pattern = Motif rapide
trigger-channel = Voie de déclenchement (première voie d'un motif)
trigger-pattern-levels = Niveaux du motif, première voie d'abord (0 ou 1)
capture-pre-trigger = Échantillons avant le déclenchement
capture-after-trigger = Échantillons après le déclenchement : { $count }
capture-bursts = Rafales (0 pour une acquisition)
# Chaînes de cartes qui acquièrent ensemble (affiché quand une carte chaînable est branchée).
chain-title = Chaînes de cartes
chain-help = Des cartes reliées entre elles acquièrent comme un seul appareil. Choisissez les cartes dans l'ordre ; la première est la maître.
chain-name = Nom de la chaîne
chain-create = Créer la chaîne
chain-remove = Retirer
chain-shift = Décalage
chain-master = Maître
chain-slave = Esclave
chain-no-boards = Aucune carte chaînable n'est libre.
chain-board-missing = (non branchée)
capture-channels = Canaux à enregistrer
capture-channels-all = Tous
capture-channels-none = Aucun
capture-no-channel = Aucun canal choisi : choisissez-en au moins un pour démarrer.

### Fichiers de session

menu-file-open = Ouvrir…
menu-file-save = Enregistrer
menu-file-save-as = Enregistrer sous…
menu-file-recent = Fichiers récents
menu-file-recent-empty = Aucun fichier récent
menu-file-recent-missing = (introuvable)
menu-file-recent-remove-missing = Retirer les entrées indisponibles
menu-file-recent-clear = Vider la liste
dialog-saving = Enregistrement de la session…
dialog-opening = Ouverture de la session…
dialog-open-title = Ouvrir une session
dialog-save-title = Enregistrer la session
dialog-filter-session = Sessions de l’analyseur logique
title-untitled = Capture sans titre
notice-view-newer = Les réglages d’affichage de ce fichier viennent d’une version plus récente et n’ont pas été appliqués. Ils restent tels quels dans le fichier.

### Paramètres et à propos

menu-file-settings = Paramètres…
menu-view-scale-up = Interface plus grande
menu-view-scale-down = Interface plus petite
menu-help-about = À propos
settings-title = Paramètres
settings-unsaved-title = Capture non enregistrée
settings-remember-choice = Mémoriser mon choix
settings-language = Langue
settings-theme = Thème
theme-system = Suivre le système
theme-light = Clair
theme-dark = Sombre
theme-high-contrast = Contraste élevé
settings-scale = Échelle de l’interface
settings-memory = Limite de mémoire d’une capture
settings-memory-unit = Mio
settings-memory-note = S’applique à la prochaine capture.
settings-memory-invalid = Saisissez un nombre entier de Mio entre { $min } et { $max }.
dialog-close = Fermer
about-title = À propos
about-body = Logic Analyzer { $version }. Enregistre des signaux numériques et les affiche sur une ligne de temps ; un appareil simulé permet de l’essayer sans matériel.
about-license = Logiciel libre sous licence GNU General Public License, version 3.

### Decoders: the sample decoder
decoder-sample-clocked-bits-name = Bits cadencés
decoder-sample-clocked-bits-description = Lit des bits sur une ligne de données aux fronts d'une ligne d'horloge et les regroupe en octets. Un décodeur d'exemple qui montre ce que les décodeurs peuvent faire.
decoder-sample-clocked-bits-signal-clock = Horloge
decoder-sample-clocked-bits-signal-data = Données
decoder-sample-clocked-bits-option-clock_edge = Front d'horloge
decoder-sample-clocked-bits-choice-clock_edge-rising = Montant
decoder-sample-clocked-bits-choice-clock_edge-falling = Descendant
decoder-sample-clocked-bits-option-bit_order = Ordre des bits
decoder-sample-clocked-bits-choice-bit_order-msb = Bit de poids fort d'abord
decoder-sample-clocked-bits-choice-bit_order-lsb = Bit de poids faible d'abord
decoder-sample-clocked-bits-option-bits = Bits par octet
decoder-sample-clocked-bits-option-idle = Limite d'inactivité (échantillons)
ann-sample-clocked-bits-byte = Octet { $value }
ann-sample-clocked-bits-bit = Bit { $value }
ann-sample-clocked-bits-incomplete = Octet incomplet : { $value } bits
ann-sample-clocked-bits-gap = Des échantillons ont été perdus ici

### Decoders: I2C
decoder-i2c-name = I2C
decoder-i2c-description = Lit le bus I2C (deux fils, horloge et données) : l'adresse, les octets et les acquittements de chaque transaction.
decoder-i2c-signal-scl = Horloge (SCL)
decoder-i2c-signal-sda = Données (SDA)
decoder-i2c-option-address = Notation de l'adresse
decoder-i2c-choice-address-addr7 = Adresse sur 7 bits (sans le bit de direction)
decoder-i2c-choice-address-addr8 = Valeur sur 8 bits (avec le bit de direction)
decoder-i2c-option-radix = Base numérique
decoder-i2c-choice-radix-hex = Hexadécimal
decoder-i2c-choice-radix-dec = Décimal
decoder-i2c-choice-radix-bin = Binaire
ann-i2c-start = Démarrage
ann-i2c-repeated-start = Redémarrage
ann-i2c-stop = Arrêt
ann-i2c-transaction-write = Écriture vers { $value }
ann-i2c-transaction-read = Lecture depuis { $value }
ann-i2c-transaction-write-long = Écriture vers { $value } : { $second }
ann-i2c-transaction-read-long = Lecture depuis { $value } : { $second }
ann-i2c-address-write = Adresse { $value } (écriture)
ann-i2c-address-read = Adresse { $value } (lecture)
ann-i2c-data = Données { $value }
ann-i2c-ack = ACK
ann-i2c-nak = NAK
ann-i2c-incomplete = Transaction incomplète
ann-i2c-error-byte-cut = Octet interrompu après { $value } bits
ann-i2c-error-address-nak = Adresse { $value } non acquittée
ann-i2c-error-data-nak = Données { $value } non acquittées
ann-i2c-error-empty = Transaction vide : aucune adresse
ann-i2c-error-gap = Des échantillons ont été perdus ici

### Decoders: menu, panel and the selected annotation
menu-decoders = Décodeurs
menu-decoders-add = Ajouter un décodeur
menu-decoders-previous = Annotation précédente
menu-decoders-next = Annotation suivante
menu-decoders-goto = Aller à l'annotation sélectionnée
panel-decoders = Décodeurs
decoders-add = Ajouter un décodeur
decoders-empty = Aucun décodeur pour l'instant. Ajoutez-en un pour voir ce que disent les signaux.
decoders-need-capture = Capturez ou ouvrez d'abord des signaux.
decoder-state-waiting = En attente
decoder-state-running = En cours
decoder-state-finished = Terminé
decoder-state-cancelled = Annulé
decoder-state-failed = Échec
decoder-needs-channels = Choisissez les canaux pour lancer le décodage.
decoder-channel-none = (aucun)
decoder-channel-unavailable = { $name } (non enregistré)
decoder-channels-unrecorded = Un canal de ce décodeur n'est pas enregistré, le décodeur ne s'exécute donc pas.
decoder-annotations = Annotations : { $number }
decoder-incomplete = Résultats incomplets

### Decoders: the simulation of signals that are valid for a decoder
decoder-generator-errors = Introduire volontairement des erreurs dans les signaux simulés
decoder-generator-rate = Environ un octet sur autant est interrompu

### Decoders: running, cancelling and the memory limit
decoder-cancel = Annuler
settings-decoder-memory = Limite de mémoire des résultats des décodeurs
settings-decoder-memory-note = Partagée par tous les décodeurs. S'applique au prochain passage.

### Decoders: stacking
decoder-source = Lit depuis
decoder-source-capture = La capture
dialog-remove-decoder-title = Supprimer le décodeur
dialog-remove-decoder-body = D'autres décodeurs lisent depuis celui-ci. Le supprimer les supprime aussi. La capture n'est pas affectée.
dialog-remove-decoder-confirm = Tout supprimer

### Decoders: saving and opening
notice-decoders-newer = Ce fichier contient des décodeurs d'une version plus récente de l'application. Ils sont conservés tels quels et ne sont pas affichés ; la capture s'ouvre normalement.
notice-layout-newer = Ce fichier contient des hauteurs de signaux enregistrées par une version plus récente de l'application. Elles sont conservées telles quelles et non affichées ; la capture s'ouvre avec des hauteurs automatiques.
settings-save-decoder-results = Enregistrer les résultats décodés avec la session
settings-save-decoder-results-note = Le fichier est plus volumineux. Sans cela, les décodeurs se relancent à l'ouverture du fichier.
