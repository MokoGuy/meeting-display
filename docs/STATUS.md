# État du projet — Meeting Display 0.5.2 (15 novembre 2026)

Ce document ne retient comme acquis que ce qui a été vérifié, avec la preuve. Les anciennes
mentions « validé » de septembre 2026 venaient d'agents, pas de l'utilisateur, et ne sont pas reprises.

Conclusion courte : l'application est installée, affiche le prochain rendez-vous réel sur la barre
et a traversé deux réunions de test accélérées. Elle n'a pas encore vécu une vraie journée de
réunions, et plusieurs fonctions ajoutées en fin de journée n'ont été vérifiées qu'en aperçu.

## Contexte

| Élément | Valeur |
|---|---|
| Barre | BB.1, firmware 1.4.1, API 27.15.0, en USB sur `http://10.0.4.20` |
| Poste | Windows 11, rustc/cargo 1.98, Node 24 |
| Source des réunions | lien ICS publié par Outlook, lu toutes les 5 minutes, fenêtre de 7 jours |

## Ce que fait l'application

Icône dans la zone de notification, panneau en agenda, six écrans sur la barre (repos, approche,
imminent, début, en réunion, dépassement), quatre sons, actions démarrer / ignorer / terminer,
option de démarrage automatique des réunions, mise à jour automatique. Le guide est dans
`app/README.md`.

## Testé

| Point | Comment | Résultat |
|---|---|---|
| Tests unitaires | `cargo test` | 31 réussis |
| Lecture du vrai calendrier | test `real_ics_file` sur un export réel de 476 Ko | horaires identiques à un parseur indépendant ; créneaux libres exclus ; réunions provisoires reconnues |
| Les six écrans | test `real_device_screens` sur la barre, captures | conformes à la démo validée à l'œil par l'utilisateur |
| Sons | test `sound_real_device` ; démo écoutée par l'utilisateur | entendus à volume 50 ; volume d'origine rétabli |
| Réunion complète, validation automatique | exécutable release + calendrier de test | imminent, LIVE à l'heure, dépassement ; sons de rappel, de début et carillon |
| Réunion complète, sans validation | idem | approche avec son, imminent, START à l'heure, retour à l'horloge à la fin, pas de carillon |
| Affichage par-dessus l'app Clock | mesure : dessin refusé à la priorité 50 et en dessous, accepté à 51 | écrans ambiants passés à 55 et 56 |
| Installation | installeur NSIS lancé en mode silencieux | app installée, raccourci menu Démarrer, réglages conservés |
| Build signé | `node tools/release.mjs` | installeur, signature et `latest.json` produits |
| Mise à jour automatique | version 0.3.1 installée, release 0.3.2 publiée, app relancée | l'app a installé la 0.3.2 seule, une minute après son lancement, puis s'est relancée |
| Release publique | téléchargement anonyme de `latest.json` et de l'installeur | HTTP 200 sans connexion GitHub |
| Historique sur disque | tests unitaires : enregistrement, durée prévue et réelle, une entrée par réunion, fenêtre de 30 jours, relecture après redémarrage | 5 tests réussis |
| Réunion lancée en avance | test unitaire | dure sa durée prévue à partir du lancement |
| App embarquée : molette et boutons | app installée sur la barre, calendrier de test, touches simulées par l'API | défilement des réunions, retour par OK, Start démarre puis termine, Back ignore, Start reprend ; chaque action reçue par l'app Windows et inscrite dans l'historique |
| App embarquée : transitions animées | app installée sur la barre, agenda fictif, touches simulées, captures d'écran en rafale | molette : le rideau noir couvre l'écran pendant que la liste change ; action : la flèche orange amène le mot puis découvre l'écran suivant, sans trou d'affichage. La fluidité elle-même (60 images par seconde) n'est pas mesurable par capture : jugée à l'œil sur la barre |
| Lancement de l'app embarquée | démarrage de l'app Windows | l'app embarquée est lancée et reçoit l'agenda |
| Calendrier de test | `node tools/test-calendar.mjs --scenario day` relu par le code de l'app | 6 réunions gardées, 3 écartées comme prévu |

## Vérifié seulement en aperçu navigateur (données fictives)

- Version 0.4.0 : navigation par jour (flèches, bouton Aujourd'hui, Échap), lignes d'historique avec durée prévue et durée réelle, total d'un jour passé, jour vide.
- Réunions réalisées ou ignorées qui restent à leur heure, icône Reprendre au survol, bouton de thème, bouton Ignorer en icône.

- Le panneau en agenda et ses états : à venir, c'est l'heure, en réunion, sans calendrier.
- Les composants du design system : interrupteur, champ d'heure au clavier, volet Avancé.

## Pas encore testé

| Point | Pourquoi |
|---|---|
| Le panneau dans la vraie fenêtre de l'app | jamais ouvert ni cliqué depuis l'app installée |
| Boutons Démarrer, Ignorer, Terminer | le code est simple et compilé, mais aucun clic réel n'a été fait |
| Démarrer une réunion en avance | essayé par l'utilisateur avant le correctif de durée ; pas rejoué depuis |
| Historique dans l'app installée | le fichier n'a encore reçu aucune vraie réunion |
| Workflow GitHub Actions | fourni comme modèle (`tools/release-workflow.yml`), jamais exécuté : le jeton GitHub du poste n'a pas le droit `workflow` |
| Lancement au démarrage de Windows | interrupteur présent, jamais activé |
| Heures de silence et mode discret en réel | tests unitaires seulement |
| Session BUSY active sur la barre | comportement de nos écrans non mesuré sur le firmware 1.4.1 |
| Une vraie journée de réunions | seules des réunions de test accélérées ont été traversées |

Pour combler ces trous sans attendre une vraie réunion : `node tools/test-calendar.mjs --use`
pointe l'app installée sur un calendrier de test, puis remet les réglages d'origine.

## Limites connues

- **La molette et les boutons demandent l'app embarquée** (`bar-app/`), parce que l'API de la barre
  ne signale pas les appuis à un programme sur PC. Elle remplace l'horloge tant qu'elle est ouverte.
- **L'app embarquée n'est pas installée par l'app Windows** : il faut la compiler et l'installer à la
  main, depuis PowerShell.
- **Les touches ont été simulées**, pas pressées : la vraie molette et les vrais boutons restent à essayer.
- **Pas de lien de visioconférence ni de participants.** Le calendrier publié par Outlook ne
  contient ni description ni participants : pas de bouton « rejoindre », et le chiffre de l'icône
  est le jour du mois de la réunion, pas le nombre d'invités.
- **Les réunions refusées** ne sont écartées que parce qu'Outlook les publie comme créneaux libres.
- **« En réunion » est dessiné par l'app** : la barre ne passe pas en état BUSY.
- **Les touches `busy`, `custom`, `off`, `apps`** de l'API déplacent le sélecteur de mode de la
  barre ; `off` éteint l'écran avant.

## Publication

- Le dépôt public est `MokoGuy/meeting-display`. Il a été créé avec un historique neuf (un commit
  de départ) après une revue de l'ancien historique, resté dans un dépôt privé d'archive.
- Mise à jour automatique : `plugins.updater` de `tauri.conf.json` pointe sur le fichier
  `latest.json` de la dernière release de ce dépôt.
- Juste après la publication d'une release, GitHub peut servir l'ancien `latest.json` pendant
  quelques minutes : un premier contrôle fait trop tôt a répondu « à jour ».
- La clé privée de signature est hors dépôt (`~/.tauri/meeting-display.key`). Elle doit être
  conservée : sans elle, plus aucune mise à jour ne peut être signée.
