### status: draft
### Brouillon traduit automatiquement ; doit être relu par une personne de langue maternelle française.

### Application

app-title = Logic Analyzer

### État et résultats de la capture

status-idle = Prêt
status-capturing = Capture en cours
status-saving = Enregistrement
status-loading = Chargement
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

### Barre d'outils, vue et boîtes de dialogue

toolbar-device = Appareil
toolbar-rate = Fréquence d'échantillonnage (Hz)
toolbar-length = Durée (échantillons)
toolbar-seed = Graine
toolbar-seed-random = aléatoire
toolbar-profile = Signal
profile-sparse = Clairsemé
profile-normal = Normal
profile-dense = Dense
toolbar-realtime = Temps réel
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
panel-channels = Canaux
channel-move-up = Monter
channel-move-down = Descendre
channel-colour = Couleur
channel-reset = Réinitialiser les canaux

### Messages

notice-dismiss = Fermer
notice-dismiss-all = Tout fermer
notice-log = Afficher le dossier du journal
notice-list = Messages
menu-help = Aide
menu-help-log = Afficher le dossier du journal

### Repères et mesures

menu-markers = Repères
menu-markers-place = Placer un repère
menu-markers-previous = Repère précédent
menu-markers-next = Repère suivant
menu-markers-remove = Supprimer le repère
menu-markers-remove-all = Supprimer tous les repères
menu-markers-snap = Aimanter aux transitions
panel-markers = Repères
marker-default = Repère { $number }
marker-go = Aller au repère
marker-remove = Supprimer
marker-measure-first = Premier repère de la mesure
marker-measure-second = Second repère de la mesure
measure-distance = Écart
measure-frequency = Fréquence
measure-pick = Choisissez deux repères pour mesurer le temps qui les sépare.

### Fichiers de session

menu-file-open = Ouvrir…
menu-file-save = Enregistrer
menu-file-save-as = Enregistrer sous…
menu-file-recent = Fichiers récents
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
decoder-lane-show = Afficher la piste
decoder-remove = Supprimer le décodeur
decoder-annotations = Annotations : { $number }
decoder-incomplete = Résultats incomplets
decoder-selected = Annotation sélectionnée
decoder-detail-length = Durée : { $value }
decoder-detail-error = Signale un problème dans le signal
decoder-detail-position = Position : de { $first } à { $second }

### Decoders: the simulation of signals that are valid for a decoder
decoder-generator-errors = Introduire volontairement des erreurs dans les signaux simulés
decoder-generator-rate = Environ un octet sur autant est interrompu

### Decoders: running, cancelling and the memory limit
decoder-run = Relancer
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
settings-save-decoder-results = Enregistrer les résultats décodés avec la session
settings-save-decoder-results-note = Le fichier est plus volumineux. Sans cela, les décodeurs se relancent à l'ouverture du fichier.
