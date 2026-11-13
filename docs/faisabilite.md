# Faisabilité

> Extrait de `idees.md` (parties A1 à A3) le 2026-11-13. Texte recopié mot pour mot.

> Note : A2 parle de C#/P-Invoke/IToolAdapter (ère C#) : à lire comme un binding Rust.

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

