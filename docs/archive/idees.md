# ModLab — Idées

> Fichier vivant. Les idées validées migrent vers ClickUp (liste « Idées ») puis vers la roadmap. Le parking d'idées d'origine est en §14 de `00-cadrage-technique.md` ; les superpouvoirs en §8.

## A. Idées issues de la demande du 14 septembre — faisabilité

### A1. Indicateurs SKSE / ENB / ReShade / Frame Generation / DLSS / FSR / XeSS

**Réalisable : oui, sans magie — par détection de signatures.** Rien n'est affiché si rien n'est détecté.

| Quoi | Comment on le détecte | Ce qu'on affiche |
|---|---|---|
| **SKSE** | `skse64_loader.exe` + `skse64_1_6_1170.dll` à la racine ; version lue dans le nom/`skse64_steam_loader.dll` ; Address Library dans `Data/SKSE/Plugins/versionlib-*.bin` | `SKSE 2.2.6 ✓` · alerte si version ≠ exe du jeu |
| **ENB** | `d3d11.dll` + `d3dcompiler_46e.dll` + `enbseries/` + `enblocal.ini` / `enbseries.ini` à la racine ; nom du preset lu dans `enbseries.ini` ou le dossier | `ENB · Rudy ✓` |
| **ReShade** | `dxgi.dll` (ou `d3d11.dll` selon l'install) + `ReShade.ini` + `reshade-shaders/` ; preset actif dans `ReShade.ini` → `PresetPath` | `ReShade · preset X ✓` |
| **Upscaler / FG (PureDark)** | DLL SKSE connues dans `Data/SKSE/Plugins/` : `SkyrimUpscaler.dll` (DLSS/FSR2/XeSS/DLAA), `PDPerfPlugin.dll` + `nvngx_dlssg.dll` (DLSS 3 Frame Generation), variantes FSR3 FG ; réglages lus dans `SkyrimUpscaler.ini` (`mUpscaleType`, `mFrameGeneration`, qualité) | `DLSS · Qualité` · `FG DLSS 3 ✓` / `FG FSR 3 ✓` |
| **Community Shaders** | `Data/SKSE/Plugins/CommunityShaders.dll` + `Data/Shaders/` | `Community Shaders ✓` (incompatible ENB → avertissement) |
| **OptiScaler / DLSS Enabler** (FG « spoofé » pour RTX 20/30, AMD) | DLL à la racine (`nvngx.dll`, `dlss-enabler*.dll`, `OptiScaler.ini`) | `FG via OptiScaler (FSR 3) ✓` |
| **Carte graphique** | WMI / DXGI (nom, VRAM, génération) | Cohérence : « DLSS FG demande une RTX 40 ; sur RTX 30 → FSR 3 FG » |

**Méthode** : un **catalogue de signatures** (YAML versionné dans le repo) : `capability → { fichiers, DLL, clés INI, mod Nexus ids }`. Le moteur scanne la racine du jeu + `Data/SKSE/Plugins` + les INI, produit des `Capability` (présente / active / version / preset / réglages) et la santé croise avec le GPU. Le catalogue se complète au fil de l'eau (c'est exactement le genre de tâche qu'un agent peut maintenir : « recense les mods qui apportent DLSS/FG sur Skyrim et leurs fichiers »).

**Limites** : les mods PureDark sont distribués via Patreon (pas de métadonnées Nexus fiables) → détection par fichiers uniquement. Un DLL renommé ne sera pas reconnu (on affiche alors « DLL SKSE inconnue : X » dans Santé plutôt que de se tromper).

### A2. LOOT intégré dans ModLab

Ton installation : `F:\_Modding\Skyrim Modding\Loot` → `LOOT.exe`, `libloot.dll`, Qt 6, `resources/`, `docs/`.

| Niveau | Comment | Coût | Ce qu'on gagne |
|---|---|---|---|
| **1. Piloté** | Lancer `LOOT.exe --game="Skyrim Special Edition" --auto-sort` dans l'environnement du loadout (VFS), attendre la fin, relire `loadorder.txt`/`plugins.txt`, capturer les messages depuis les rapports | faible (phase 1) | tri fiable sans réécrire quoi que ce soit ; LOOT reste à jour tout seul |
| **2. En process** | `libloot.dll` est écrit en Rust avec une API C++ ; on écrit un **shim C** de ~200 lignes (`loot_create_game`, `loot_load_lists`, `loot_sort_plugins`, `loot_get_plugin_messages`) chargé par P/Invoke depuis C# | moyen (phase 4, spike 1–2 j) | tri + métadonnées + messages **dans** l'onglet Plugins, sans fenêtre LOOT ; on peut trier une branche jetable sans toucher au jeu |
| **3. Métadonnées seules** | Parser la masterlist YAML (`resources/masterlist.yaml` / dépôt Git de LOOT) | faible | dirty edits, incompatibilités, prérequis affichés au survol, même sans trier |

**Recommandation** : 1 tout de suite, 3 en même temps (gratuit), 2 quand le moteur est stable. Le tout derrière `IToolAdapter` pour que « Trier avec LOOT » ne change pas d'apparence quel que soit le niveau.

### A3. Installation manuelle intelligente (« je donne l'archive, ça va au bon endroit »)

**Pourquoi ce n'est pas déjà standard ?** MO2 ne virtualise que `Data/` : tout ce qui doit vivre à la **racine du jeu** (SKSE, ENB, ReShade, `d3d11.dll`, OptiScaler) sort de son modèle — d'où le plugin communautaire *Root Builder* et les guides « copiez ces fichiers à la main ». Vortex sait le faire pour certains jeux via des extensions, mais pas de façon uniforme. Personne n'a voulu porter la responsabilité d'écrire dans le dossier du jeu à la place de l'utilisateur.

**ModLab** : `IInstaller` analyse l'archive et produit un **plan de placement visuel** avant d'écrire quoi que ce soit :

```
skse64_2_02_06.7z
├─ skse64_loader.exe          → racine du jeu          (backend racine, tracé)
├─ skse64_1_6_1170.dll        → racine du jeu
├─ skse64_steam_loader.dll    → racine du jeu
└─ Data/Scripts/*.pex         → mods/SKSE64/Scripts    (backend VFS)
```

Détection : fichiers racine connus (`*_loader.exe`, `d3d11.dll`, `dxgi.dll`, `enbseries/`), dossier `Data/` ou ses sous-dossiers typiques (`meshes/`, `textures/`, `scripts/`, `SKSE/`), FOMOD (`fomod/ModuleConfig.xml`), BAIN (`00 Core/`, `01 Option/`), archive « nue » (textures en vrac → on propose la racine `Data/`). Tout ce qui va à la racine est enregistré dans la base (provenance + hash) → désinstallation propre, détection d'écrasement (ex. deux mods qui posent `d3d11.dll`). **Réalisable en phase 1.**

## B. 20 nouvelles idées (14 septembre)

1. **Mode Théâtre** — quand on clique Jouer, ModLab se réduit à un petit overlay au-dessus du jeu (FPS, révision, chrono, bouton « marquer un bug ») ; l'app se redéploie à la sortie.
2. **Marque-page en jeu** — une touche globale pendant la partie : capture d'écran + position (cellule, coordonnées) + révision → une carte « à investiguer » dans Santé, pour ne plus oublier le bug visuel croisé à 23 h.
3. **Profil matériel** — détection GPU/CPU/RAM/VRAM et presets cohérents (ENB, FG, ombres, grass) ; Santé signale « ce mod dépasse ta VRAM » avant de lancer.
4. **Sandbox en un clic** — « Essayer ce mod » : branche jetable, installation, lancement, puis à la sortie du jeu : *Garder* (commit) ou *Jeter* (rollback).
5. **Coût FPS par mod** — bench différentiel automatique (session A/B) qui attribue un coût estimé aux mods lourds (grass, ENB, LOD, textures 4K) et le montre dans la table.
6. **Épingles de version avec raison** — « ne pas mettre à jour SkyUI tant que Dear Diary n'est pas compatible » ; le digest des mises à jour respecte les épingles et rappelle pourquoi.
7. **Résolveur visuel de textures** — pour un conflit de texture/mesh, comparer gagnant/perdant côte à côte (zoom, curseur), choisir par fichier, règle persistante par branche.
8. **Sauvegardes liées aux révisions** — chaque `.ess` connaît la révision qui l'a créée ; charger une save avec une autre branche déclenche une alerte et propose « voyager » vers la bonne révision.
9. **Mise à jour sûre** — lit le changelog Nexus, repère « nouvelle partie requise » / « clean save », propose une branche de test et un plan réversible.
10. **Nettoyage d'espace avec preuves** — archives jamais réutilisées, sorties d'outils périmées, blobs orphelins : « libérer 23 Go », chaque élément justifié, corbeille interne.
11. **Tableau de bord « ma vie de moddeur »** — heures par branche, crashs / 10 h, mods installés par mois, catégories préférées, temps passé dans les outils.
12. **Collection « as code » partageable** — exporter une branche en fichier texte (liste, choix FOMOD, hashes, sources) qu'un ami importe sans redistribuer un seul asset (Wabbajack-lite).
13. **Mode « debugger de problèmes »** — parcourir les problèmes un par un comme des breakpoints : preuve, proposition, *Appliquer / Ignorer / Noter*, avec avancement.
14. **Rappels de maintenance** — « 12 mods ajoutés depuis le dernier DynDOLOD », « 30 jours sans LOOT », « Nemesis à régénérer (nouveau mod d'animation) » — cartes dans Santé, jamais intrusives.
15. **Journal de session lisible** — après chaque partie : durée, zones visitées, avertissements Papyrus, crash éventuel, en une page ; « demander à Claude » pré-rempli avec ce contexte.
16. **Minecraft : une branche par serveur** — mods requis synchronisés depuis le modpack (Modrinth / CurseForge), Java, RAM et arguments JVM par branche, détection de la version du serveur.
17. **Migration guidée** — import MO2 / Vortex / Prism Launcher → carte « reconnu / à vérifier / ignoré », jamais destructif, avec possibilité de continuer à utiliser l'ancien outil en parallèle.
18. **Réglages transversaux** — un curseur ModLab (ex. « qualité des ombres ») qui écrit dans plusieurs INI/JSON de plusieurs mods à la fois, tracé et réversible, avec profils (perf / qualité).
19. **Assistant de premier lancement** — après Jouer, ModLab lit le crash log / temps de chargement et propose l'étape suivante (comme un tuto adaptatif), désactivable dès qu'on n'en veut plus.
20. **Surveillance du dossier Téléchargements** — une archive téléchargée à la main (Nexus, Patreon, Discord) est détectée, identifiée (hash → Nexus si possible), et proposée à l'installation avec le plan de placement.

## C. En attente de tri

- (vide — ajouter ici au fil des sessions)
