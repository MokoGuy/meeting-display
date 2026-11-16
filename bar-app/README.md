# Meeting Display — app embarquée

La partie de Meeting Display qui tourne **sur la BUSY Bar elle-même**. Elle dessine les écrans
de réunion et lit la molette et les boutons, ce qu'un programme sur PC ne peut pas faire.

L'app Windows reste le cerveau : elle lit le calendrier et pousse un petit agenda à la barre.
Sans l'app embarquée, tout fonctionne comme avant, sans molette.

## Gestes

| Geste | Effet |
|---|---|
| Molette | un cran = la réunion suivante ou précédente, sur le même écran que d'habitude ; aujourd'hui et les jours suivants |
| OK | revient à la réunion du moment |
| Start | démarre la réunion choisie (à l'heure ou en avance) ; en réunion, la termine ; sur une réunion réalisée ou ignorée, la reprend |
| Back, appui court | ignore la réunion choisie |
| Back, appui long | quitte l'app (réservé par la barre) |
| 10 secondes sans geste | retour à la réunion du moment |

Sur chaque réunion, l'icône donne le jour du mois et la ligne du bas l'heure (avec le jour de la
semaine si ce n'est pas aujourd'hui). À droite : le temps restant pour une réunion d'aujourd'hui
(« 25 min », « dans 9h27 », « dans 12h » : le mot « dans » évite de lire une heure),
« faite » en vert, « ignoree » en gris. Le trait orange sur le bord droit situe la réunion parmi
les autres. Une réunion ignorée ou réalisée par erreur se reprend ainsi : molette jusqu'à elle,
puis Start.

Toutes les trois secondes, la ligne du bas bascule entre **quand** (l'heure, et le temps restant
ou l'état) et **combien de temps** : « duree 1h00 » pour une réunion à venir ou ignorée,
« prevu 15 » et « reel 14 » pour une réunion réalisée. Il en va de même sur l'écran de repos, pour
le prochain rendez-vous. Les écrans actifs (compte à rebours, START, LIVE, dépassement) ne
basculent pas. Il faut Meeting Display 0.5.9 ou plus récent sur le PC pour les durées.

Il faut Meeting Display 0.5.3 ou plus récent sur le PC : avant, seules les réunions du jour
étaient envoyées à la barre.

Une action faite sur la barre arrive à l'app Windows en 5 secondes au plus ; l'écran affiche
tout de suite « Demarree », « Terminee », « Ignoree » ou « Reprise ».

## Transitions

La barre joue elle-même un fichier d'animation à 60 images par seconde ; une app qui redessine
chaque image n'en tient qu'une douzaine. Les transitions sont donc des fichiers d'animation posés
par-dessus l'écran, dans `src/animations/` :

| Fichier | Quand | Ce qu'on voit |
|---|---|---|
| `reveal_up`, `reveal_down` | molette | la réunion suivante arrive sous un rideau noir qui se lève dans le sens du défilement ; molette tournée vite (deux crans en moins de 0,45 s) : pas de rideau, la carte change aussitôt |
| `in_<mot>` | Start ou Back | une flèche orange traverse de gauche à droite ; derrière elle le mot est déjà là |
| `out_<mot>` | 1,5 s plus tard | la même flèche découvre l'écran suivant |

Les mots (« Demarree », « Terminee », « Ignoree », « Reprise ») sont dessinés dans les fichiers,
à partir d'une capture de la barre pour avoir sa police. Pour changer un mot, la couleur ou la
vitesse : modifier `scripts/make-anims.mjs`, puis, barre branchée, `node scripts/make-anims.mjs`
et réinstaller l'app.

## Installer sur la barre

**L'app Windows transporte l'app embarquée et l'installe sur un clic.** Elle compare la version
présente sur la barre avec celle qu'elle transporte. Si la barre n'a pas l'app, ou en a une plus
ancienne, le panneau affiche en haut de l'agenda « App de la barre à installer » (ou « à mettre à
jour ») avec un bouton **Mettre à jour**. Le clic l'installe puis la lance ; quelques secondes.
Rien n'est installé sans ce clic, et une version plus récente que la sienne n'est jamais remplacée.

Quand c'est l'app Windows qui a une nouvelle version, le même bouton fait les deux : l'app Windows
se met à jour et redémarre, puis la nouvelle version installe son app embarquée sur la barre.

Pour retirer l'app embarquée : `npm run app:uninstall` dans ce dossier. Le panneau proposera
alors de la réinstaller.

### Pour développer

Barre branchée en USB, depuis **PowerShell** (pas Git Bash : l'étape d'archivage y échoue) :

```powershell
cd bar-app
npm install
npm run build
npm run app:install
```

Pour qu'une version de l'app Windows transporte ce que tu viens de construire :
`node tools/bundle-bar-app.mjs` à la racine, qui copie le paquet dans
`app/src-tauri/assets/bar-app.tgz` et vérifie que sa version est celle de l'app Windows.

Une seule app tourne à la fois sur la barre : tant que celle-ci est ouverte, elle remplace
l'horloge. Si tu ouvres une autre app ou changes de mode, l'app Windows reprend l'affichage
(sans molette) au bout d'une minute.

## Comment les deux apps se parlent

Par trois petits fichiers dans le dossier de données de l'app embarquée, via l'API de stockage
de la barre. Vu du PC : `/ext/apps_data/js_apps/meeting-display/` ; vu de l'app : `/ext/data/`.

| Fichier | Écrit par | Contenu |
|---|---|---|
| `agenda.json` | l'app Windows, quand il change | la réunion du moment et les réunions du jour |
| `actions.txt` | l'app embarquée, à chaque geste | une ligne `<numéro>\|<action>\|<id de réunion>` |
| `alive.txt` | l'app embarquée, toutes les 20 s | un battement : tant qu'il bouge, l'app Windows ne dessine pas |

## Outil sous Windows

`@busy-app/cli` 0.6.0 a deux défauts sous Windows. Le premier (chemins de polices) est corrigé
automatiquement après `npm install` par `scripts/patch-cli.cjs`. Le second (archivage) impose
de lancer le build depuis PowerShell.
