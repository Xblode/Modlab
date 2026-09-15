# ADR 0001 — USVFS comme backend de déploiement, via un binding FFI Rust écrit à la main

- **Statut :** accepté (rétroactif — décision prise et vérifiée en Phase 1, commit `c1b508a` puis `8c8edb2`, 15 septembre 2026)
- **Portée :** `crates/modlab-game-skyrim/src/usvfs.rs`, adaptateur Skyrim
- **Références :** `docs/01-architecture-decidee.md` §2 (décision 4), §4, §8, §13 (Phase 1), §14 (risque « USVFS via FFI Rust plus dur que prévu »)

---

## Contexte

Le jalon de sortie de la Phase 1 est **« je joue à Skyrim lancé par ModLab avec ma vraie modlist »**. Ça suppose un backend de déploiement capable de présenter au jeu une vue fusionnée de ~400 dossiers de mods **sans écrire un seul fichier dans le `Data/` réel** — l'architecture verrouillée interdit d'écrire directement dans les dossiers du jeu (`CLAUDE.md`, §5 : tout est réversible, rien n'est jamais écrasé en place).

`01-architecture-decidee.md` §2 avait déjà tranché le principe (« USVFS par défaut + matérialisation à la demande »), mais pas le **comment** : USVFS est une DLL C++ (`usvfs_x64.dll`), et la façon de l'appeler depuis Rust restait à décider. Le spike Phase 0 « FFI USVFS » avait seulement établi que la DLL expose un ABI C propre et non décoré (`dumpbin /EXPORTS` sur la vraie DLL de l'installation MO2 de Benjamin).

## Options considérées

1. **Backend hardlink / copy-on-write maison** (le plan B de §14). Pur Rust, aucun FFI, aucune dépendance externe. Mais : écrit réellement dans `Data/`, donc ownership des fichiers à tracer à la main, désinstallation fragile, et divergence immédiate d'avec le comportement que Benjamin connaît de MO2. Ne satisfait pas « rien n'est écrit dans le jeu ».
2. **USVFS lié au *link time*** via une bibliothèque d'import générée (`.lib`) ou un crate `-sys` auto-généré par `bindgen` sur les en-têtes du dépôt `ModOrganizer2/usvfs`. Ergonomique, typé automatiquement — mais exige que la DLL soit présente **au démarrage du processus**, vendorée dans le repo, et fige les noms de symboles de l'en-tête `master`.
3. **USVFS chargé dynamiquement** via `libloading`, avec un binding FFI écrit à la main : types de fonction déclarés à la main, symboles résolus par `GetProcAddress` au moment de l'appel, chemin de la DLL fourni par l'appelant.
4. **Sidecar** réutilisant l'outillage MO2 (lancer `usvfs_proxy.exe` ou équivalent). Ajoute un processus et un protocole à maintenir pour un gain nul ici — l'ABI est déjà appelable directement.

## Décision

**Option 3.** USVFS est le backend de déploiement Skyrim de la Phase 1, accédé par un **binding FFI Rust écrit à la main et chargé dynamiquement avec `libloading`**, la DLL étant désignée par un chemin fourni par l'appelant (pendant la Phase 1 : la DLL de la vraie installation MO2 de Benjamin — ModLab ne vendorise pas encore sa propre copie).

Le binding reste **confiné dans l'adaptateur Skyrim** : le Core ne connaît ni USVFS, ni `Data/`, ni SKSE. Il ne voit que des `LoadoutEntryRow` ordonnées ; c'est `deploy_enabled_mods` côté adaptateur qui les traduit en appels `VirtualLinkDirectoryStatic`. La règle « le Core ne connaît aucun jeu » (§3) est respectée.

## Conséquences

**Ce que ça nous donne.**

- Le déploiement ne touche **aucun fichier réel** : USVFS hooke les API fichiers de Windows *dans le processus enfant lancé*. Le rollback d'un déploiement est, littéralement, « ne pas relancer le jeu » — la propriété la plus forte qu'on pouvait obtenir pour le risque n°1 du projet (perte de fichiers, §14).
- Le chargement dynamique nous rend **résilients aux variantes de build de la DLL**, ce qui s'est avéré indispensable (voir ci-dessous).

**Ce que ça nous coûte, et que toute session future doit savoir.**

1. **Les noms de symboles de l'en-tête public ne sont pas ceux de la DLL réelle.** La `usvfs_x64.dll` de Benjamin (build `0.5.5.1`, livrée avec son MO2) est à mi-chemin d'une migration de nommage : les fonctions de *paramètres* portent déjà les noms modernes préfixés (`usvfsCreateParameters`, `usvfsSetInstanceName`, `usvfsCreateVFS`, `usvfsFreeParameters`), mais les opérations VFS n'existent **que** sous leurs anciens noms non préfixés (`VirtualLinkDirectoryStatic`, `CreateProcessHooked`, `DisconnectVFS`). La toute première version du module échouait immédiatement sur un `GetProcAddress` « procedure not found ». **Règle qui en découle : ne jamais supposer qu'un symbole de l'en-tête est exporté par la DLL chargée — le vérifier.** C'est exactement ce que l'option 2 (link-time / `bindgen` sur `master`) aurait rendu impossible à absorber sans recompiler.
2. **`USVFSVersionString` n'est pas appelé**, délibérément. Ce build n'expose pas `usvfsVersionString`, seulement une forme ancienne dont la convention d'appel n'est documentée nulle part dont on dispose. Deviner un détail d'ABI C++ peut corrompre la pile ; on s'abstient. Conséquence : **ModLab ne sait pas encore lire la version de la DLL qu'il charge**, ce qui reste à résoudre le jour où on vendorisera notre propre copie.
3. **Un seul `Vfs` vivant par processus OS.** L'en-tête le dit (« un processus ne peut être connecté qu'à un seul vfs », bascule silencieuse), et c'est confirmé pour de vrai : deux `Vfs` créés simultanément sur deux threads du *même* binaire de test produisent un `CreateVfsFailed` reproductible (d'où `VFS_TEST_LOCK`). Sans conséquence pour `modlab-cli` (un processus par invocation), **mais c'est une contrainte structurante pour tout futur processus ModLab de longue durée — typiquement le backend Tauri**, qui ne pourra pas déployer deux loadouts en parallèle dans le même processus. Si le besoin apparaît, il faudra un processus déployeur dédié par session.
4. **La fin de session se détecte par sondage de `GetVFSProcessList`, pas par attente d'un handle de processus.** `skse64_loader.exe` se termine quelques secondes après avoir passé la main au vrai processus du jeu ; attendre son handle conclurait « partie terminée » presque tout de suite. On sonde donc le nombre de processus enregistrés contre l'instance VFS, avec une phase d'amorçage (`startup_timeout`) qui distingue « pas encore démarré » de « déjà terminé ». Toute future intégration d'outil lancé sous VFS (xEdit, Nemesis, DynDOLOD) doit réutiliser ce mécanisme, pas réinventer une attente sur handle.
5. **Licence :** USVFS est GPL-3, `libloading` est MIT/Apache-2.0 — compatibles avec la licence GPL-3 de ModLab (§2, décision 9). Aucun drapeau.
6. **Dette assumée, non bloquante :** la DLL n'est pas vendorée, donc ModLab dépend aujourd'hui d'une installation MO2 présente. Acceptable pour la Phase 1 (dont l'import MO2 est de toute façon le point d'entrée) ; à reprendre avant toute distribution.

## Ce que cette décision ne tranche pas

- La **matérialisation à la demande** (backend fichiers réels pour ENB/ReShade/Root Builder, §9) reste à faire — USVFS ne couvre pas les fichiers à la racine du jeu.
- Le choix reste **encapsulé derrière le Game Adapter** : un futur jeu (Minecraft) n'hérite de rien de tout ça, et un futur backend hardlink pourra coexister sans que le Core bouge.
