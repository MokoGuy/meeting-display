# Meeting Display — app embarquée

La partie de Meeting Display qui tourne **sur la BUSY Bar elle-même**. Elle dessine les écrans
de réunion et lit la molette et les boutons, ce qu'un programme sur PC ne peut pas faire.

L'app Windows reste le cerveau : elle lit le calendrier et pousse un petit agenda à la barre.
Sans l'app embarquée, tout fonctionne comme avant, sans molette.

## Gestes

| Geste | Effet |
|---|---|
| Molette | fait défiler les réunions du jour ; la réunion choisie est en haut, la suivante dessous |
| OK | revient à la réunion du moment |
| Start | démarre la réunion choisie (à l'heure ou en avance) ; en réunion, la termine ; sur une réunion réalisée ou ignorée, la reprend |
| Back, appui court | ignore la réunion choisie |
| Back, appui long | quitte l'app (réservé par la barre) |
| 10 secondes sans geste | la liste se referme |

Dans la liste, la barre devant chaque ligne suit le code du panneau : orange pour la réunion
choisie, grise pour une réunion à venir ou ignorée, verte pour une réunion réalisée.

Une action faite sur la barre arrive à l'app Windows en 5 secondes au plus ; l'écran affiche
tout de suite « Demarree », « Terminee », « Ignoree » ou « Reprise ».

## Installer sur la barre

Barre branchée en USB, depuis **PowerShell** (pas Git Bash : l'étape d'archivage y échoue) :

```powershell
cd bar-app
npm install
npm run build
npm run app:install
```

L'app Windows lance ensuite l'app embarquée à chaque démarrage. Pour la retirer :
`npm run app:uninstall` ; l'app Windows reprend alors l'affichage elle-même.

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
