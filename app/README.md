# Meeting Display

Application Windows qui tient le prochain rendez-vous de ton agenda affiché sur une
[BUSY Bar](https://busy.app) branchée en USB, et accompagne chaque réunion : approche,
compte à rebours, début, temps restant, dépassement.

L'état vérifié du projet est dans [`docs/STATUS.md`](../docs/STATUS.md).

## Installation

1. Lancer `Meeting Display_<version>_x64-setup.exe` (dans `src-tauri/target/release/bundle/nsis/` après un build).
2. Ouvrir « Meeting Display » depuis le menu Démarrer : une icône apparaît dans la zone de notification.
3. Clic sur l'icône, roue dentée **Réglages** : coller le lien ICS du calendrier, **Enregistrer**.
4. Activer **Lancer au démarrage de Windows**.

Le lien ICS s'obtient dans Outlook sur le web : Paramètres, Calendrier, Calendriers partagés,
Publier un calendrier, « Tous les détails », lien ICS. Il n'est stocké que dans
`%APPDATA%\com.mokoguy.meeting-display\settings.json`.

## Ce que fait la barre

| Moment | Écran | Son |
|---|---|---|
| Plus de 15 min avant | heure de début et temps restant | aucun |
| 15 min avant | temps restant en ambre | note douce |
| 5 min avant | compte à rebours ; LED la dernière minute | rappel |
| Heure de début | « START », LED rouge, notification Windows | début |
| Réunion validée | « LIVE » et temps restant | aucun |
| Fin prévue d'une réunion validée | « + mm:ss » en orange, 30 min au plus | carillon |

Le chiffre dans l'icône calendrier est le jour du mois de la réunion affichée, comme un calendrier de bureau.
L'écran arrière de la barre affiche l'état, le sujet complet et un détail.

## Le panneau

Un clic sur l'icône ouvre le panneau : la date du jour, la réunion du moment en première ligne,
puis le reste de la journée et les jours suivants.

Sur la réunion du moment : **Démarrer** la valide (à l'heure, ou en avance), l'icône **Ignorer** l'écarte,
**Terminer** la clôt. Une réunion terminée (« réalisée ») ou ignorée reste à son heure dans la liste,
grisée ; au survol de sa ligne, une icône **Reprendre** la remet en jeu, elle seule
(aujourd'hui ou un jour à venir ; les jours passés restent tels quels dans l'historique).
Clic droit sur l'icône puis **Démarrer la réunion** valide aussi. Le bouton physique de la barre
ne peut pas servir à valider (l'API de la barre ne signale pas les appuis). Avec l'option
**Passer en réunion automatiquement à l'heure**, la validation est faite d'office.

Ne sont pas affichées : les journées entières, les réunions annulées, et les créneaux marqués
« libre » (dont les réunions refusées, qu'Outlook publie ainsi). Les réunions provisoires sont
affichées sans LED.

### Historique

Chaque réunion réalisée ou ignorée est enregistrée sur le poste, 30 jours glissants, dans
`%APPDATA%com.mokoguy.meeting-displayhistory.json`. Rien n'est envoyé ailleurs.

Les flèches de l'en-tête (ou les flèches gauche et droite du clavier) font défiler les jours :
30 jours en arrière, 7 en avant. Le bouton **Aujourd'hui**, ou Échap, ramène au jour courant, et le
panneau se rouvre toujours sur aujourd'hui. Une réunion réalisée affiche sa durée prévue et sa durée
réelle ; un jour passé se termine par son total. Une réunion lancée en avance dure sa durée prévue à
partir du lancement.

Dans le pied du panneau, le bouton de thème alterne automatique (réglage de Windows), clair et sombre.

## Molette et boutons de la barre

Avec l'app embarquée (dossier `bar-app/`), que l'app Windows installe et met à jour sur la barre quand tu cliques sur **Mettre à jour**, la molette passe
d'une réunion à la suivante ou à la précédente (aujourd'hui et les jours suivants) et les boutons Start et Back démarrent, terminent, ignorent ou
reprennent une réunion directement sur la barre. Voir [`bar-app/README.md`](../bar-app/README.md).

## Paramètres

| Réglage | Effet |
|---|---|
| Lien du calendrier (ICS) | source des réunions, relue toutes les 5 minutes |
| Passer en réunion automatiquement à l'heure | saute l'écran « START » |
| Mode discret | n'affiche plus rien sur la barre et coupe les sons |
| Silence de … à … | même effet, sur une plage horaire |
| Adresse de la barre | `http://10.0.4.20` en USB |
| Lancer au démarrage de Windows | démarre l'app avec la session |

**Quelle version est installée ?** En bas des réglages, à gauche du bouton Enregistrer : la version
de l'app Windows (avec « à jour » ou la version disponible, une fois la première vérification
faite), puis celle de l'app embarquée sur la barre. Sur la barre, l'écran arrière affiche la version
de l'app embarquée en haut à droite quand aucune réunion n'approche.

**Vérifier tout de suite s'il y a une mise à jour** : l'icône aux deux flèches à droite de la ligne
de version, ou clic droit sur l'icône de la barre des tâches puis **Vérifier les mises à jour**
(le panneau s'ouvre avec la réponse). Cela ne fait que vérifier ; rien ne s'installe sans le
bouton **Mettre à jour**.

Les sons sont joués à un volume de 50 quand la barre est réglée plus bas, puis le volume
d'origine est rétabli.

## Tester sans attendre une réunion

`tools/test-calendar.mjs` sert un calendrier ICS de test sur cette machine :

```powershell
node tools/test-calendar.mjs --list                    # les scénarios
node tools/test-calendar.mjs --scenario quick --use    # pointe l'app installée dessus
```

Avec `--use`, les réglages sont sauvegardés, l'app relancée sur le calendrier de test, et tout est
remis en place à l'arrêt (Ctrl+C). Sans `--use`, le script affiche un lien
`http://127.0.0.1:8799/test.ics` à coller dans les réglages. Scénarios : `quick` (une réunion dans
6 minutes), `day` (journée chargée, provisoire, créneaux à ne pas afficher), `overrun`, `accents`,
`empty`.

## Publier une version

1. Changer la version dans `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` et `package.json`,
   ainsi que dans `bar-app/package.json` et `bar-app/src/appmeta/manifest.json` : les deux apps
   portent le même numéro.
2. Embarquer l'app de la barre : dans `bar-app`, depuis PowerShell, `npm run build` ; puis à la
   racine `node tools/bundle-bar-app.mjs`. La publication refuse de partir si le paquet embarqué
   n'est pas à la version de l'app Windows.
3. Depuis un poste qui a la clé de signature : `node tools/release.mjs --publish` (clé lue dans
   `~/.tauri/meeting-display.key`, GitHub CLI connecté). Le script construit l'installeur, le signe
   et crée la release.
4. Ou par GitHub Actions : copier `tools/release-workflow.yml` dans `.github/workflows/release.yml`
   (il faut un jeton GitHub avec le droit `workflow`), puis pousser un tag `v<version>`. Le secret
   `TAURI_SIGNING_PRIVATE_KEY` est déjà enregistré dans le dépôt.

Les apps installées cherchent une mise à jour vingt secondes après leur lancement puis toutes les
six heures, et n'installent rien d'elles-mêmes : le panneau affiche « Version x disponible » en haut
de l'agenda, avec un bouton **Mettre à jour**. Au clic, l'app se met à jour et redémarre, puis la
nouvelle version installe sur la barre l'app embarquée qu'elle transporte. L'adresse consultée est
dans `plugins.updater.endpoints` de `tauri.conf.json`.

## Design system

Le panneau suit le design system « Meeting Display » : aucun contrôle natif du navigateur,
tout est dessiné à partir de jetons. Les jetons sont dans `src/ui/tokens.css`, les styles dans
`src/ui/components.css`, les composants React dans `src/ui/components.tsx`.

Aperçu du panneau sans l'application : `npm run dev`, puis `http://localhost:1420/?phase=upcoming`
(aussi `armed`, `live`, `overrun`, `none`), avec des réunions fictives.

## Développement

Depuis `app/` :

```powershell
npm install
npm run tauri dev      # application avec rechargement à chaud
npm run tauri build    # exécutable et installeur NSIS
```

Depuis `app/src-tauri/` :

```powershell
cargo test                                                       # tests unitaires
cargo test real_device_screens -- --ignored --nocapture          # joue les six écrans sur la barre
cargo test sound_real_device -- --ignored --nocapture            # joue les sons sur la barre
$env:BUSYBAR_ICS_FILE="chemin\export.ics"; cargo test real_ics_file -- --ignored --nocapture
```

Les journaux sont dans `%LOCALAPPDATA%\com.mokoguy.meeting-display\logs\` (un fichier par jour).
