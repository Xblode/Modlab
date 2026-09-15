# MODLAB
## Document de cadrage produit, architecture et vision technique

> **Statut : document de réflexion / architecture initiale**
>
> Ce document rassemble les décisions, idées et hypothèses discutées pour le projet Modlab.
>
> **Important pour l'implémentation : rien dans ce document ne doit être considéré comme définitivement imposé.**
> Le rôle de l'agent de développement est également d'analyser la maquette, le code existant, les contraintes réelles et les besoins du produit afin de confirmer, modifier ou remplacer les choix proposés ici lorsqu'une meilleure solution existe.

---

# 1. Vision du projet

## Modlab — "The IDE for Game Modding"

Modlab est un gestionnaire/modding workspace nouvelle génération.

L'idée n'est pas simplement de créer un équivalent de Mod Organizer 2, Vortex ou CurseForge.

L'objectif est de créer une application qui soit à la fois :

- un mod manager ;
- un environnement de travail pour le modding ;
- un explorateur de mods ;
- un gestionnaire de téléchargements ;
- un gestionnaire d'archives ;
- un gestionnaire de profils ;
- un système de déploiement ;
- un système de diagnostic ;
- un environnement multi-jeux ;
- un terminal/outillage pour modding ;
- une interface avec les plateformes de mods ;
- un environnement permettant aux IA de comprendre et manipuler réellement un jeu moddé.

La philosophie générale est proche d'un **IDE pour le modding**, voire du concept :

> **"Cursor pour le game modding."**

L'application doit pouvoir être utilisée très simplement par un débutant, tout en permettant à un utilisateur expert d'avoir accès à quasiment tous les détails internes.

---

# 2. Problème que Modlab doit résoudre

Les gestionnaires actuels ont chacun leurs avantages.

### Mod Organizer 2

Très puissant et apprécié par les utilisateurs avancés.

Avantages :

- contrôle extrêmement précis ;
- séparation claire des mods ;
- gestion fine du load order ;
- virtualisation du système de fichiers ;
- excellent pour les grosses modlists.

Mais :

- interface intimidante ;
- beaucoup de concepts à comprendre ;
- beaucoup de choses restent manuelles ;
- l'utilisateur doit souvent comprendre lui-même pourquoi quelque chose ne fonctionne pas.

### Vortex

Très orienté automatisation et simplicité.

Avantages :

- intégration Nexus ;
- installation simple ;
- profils ;
- collections ;
- automatisation ;
- gestion des règles.

Mais une critique récurrente est que l'automatisation peut devenir opaque.

Lorsqu'un problème survient, l'utilisateur peut avoir du mal à comprendre :

- pourquoi un mod est chargé avant un autre ;
- pourquoi un fichier est utilisé ;
- pourquoi une règle existe ;
- pourquoi un déploiement échoue ;
- pourquoi un mod ne fonctionne plus.

### CurseForge

Très efficace pour Minecraft et d'autres jeux supportés.

Il est particulièrement intéressant comme référence pour :

- instances ;
- modpacks ;
- dépendances ;
- versions ;
- modloaders ;
- téléchargement intégré ;
- gestion simplifiée des environnements.

Mais son objectif principal reste la distribution et la gestion de contenu plutôt qu'un environnement général de travail sur le modding.

---

# 3. Positionnement de Modlab

Le positionnement recherché :

### MO2

> "Je contrôle précisément mon installation."

### Vortex

> "Le gestionnaire automatise mon installation."

### CurseForge

> "Je veux installer et maintenir facilement un environnement moddé."

### Modlab

> **"Je peux laisser Modlab automatiser le travail, mais je peux comprendre et contrôler absolument tout lorsque j'en ai besoin."**

L'application doit donc combiner :

**Automatisation + transparence + contrôle expert + IA.**

---

# 4. Principes fondamentaux

## 4.1 L'automatisation ne doit jamais devenir opaque

Lorsqu'une action automatique est réalisée, Modlab doit pouvoir expliquer :

- ce qui a été fait ;
- pourquoi ;
- quels fichiers ont été modifiés ;
- quelles dépendances ont été installées ;
- quels conflits ont été détectés ;
- pourquoi un mod est activé/désactivé ;
- pourquoi une installation est bloquée ;
- comment revenir en arrière.

---

## 4.2 Les actions doivent être réversibles

Autant que possible :

```text
Action
 ↓
Transaction
 ↓
Backup / état précédent
 ↓
Modification
 ↓
Validation
```

Si quelque chose échoue :

```text
Rollback
```

Le système doit éviter les modifications irréversibles inutiles.

---

## 4.3 Trois niveaux d'utilisation

### Débutant

Interface simple :

```text
[ Installer ]

[ Mettre à jour ]

[ Lancer ]

[ Réparer ]
```

Pas besoin de comprendre tous les détails.

### Avancé

Accès à :

- conflits ;
- dépendances ;
- fichiers ;
- load order ;
- profils ;
- logs ;
- déploiement.

### Expert

Accès à :

- système de fichiers ;
- architecture des mods ;
- graph de dépendances ;
- graph de conflits ;
- console ;
- logs détaillés ;
- outils externes ;
- API ;
- MCP ;
- commandes avancées.

---

# 5. Architecture générale proposée

Architecture initialement envisagée :

```text
                    MODLAB
                       │
              ┌────────┴────────┐
              │                 │
           React/TS          Rust Core
              │                 │
           Tauri IPC ───────────┘
                                │
        ┌───────────┬───────────┼────────────┐
        │           │           │            │
    Downloads    Archives    Deployment    Database
        │           │           │            │
        └───────────┴───────────┴────────────┘
                                │
                       Game Adapter Layer
                                │
             ┌──────────────────┼─────────────────┐
             │                  │                 │
          Skyrim            Minecraft         Witcher 3
          Adapter            Adapter            Adapter
             │                  │                 │
             │             Fabric/Forge       REDengine
             │             NeoForge/Quilt
             │
          SKSE/ESP/ESM/etc.
```

Cette architecture est une **proposition initiale**.

Claude Code doit l'évaluer avant de la figer.

---

# 6. Rust Core

Le Core devrait idéalement être indépendant de l'interface.

Responsabilités potentielles :

```text
Core
├── Games
├── Mods
├── Profiles
├── Downloads
├── Archives
├── Filesystem
├── Deployment
├── Dependencies
├── Conflicts
├── Load Order
├── Processes
├── Launching
├── Backups
├── Diagnostics
├── Database
├── Providers
├── Tools
├── AI integration
└── MCP
```

Le Core doit être la **source de vérité de Modlab**.

L'interface graphique ne doit pas contenir la logique métier critique.

---

# 7. Pourquoi Rust

Rust est envisagé pour le Core parce que Modlab est fondamentalement une application système :

- manipulation de fichiers ;
- archives ;
- gros volumes de données ;
- téléchargements ;
- processus ;
- lancement de jeux ;
- virtualisation/abstraction filesystem ;
- déploiement ;
- opérations concurrentes ;
- analyse ;
- base de données ;
- outils ;
- intégration IA/MCP.

Rust apporte :

- sécurité mémoire ;
- bonne gestion de la concurrence ;
- performances ;
- faible overhead ;
- bonne adéquation aux opérations système.

Important :

Rust n'est PAS choisi simplement parce que "Rust est plus rapide".

L'inférence IA dépendra principalement :

- du GPU ;
- du CPU ;
- du runtime d'inférence ;
- du modèle ;
- de la quantification ;
- de la VRAM/RAM.

Le choix Rust concerne surtout le **Core système**.

---

# 8. UI : React + TypeScript

L'interface envisagée utilise :

- React ;
- TypeScript ;
- Vite.

Pourquoi :

- très flexible ;
- adapté aux interfaces complexes ;
- excellent pour créer une interface type IDE ;
- possibilité d'avoir plusieurs panneaux ;
- composants réutilisables ;
- écosystème important.

L'interface doit être pensée davantage comme :

```text
IDE
```

que comme :

```text
simple bibliothèque de jeux
```

---

# 9. Tauri

Tauri est envisagé comme shell desktop.

Architecture :

```text
React
   ↕
Tauri IPC
   ↕
Rust Core
```

Sous Windows, Tauri utilise WebView2 pour afficher l'interface web.

Avantages recherchés :

- application desktop native ;
- frontend moderne ;
- Rust backend ;
- IPC ;
- permissions/capabilities ;
- faible overhead comparé à une application Electron équivalente ;
- intégration Windows.

---

# 10. WebView2

WebView2 permet notamment d'utiliser le moteur Chromium/Edge dans l'application.

Cela peut être intéressant pour :

- pages web intégrées ;
- Nexus ;
- CurseForge ;
- GitHub ;
- documentation ;
- navigateurs de mods ;
- authentification lorsque supportée ;
- affichage de contenu web.

Cependant, il faut vérifier pour chaque plateforme :

- API disponible ;
- authentification ;
- conditions d'utilisation ;
- restrictions ;
- téléchargement direct.

Ne pas supposer que l'intégration web autorise automatiquement toutes les opérations.

---

# 11. Architecture Game Adapter

C'est un élément central du projet.

Il doit y avoir un **Game Adapter par jeu**, mais chaque adapter ne doit pas réimplémenter tout Modlab.

Le Core fournit les primitives générales.

L'Adapter fournit les connaissances propres au jeu.

---

# 12. Responsabilités d'un Game Adapter

Un adapter devrait idéalement savoir :

## Identification

- détecter le jeu ;
- détecter son installation ;
- détecter les différentes éditions ;
- détecter la version ;
- détecter les composants importants.

## Structure

Décrire la structure du jeu :

```text
Game Root
Data
Mods
Plugins
Configs
Saves
...
```

selon le jeu.

## Modding

Décrire :

- types de mods ;
- formats ;
- load order ;
- modloaders ;
- plugins ;
- scripts ;
- dépendances ;
- outils.

## Installation

Déterminer comment installer correctement un mod.

## Désinstallation

Déterminer comment retirer proprement un mod.

## Diagnostic

Identifier :

- dépendances manquantes ;
- mauvaises versions ;
- incompatibilités ;
- fichiers incorrects ;
- conflits ;
- erreurs connues.

## Launch

Savoir comment lancer :

- le jeu ;
- un launcher ;
- un modloader ;
- des outils associés.

## Outils

Déclarer les outils spécifiques au jeu :

Exemple Skyrim :

```text
SKSE
xEdit
LOOT
DynDOLOD
Nemesis
Pandora
...
```

---

# 13. Exemple : Skyrim Adapter

Le Skyrim Adapter pourrait gérer :

```text
Skyrim SE / AE / VR
```

et comprendre notamment :

```text
Data
ESP
ESM
ESL
BSA
BA2
FOMOD
SKSE
Plugins
Load Order
Script Extenders
```

Il pourrait également intégrer des diagnostics spécifiques.

Exemple :

```text
Mod A
 ↓
requiert SKSE
 ↓
SKSE absent
```

Modlab peut alors afficher :

```text
Installation bloquée

Mod A nécessite SKSE.

[Installer SKSE]
[Ignorer]
```

---

# 14. Exemple : Minecraft Adapter

Minecraft doit être prévu dès le début dans l'architecture.

L'Adapter doit pouvoir comprendre :

```text
Minecraft versions
Forge
Fabric
NeoForge
Quilt
Vanilla
Mods .jar
Modpacks
Dependencies
Resource Packs
Shader Packs
Configs
Java
Instances
Launch arguments
```

Minecraft est particulièrement important car il force l'architecture à gérer correctement :

```text
Game version
+
Modloader
+
Mod version
+
Dependencies
```

Exemple :

```text
Minecraft 1.21.x
        │
      Fabric
        │
   ┌────┼────┐
   │    │    │
Sodium Lithium ModMenu
   │
Fabric API
```

---

# 15. Exemple : Witcher 3 Adapter

Il pourrait gérer :

```text
The Witcher 3
REDengine
mods/
DLC
scripts
mod0000_MergedFiles
Script Merger
DLC Merger
REDmod
```

L'objectif n'est pas de faire un système spécial Witcher 3 dans le Core.

L'Adapter décrit simplement les particularités du jeu.

---

# 16. Abstraction filesystem

C'est un point très important.

Le Core ne doit pas être rempli de :

```text
C:\Games\Skyrim\Data
F:\SteamLibrary\...
```

en dur.

Il faut une abstraction :

```text
GameFilesystem
ModFilesystem
ArchiveFilesystem
LibraryFilesystem
DeploymentFilesystem
```

Le chemin réel doit être une implémentation/configuration.

Objectifs :

- masquer les chemins complexes à l'utilisateur ;
- permettre de changer de disque ;
- permettre de déplacer la bibliothèque ;
- simplifier les backups ;
- faciliter les tests ;
- permettre potentiellement Linux/Proton plus tard ;
- permettre plusieurs stratégies de déploiement.

---

# 17. Gestion des téléchargements

Les téléchargements doivent être séparés de l'installation.

Workflow :

```text
Recherche
   ↓
Téléchargement
   ↓
Archive locale
   ↓
Analyse
   ↓
Pending Install
   ↓
Installation
```

L'utilisateur ne devrait pas avoir besoin de gérer manuellement :

```text
Downloads/
Archives/
Mod Files/
```

Modlab possède ces espaces.

L'utilisateur choisit éventuellement l'emplacement de stockage dans Settings.

---

# 18. Archives

Modlab doit pouvoir analyser :

- ZIP ;
- 7z ;
- RAR si pertinent ;
- autres formats nécessaires.

Lorsqu'une archive est téléchargée :

```text
Archive
 ↓
Analyse
 ↓
Détection du contenu
 ↓
Détection du type de mod
 ↓
Détection des dépendances
 ↓
Détection d'une structure FOMOD
 ↓
Détection de plusieurs mods dans une archive
```

Cas particulier :

```text
Archive
├── Mod A
├── Mod B
└── Mod C
```

Le système doit pouvoir détecter qu'il s'agit potentiellement de plusieurs mods.

---

# 19. Providers

Les plateformes de mods doivent être abstraites.

Architecture proposée :

```text
Provider
├── NexusProvider
├── CurseForgeProvider
├── GitHubProvider
├── ModDBProvider
└── autres
```

Le Core ne doit pas dépendre directement d'une plateforme particulière.

Chaque Provider peut gérer :

```text
search
get_project
get_versions
get_files
get_dependencies
download
metadata
```

---

# 20. CurseForge

Minecraft rend CurseForge particulièrement important.

Le système doit prévoir un Provider CurseForge séparé.

Ne pas intégrer les règles CurseForge directement dans le Core.

Si les API ou conditions changent, le Provider doit pouvoir évoluer indépendamment.

Vérifier systématiquement les conditions/API actuelles avant implémentation.

---

# 21. Nexus

Même logique :

```text
NexusProvider
```

L'application pourrait permettre :

- recherche ;
- informations du mod ;
- versions ;
- fichiers ;
- téléchargement ;
- dépendances ;
- métadonnées.

Ne pas supposer qu'un site web peut être automatisé simplement en affichant sa page dans WebView2.

Privilégier les API/intégrations officielles lorsqu'elles existent.

---

# 22. Profiles / Loadouts

Un utilisateur doit pouvoir avoir plusieurs environnements.

Exemple Skyrim :

```text
Skyrim
├── Vanilla+
├── Graphics
├── Survival
├── Hardcore
└── Testing
```

Chaque profil peut définir :

- mods actifs ;
- plugins ;
- load order ;
- configuration ;
- outils ;
- paramètres ;
- éventuellement version du jeu/modloader.

Minecraft :

```text
Minecraft
├── Vanilla+
├── Performance
├── Shaders
├── Modpack A
└── Modpack B
```

---

# 23. Déploiement

Le système de déploiement doit être abstrait.

Selon le jeu, il peut utiliser :

- filesystem réel ;
- liens ;
- hardlinks ;
- virtualisation ;
- overlays ;
- autre stratégie.

Le Core doit demander à l'Adapter quelle stratégie est adaptée.

---

# 24. Diagnostics

Une fonctionnalité majeure de Modlab.

Exemple :

```text
12 problèmes détectés

2 dépendances manquantes
3 conflits
1 mauvais load order
2 fichiers invalides
4 avertissements
```

Mais surtout :

**expliquer les problèmes.**

Pas simplement :

```text
ERROR 0x4827
```

Mais :

```text
❌ Immersive Armor ne fonctionne probablement pas.

Cause :
Le plugin X nécessite Y.

Y est absent.

Solution recommandée :
Installer Y version 3.4.1.
```

---

# 25. AI Layer

L'IA doit être une couche indépendante du Core.

Objectif :

pouvoir utiliser différents modèles :

```text
Qwen
Llama
Mistral
Claude
OpenAI
autres modèles
```

Le Core ne doit pas dépendre d'un modèle particulier.

Architecture :

```text
AI Provider
├── Local Qwen
├── Local Llama
├── Local Mistral
├── Claude
└── OpenAI
```

---

# 26. IA locale

Qwen est envisagé pour l'IA locale.

L'important est que le modèle ne soit pas obligé de connaître parfaitement chaque jeu.

Il peut interroger Modlab.

Exemple :

```text
User:
"Pourquoi mon Skyrim crash ?"

Qwen
 ↓
inspect_game()
 ↓
inspect_modlist()
 ↓
read_logs()
 ↓
find_conflicts()
 ↓
find_dependencies()
 ↓
diagnostic
```

L'IA devient donc une couche de raisonnement au-dessus d'un système de données fiable.

---

# 27. MCP

MCP = Model Context Protocol.

Modlab devrait exposer un **MCP Modlab générique**.

Le MCP ne doit pas être spécifique à Claude.

Il doit pouvoir être utilisé par toute IA compatible MCP.

Architecture :

```text
                    Modlab MCP
                        │
          ┌─────────────┼─────────────┐
          │             │             │
       Claude          Qwen       autre IA
          │             │             │
          └─────────────┼─────────────┘
                        │
                   Modlab Core
```

---

# 28. Outils MCP

Le MCP pourrait exposer des outils comme :

## Lecture

```text
inspect_game()
inspect_mod()
inspect_archive()
inspect_profile()
search_mods()
get_dependencies()
find_conflicts()
get_load_order()
get_logs()
get_file()
get_game_state()
```

## Actions

```text
download_mod()
install_mod()
uninstall_mod()
enable_mod()
disable_mod()
set_load_order()
create_profile()
launch_game()
run_tool()
repair_mod()
create_backup()
restore_backup()
```

La liste définitive doit être déterminée après analyse des besoins réels.

---

# 29. Permissions MCP

L'IA ne doit pas avoir accès directement au filesystem brut.

Mauvaise architecture :

```text
AI
 ↓
arbitrary shell
 ↓
C:\
```

Architecture souhaitée :

```text
AI
 ↓
MCP
 ↓
Modlab permissions
 ↓
Modlab tool
 ↓
Rust Core
 ↓
Filesystem
```

Les actions peuvent avoir différents niveaux :

### Lecture

Automatique :

```text
inspect_game
inspect_mod
find_conflicts
```

### Modification réversible

Confirmation potentiellement configurable :

```text
install_mod
enable_mod
disable_mod
```

### Action dangereuse

Confirmation obligatoire :

```text
delete
overwrite
modify system files
execute arbitrary commands
```

---

# 30. Pourquoi MCP est particulièrement intéressant

Sans outils :

```text
IA
 ↓
"Je pense que ton mod est incompatible."
```

Avec Modlab :

```text
IA
 ↓
inspect_game()
 ↓
Minecraft 1.21.8
Fabric
 ↓
inspect_mod()
 ↓
Mod requires Fabric API >= X
 ↓
get_installed_dependencies()
 ↓
Fabric API absent
```

L'IA ne devine plus.

Elle **observe l'état réel de l'environnement**.

---

# 31. MCP comme API intelligente

Il faut penser le MCP comme une couche de contrôle, pas uniquement comme une fonctionnalité chatbot.

Architecture :

```text
                         Modlab Core
                              │
              ┌───────────────┼───────────────┐
              │               │               │
             UI              MCP             CLI/API
              │               │               │
          humain             IA              outils
```

Tous ces systèmes utilisent les mêmes capacités du Core.

Cela évite d'avoir :

```text
UI → logique A
MCP → logique B
CLI → logique C
```

Il faut une seule logique métier.

---

# 32. AI + Game Adapter

Le Game Adapter doit fournir au système les connaissances structurées du jeu.

L'IA peut alors demander :

```text
inspect_game()
```

et recevoir quelque chose de spécifique :

```text
Game: Skyrim SE
Version: 1.6.1170
Script Extender: installed
Plugins: 184
Active mods: 247
Conflicts: 3
Missing dependencies: 1
```

Pour Minecraft :

```text
Game: Minecraft
Version: 1.21.x
Loader: Fabric
Loader version: X
Mods: 87
Missing dependencies: 2
```

L'IA n'a donc pas besoin de connaître ces structures à l'avance.

---

# 33. Interface utilisateur

La maquette actuelle doit être analysée avant de figer l'architecture UI.

L'idée initiale était une interface proche d'une bibliothèque Steam :

```text
Jeux à gauche
Mods à droite
```

Mais la vision actuelle est davantage :

> **IDE de modding**

L'interface pourrait donc utiliser :

```text
┌─────────────────────────────────────────────┐
│ Toolbar                                     │
├────────────┬──────────────────┬─────────────┤
│            │                  │             │
│ Explorer   │ Workspace        │ AI          │
│            │                  │             │
│ Games      │ Mods             │ Assistant   │
│ Mods       │ Conflicts        │             │
│ Profiles   │ Files            │             │
│ Tools      │ Logs             │             │
│            │ Terminal         │             │
│            │                  │             │
├────────────┴──────────────────┴─────────────┤
│ Logs / Terminal / Diagnostics                │
└─────────────────────────────────────────────┘
```

Mais **ceci est seulement une hypothèse**.

Claude Code doit analyser la maquette réelle et proposer :

- meilleure organisation ;
- meilleure hiérarchie ;
- meilleure navigation ;
- meilleur usage de l'espace ;
- composants à conserver ;
- composants à supprimer ;
- éventuels problèmes UX.

---

# 34. Philosophie UI

Éviter autant que possible de montrer :

```text
C:\Users\Benjamin\AppData\...
F:\SteamLibrary\...
```

à un utilisateur normal.

Montrer plutôt :

```text
Skyrim
> Mods
> Profiles
> Tools
> Downloads
```

Les chemins réels restent disponibles pour les utilisateurs experts.

---

# 35. Une seule source de vérité

Le système doit éviter de maintenir plusieurs états contradictoires.

Exemple :

```text
Database
Filesystem
UI
MCP
AI
```

Tous doivent être synchronisés autour du Core.

Le Core connaît l'état réel.

L'UI ne doit pas inventer son propre état.

---

# 36. Database

SQLite est envisagé.

Elle pourrait stocker :

```text
Games
Mods
Files
Archives
Profiles
Dependencies
Conflicts
Versions
Downloads
Deployments
Tools
Providers
Settings
Actions
Backups
Diagnostics
```

Il faut toutefois éviter de stocker inutilement des informations qui peuvent être recalculées à partir du filesystem.

---

# 37. Historique des actions

Une idée importante :

Modlab devrait garder un historique :

```text
10:31
Installed Sodium

10:32
Installed Fabric API

10:33
Enabled Sodium

10:34
Detected conflict
```

Et idéalement :

```text
[Undo]
```

ou :

```text
Rollback to state
```

---

# 38. Logs

Tous les systèmes importants doivent produire des logs exploitables :

```text
Core
Downloads
Archives
Deployment
Game Adapter
Provider
MCP
AI
Tools
```

L'IA doit pouvoir lire ces logs via des outils contrôlés.

---

# 39. Terminal

Un terminal intégré peut être utile pour les utilisateurs avancés.

Mais il doit rester séparé du système de permissions IA.

Le terminal humain peut être plus puissant.

L'IA ne doit pas automatiquement avoir le même accès que le terminal.

---

# 40. Outils externes

Modlab devrait pouvoir détecter et gérer les outils externes.

Exemples Skyrim :

```text
SKSE
xEdit
LOOT
DynDOLOD
Nemesis
Pandora
```

Minecraft :

```text
Java
Mod loaders
outils éventuels
```

Witcher 3 :

```text
Script Merger
DLC tools
REDmod
```

L'Adapter doit déclarer les outils pertinents.

---

# 41. Multi-game dès le départ

Même si le développement commence avec Skyrim, l'architecture doit être multi-game dès le début.

Ordre envisagé :

```text
Phase 1
Skyrim

Phase 2
Minecraft

Phase 3
Witcher 3

Puis :
Fallout
Cyberpunk
etc.
```

Il faut éviter de construire Skyrim de façon tellement spécifique que Minecraft nécessite une réécriture du Core.

---

# 42. Le Core ne doit pas connaître Skyrim

Exemple de mauvaise architecture :

```text
Core
├── SkyrimInstall()
├── SkyrimPlugin()
├── SkyrimLoadOrder()
├── MinecraftInstall()
└── MinecraftLoader()
```

Préférer :

```text
Core
├── install_mod()
├── inspect_game()
├── get_load_order()
├── find_dependencies()
└── deploy()
```

Puis :

```text
SkyrimAdapter
MinecraftAdapter
Witcher3Adapter
```

implémentent les différences.

---

# 43. Modèle conceptuel

Il faut réfléchir en objets/domaines plutôt qu'en chemins de fichiers.

Par exemple :

```text
Game
Mod
ModVersion
Archive
File
Dependency
Conflict
Profile
Plugin
Tool
Deployment
Download
Provider
Diagnostic
Action
```

Cela permettra à l'IA et au MCP d'avoir une représentation propre.

---

# 44. Objectif ultime

L'utilisateur ne devrait pas avoir besoin de savoir :

```text
où se trouve le fichier
comment extraire l'archive
quel dossier utiliser
quelle dépendance installer
dans quel ordre charger les mods
quelle version télécharger
quel outil lancer
quel log consulter
```

sauf lorsqu'il veut réellement le faire lui-même.

Il peut simplement dire :

> "Je veux ce mod."

ou :

> "Fais-moi un setup graphique."

ou :

> "Pourquoi ça crash ?"

ou :

> "Optimise cette modlist."

Et Modlab doit transformer cela en opérations concrètes, vérifiables et réversibles.

---

# 45. Mais ne jamais transformer Modlab en "boîte noire"

L'utilisateur expert doit pouvoir descendre dans les couches :

```text
Simple
 ↓
Advanced
 ↓
Expert
 ↓
Raw
```

Il doit toujours être possible de comprendre :

```text
Pourquoi ?
Comment ?
Quels fichiers ?
Quelle règle ?
Quelle dépendance ?
Quelle action ?
Quel résultat ?
```

---

# 46. Critères de réussite

Modlab sera réussi si :

### Débutant

Peut installer des mods sans comprendre la structure interne.

### Utilisateur intermédiaire

Peut gérer facilement une modlist importante.

### Expert

Peut contrôler précisément l'environnement.

### Développeur de mods

Peut analyser/debugger ses mods.

### IA

Peut comprendre l'état réel du jeu et agir via des outils sûrs.

---

# 47. Questions que Claude Code doit analyser

Avant de commencer une grosse implémentation, analyser notamment :

## Architecture

- Rust est-il réellement le meilleur choix pour le Core ?
- Tauri est-il toujours le meilleur shell ?
- React/TypeScript est-il adapté à la maquette réelle ?
- SQLite est-il suffisant ?
- Faut-il séparer davantage les crates Rust ?
- Faut-il créer un Core totalement indépendant de Tauri ?

## Game Adapter

- Quelles interfaces communes sont réellement nécessaires ?
- Qu'est-ce qui doit être dans le Core ?
- Qu'est-ce qui doit être dans l'Adapter ?
- Comment éviter de créer une abstraction trop générique et inutile ?

## Minecraft

- Quelle abstraction représente correctement :
  - version ;
  - modloader ;
  - instance ;
  - modpack ;
  - dépendance ?
- Comment gérer CurseForge ?
- Comment gérer Fabric/Forge/NeoForge/Quilt ?

## Skyrim

- Comment gérer plugins/load order ?
- Virtual filesystem ou deployment classique ?
- Comment intégrer les outils externes ?
- Comment gérer FOMOD ?

## AI

- Quelle abstraction pour les modèles ?
- Comment intégrer Qwen localement ?
- Quel runtime d'inférence ?
- Comment gérer la VRAM ?
- Comment gérer le contexte ?
- Comment éviter les hallucinations ?
- Quels outils MCP sont réellement nécessaires ?

## MCP

- Quelle granularité pour les tools ?
- Comment gérer permissions ?
- Comment gérer confirmations ?
- Comment éviter qu'un agent fasse une action dangereuse ?
- Comment exposer l'état de Modlab proprement ?

## UX

- La maquette actuelle correspond-elle réellement à cette vision ?
- Est-ce trop complexe ?
- Quels panneaux sont inutiles ?
- Quels éléments devraient être fusionnés ?
- Comment créer une interface puissante sans la rendre intimidante ?

---

# 48. Règle importante pour Claude Code

**Ne pas implémenter aveuglément ce document.**

Avant toute architecture définitive :

1. analyser la maquette existante ;
2. analyser le repository ;
3. analyser les contraintes Windows ;
4. analyser les API disponibles ;
5. analyser les besoins Skyrim ;
6. analyser les besoins Minecraft ;
7. analyser le fonctionnement MCP ;
8. proposer les changements nécessaires ;
9. identifier les risques ;
10. proposer une architecture finale.

Le document représente la **vision et les besoins**, pas nécessairement la solution technique définitive.

---

# 49. Architecture cible conceptuelle

La cible idéale ressemble à :

```text
                         ┌──────────────────────┐
                         │        USERS         │
                         └──────────┬───────────┘
                                    │
                         ┌──────────▼───────────┐
                         │      Modlab UI       │
                         │     React / TS       │
                         └──────────┬───────────┘
                                    │
                                  Tauri
                                    │
                         ┌──────────▼───────────┐
                         │     MODLAB CORE      │
                         │        Rust          │
                         └──────────┬───────────┘
                                    │
       ┌──────────────┬─────────────┼──────────────┬─────────────┐
       │              │             │              │             │
   Downloads       Archives     Deployment      Database      Processes
       │              │             │              │             │
       └──────────────┴─────────────┴──────────────┴─────────────┘
                                    │
                         ┌──────────▼───────────┐
                         │    GAME ADAPTERS     │
                         ├──────────────────────┤
                         │ Skyrim               │
                         │ Minecraft            │
                         │ Witcher 3            │
                         │ ...                  │
                         └──────────┬───────────┘
                                    │
                         ┌──────────▼───────────┐
                         │      PROVIDERS       │
                         ├──────────────────────┤
                         │ Nexus                │
                         │ CurseForge           │
                         │ GitHub               │
                         │ ModDB                │
                         │ ...                  │
                         └──────────────────────┘


                  AI / AUTOMATION LAYER
                           │
              ┌────────────┴────────────┐
              │                         │
         Local Models               External
         Qwen / Llama               Claude/etc.
              │                         │
              └────────────┬────────────┘
                           │
                          MCP
                           │
                    Modlab Core
```

---

# 50. Philosophie finale

Modlab ne doit pas être :

> "un autre mod manager."

Il doit devenir :

> **l'environnement dans lequel on fait du modding.**

L'utilisateur peut :

- découvrir ;
- télécharger ;
- analyser ;
- installer ;
- configurer ;
- organiser ;
- diagnostiquer ;
- modifier ;
- tester ;
- lancer ;
- réparer ;
- automatiser ;
- comprendre ;

tout cela depuis le même environnement.

Et l'IA doit être capable de travailler **avec le même environnement**, plutôt que d'être simplement un chatbot posé à côté.

Le principe fondamental est :

> **L'IA raisonne. Modlab connaît l'état réel. Le Core exécute. Le Game Adapter connaît les règles du jeu. Le MCP permet à différentes IA d'utiliser ces capacités.**

C'est cette séparation qui doit permettre à Modlab de rester extensible, multi-jeux, multi-IA et maintenable sur le long terme.