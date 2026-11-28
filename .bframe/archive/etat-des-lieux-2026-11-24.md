---
titre: État des lieux du 24 nov. 2026
description: Ce qui marche vraiment pour Skyrim, Minecraft et le socle multi-jeux, preuves à l'appui.
date: 2026-11-24
---

# État des lieux du 24 nov. 2026

Trois analyses (code, journaux SKSE, web) faites en lecture seule. Builds : `cargo check` OK, `cargo test` 144 réussis / 0 échoué, `npm run build` OK.

## Skyrim

| Fonction | État | Ce qui manque |
| --- | --- | --- |
| Installer un mod (navigateur intégré, FOMOD) | partiel | Pas d'installation depuis un fichier du disque ; archives avec un dossier `Data/` probablement posées un niveau trop bas (`Data/Data/…`, `install_archived_mod` dans `crates/modlab-ui/src/lib.rs` n'utilise pas `data_dir`) |
| Activer / désactiver un mod | marche | — |
| Ordre des mods | absent | Réordonner + vérifier le sens de priorité de MO2 (peut-être inversé à l'import) |
| Ordre et activation des plugins (`plugins.txt`) | absent | ModLab n'écrit jamais `plugins.txt` : **les ESP des mods ne sont pas chargés en jeu** |
| SKSE | partiel | SKSE se charge (journal du 22/09), mais **0 plugin SKSE trouvé**. Détection par le nom du mod, pas par les fichiers ; mods au nom « skse » copiés au mauvais endroit ; retombe sans prévenir sur `SkyrimSE.exe` |
| Conflits de fichiers | marche (lecture) | Sens de priorité à confirmer |
| Sauvegardes | lecture seule | Copie de sécurité, restauration, lien avec l'instance |
| Annuler / instantanés | partiel | Ne restaure que la base, jamais les fichiers |
| Corbeille | partiel | La désinstallation supprime vraiment le dossier (`remove_dir_all`) : contraire à la règle « rien de définitif » |
| Cycle plan → approbation → application → vérification | 1 seule action sur ~10 | Toutes les autres mutations n'ont qu'un journal |

Données : `F:\ModLab` n'existe pas sur cette machine ; l'app retombe sur `.modlab-dev` (base vide de mods).

## Minecraft

| Besoin | État |
| --- | --- |
| Créer une instance | coquille vide (un nom en base, aucun dossier, ni version ni loader) |
| Vanilla / Fabric / Forge + version | absent |
| Mods déposés à la main | import ponctuel seulement, pas de synchronisation |
| Navigateur → mod installé | non relié (le jar passerait par le chemin Skyrim) |
| Activer / désactiver | **la case ment** : change la base, ne renomme jamais le `.jar` |
| Désinstaller | absent |
| Lancer | absent |

## Socle multi-jeux

- Le Core est propre (aucun code de jeu).
- Le contrat `GameAdapter` ne couvre que détection, onglets et chemins. Installer, activer (effet sur les fichiers), désinstaller, lancer, importer : hors contrat.
- Du code Skyrim est en dur dans l'hôte Tauri (`crates/modlab-ui/src/lib.rs`) et dans l'UI (`App.tsx`, tableau de bord, sauvegardes) ; liste des jeux en dur.

## Connexion officielle Minecraft (web)

- Un lanceur propre exige une application Azure **approuvée par Microsoft/Mojang** (formulaire, délai inconnu ; sinon erreur 403 « Invalid app registration »).
- Voie sans approbation : écrire une installation dans `launcher_profiles.json` du launcher officiel, pointée sur le dossier de l'instance ; l'humain clique « Jouer » dans le launcher officiel. Pas d'option pour choisir le profil en ligne de commande. La version Microsoft Store complique tout.
- Versions : manifeste Mojang (vanilla), `meta.fabricmc.net` (Fabric), installateur Forge `--installClient` téléchargé à la source.
- Modrinth : API libre (User-Agent obligatoire, 300 req/min) ; **afficher modrinth.com dans l'app est interdit** par ses conditions. CurseForge : clé d'API à demander, certains mods refusent la distribution tierce.
