# ModLab — Architecture décidée

> **Statut : verrouillé le 15 septembre 2026.** Ce document est la référence unique pour démarrer l'implémentation.
>
> **Ce qu'il remplace :** `00-cadrage-technique.md` et `Modlab — Document de cadrage & architecture.md` sur toute décision structurante (stack, architecture, permissions, roadmap). Les deux restent dans `docs/` comme historique de la réflexion — ce document-ci synthétise et tranche, il ne les répète pas en entier.
>
> **Ce qui ne change pas :** `docs/idees.md` (parking d'idées), `docs/design/shell-maquette.html` (la maquette — **source de vérité visuelle**, activement itérée en parallèle de ce document).
>
> **Légende :** ✅ décidé · ⚠️ risque connu, avec plan · 🔬 à valider par spike Phase 0 · 💬 encore ouvert

---

## 1. Vision & positionnement

> **Modlab — "Cursor pour le game modding."**

Pas un MO2 plus joli, pas un Vortex plus ouvert : un **environnement de travail** pour le modding — mod manager, explorateur, gestionnaire de téléchargements/archives, système de déploiement, système de diagnostic, multi-jeux par construction, et un environnement dans lequel une IA peut **comprendre et manipuler réellement** un jeu moddé, pas juste en discuter à côté.

| Outil | Promesse |
|---|---|
| MO2 | "Je contrôle précisément mon installation." |
| Vortex | "Le gestionnaire automatise mon installation." |
| CurseForge | "Je veux installer et maintenir facilement un environnement moddé." |
| **ModLab** | **"Je peux laisser ModLab automatiser le travail, mais je peux comprendre et contrôler absolument tout quand j'en ai besoin."** |

Trois niveaux d'usage cohabitent dans la même app : **Débutant** (Installer / Mettre à jour / Lancer / Réparer, rien d'autre à comprendre), **Avancé** (conflits, dépendances, load order, logs), **Expert** (filesystem, graphe de dépendances/conflits, console, API, MCP, commandes avancées). L'automatisation ne doit **jamais devenir une boîte noire** : chaque action explique quoi, pourquoi, quels fichiers, et comment revenir en arrière.

Les 10 lignes conductrices du cadrage d'origine (personnel d'abord, MO2 = référence de maîtrise, GitHub/Claude Code/Cursor = référence d'interface, réversibilité totale, vérité par les faits, le créatif tient le volant…) restent valables telles quelles — voir `00-cadrage-technique.md` §1.2 pour le détail.

---

## 2. Décisions structurantes verrouillées

| # | Décision | Verdict |
|---|---|---|
| 1 | **Runtime Core** | ✅ **Rust** (voir §3, §4 pour le risque associé) |
| 2 | **UI** | ✅ **React + TypeScript + Vite**, dans un shell **Tauri** (pas WPF/Avalonia) |
| 3 | **Layout disque** | ✅ Compatible MO2 (dossiers de mods nommés, profils exportables) + CAS (content-addressed store) derrière |
| 4 | **Backend de déploiement Skyrim** | ✅ Abstrait par le Game Adapter ; USVFS par défaut + matérialisation à la demande |
| 5 | **Outil de suivi** | ✅ ClickUp, plan gratuit, distinction par tags (pas par champ personnalisé) |
| 6 | **Racine des données** | ✅ **`F:\ModLab`** |
| 7 | **Versioning** | ✅ `git init` local dès la Phase 0 ; **pas de remote GitHub pour l'instant** (repoussé jusqu'à ce que ce soit vraiment nécessaire) |
| 8 | **IA locale** | ✅ Déjà implémenté dans la maquette (v21+) : Qwen2.5-1.5B-Instruct par défaut, choix Llama-3.2-3B / Phi-3.5-mini / Qwen3 8B, file d'attente pendant le jeu — voir §6 |
| 9 | **Licence** | ✅ GPL-3 dès maintenant (cohérent avec l'écosystème MO2/Wabbajack/Mutagen, aucun impact tant que c'est privé) |
| 10 | **Design system / UI** | ✅ Le style de la maquette HTML actuelle est la cible — pas de refonte visuelle ; toute évolution UI passe par la maquette d'abord |

---

## 3. Architecture cible

```text
                         ┌──────────────────────┐
                         │        Toi           │
                         └──────────┬───────────┘
                                    │
                         ┌──────────▼───────────┐
                         │      ModLab UI        │
                         │   React / TypeScript  │
                         └──────────┬───────────┘
                                    │
                                  Tauri (IPC)
                                    │
                         ┌──────────▼───────────┐
                         │     MODLAB CORE       │   ← source de vérité unique
                         │         Rust          │
                         └──────────┬───────────┘
                                    │
      ┌───────────┬────────────────┼────────────────┬───────────────┐
      │           │                │                 │               │
  Downloads    Archives       Deployment          Database       Processes
      │           │                │                 │               │
      └───────────┴────────────────┴─────────────────┴───────────────┘
                                    │
                         ┌──────────▼───────────┐
                         │    GAME ADAPTERS      │   ← connaît les règles du jeu
                         │  Skyrim · Minecraft ·  │
                         │  Witcher 3 · ...       │
                         └──────────┬───────────┘
                                    │
                         ┌──────────▼───────────┐
                         │      PROVIDERS        │   ← connaît une plateforme de mods
                         │  Nexus · CurseForge ·  │
                         │  GitHub · ModDB · ...  │
                         └──────────────────────┘

                    AI / AUTOMATION LAYER (indépendante du Core)
                                    │
                     ┌──────────────┴──────────────┐
                     │                             │
              Modèles locaux                  Modèles externes
             (Qwen / Llama / Phi)                 (Claude)
                     │                             │
                     └──────────────┬──────────────┘
                                    │
                                   MCP  ←  générique, utilisable par toute IA compatible
                                    │
                             ModLab Core
```

**Principe non-négociable : un seul moteur, plusieurs clients.** UI, CLI et serveur MCP parlent tous à la même API du Core Rust via IPC (Tauri) ou JSON-RPC local (CLI/MCP). Aucun client n'invente son propre état — sinon on aura des désynchronisations entre ce que Claude fait via MCP et ce que tu vois dans l'UI.

**Le Core ne connaît aucun jeu.** Il expose des primitives génériques (`install_mod()`, `inspect_game()`, `get_load_order()`, `find_dependencies()`, `deploy()`) ; c'est le **Game Adapter** qui traduit ces primitives dans le vocabulaire du jeu (ESP/ESM/BSA pour Skyrim, Fabric/Forge/`.jar` pour Minecraft). Même logique pour les plateformes : le Core ne sait pas ce qu'est Nexus, un **Provider** le sait. Ça évite le piège identifié dans les deux documents d'origine : un Core qui accumule des `if game == Skyrim` un peu partout et devient impossible à étendre.

### 3.1 Responsabilités d'un Game Adapter

Identification (détecter le jeu, l'édition, la version) · Structure (où sont `Data/`, les plugins, les configs, les saves) · Modding (formats, load order, modloaders, dépendances) · Installation / désinstallation propre · Diagnostic (dépendances manquantes, incompatibilités, erreurs connues) · Lancement (jeu, launcher, modloader, outils) · Déclaration des outils spécifiques (SKSE/xEdit/LOOT pour Skyrim, rien de spécifique dans le Core).

### 3.2 Responsabilités d'un Provider

`search()` · `get_project()` · `get_versions()` · `get_files()` · `get_dependencies()` · `download()` · `metadata()`. Nexus et CurseForge sont les deux premiers ; GitHub/ModDB viendront si le besoin se confirme. **Ne jamais supposer qu'une page web peut être automatisée simplement en l'affichant dans une WebView — privilégier l'API officielle, vérifier les conditions d'utilisation à chaque implémentation.**

---

## 4. ⚠️ Le risque Mutagen — et le plan

**Le problème.** Mutagen (C#) est la seule bibliothèque de l'écosystème qui lit/écrit les plugins Bethesda de façon *typée*, au niveau des *records* — c'est ce qui permet l'analyse de conflits fine (xEdit-like) et la génération de patchers à la volée (Synthesis-like). **Il n'existe pas d'équivalent Rust mature.** `esplugin` (Rust, utilisé par libloot) ne fait que lire les en-têtes et les masters — suffisant pour le tri du load order, pas pour l'analyse record-level.

**Le plan, en trois paliers :**

1. **Maintenant (Phase 0-1) : orchestrer xEdit**, exactement comme MO2 le fait aujourd'hui — arguments (`-autoload`, `-autoexit`, `-veryquickshowconflicts`) + scripts Pascal, capture des logs. Zéro nouveau risque, prouvé, **ne bloque pas** le jalon "je joue à Skyrim lancé par ModLab".
2. **Phase 4+ : évaluer l'écriture d'un parseur Rust maison** pour les records qu'on utilise le plus souvent (le format est documenté, `esplugin` prouve que c'est faisable côté lecture). Décision prise seulement si l'orchestration xEdit s'avère un vrai frein en usage réel.
3. **Repli : sidecar .NET minimal avec Mutagen**, appelé en local par le Core Rust juste pour cette tâche précise (un seul composant polyglotte, isolé, pas tout le Core en C#).

🔬 **Bonus Rust qui compense en partie :** libloot (le tri LOOT) a été **réécrit en Rust en 2025** — l'intégrer depuis un Core Rust est donc *plus simple* que ça ne l'aurait été depuis C# (pas de marshaling P/Invoke complexe, juste une dépendance Rust native ou un ABI C fin). Pareil pour USVFS (C++) : le FFI Rust vers une DLL C++ à ABI C est direct. `sevenz-rust` (7z pur Rust, sans dépendance à `7z.dll`) est une piste à valider en spike pour les archives.

---

## 5. Modèle de données & transactions

*(Conceptuellement inchangé depuis `00-cadrage-technique.md` §4.3/§4.6 — juste porté en Rust : SQLite reste la base, un seul processus Rust en est le seul écrivain.)*

**Sur disque**, racine `F:\ModLab\` :

```text
F:\ModLab\
├─ store\blobs\ab\<hash>          # CAS : chaque contenu unique une seule fois
├─ downloads\<archive>.7z + .meta.json
├─ games\skyrimse\
│   ├─ mods\<Nom du mod>\...      # dossiers de mods (compatibles MO2)
│   ├─ loadouts\<Nom>\            # loadout.toml, plugins.txt, ini\, saves\
│   ├─ overwrite\
│   └─ tools\
├─ db\modlab.db                   # SQLite (WAL)
├─ trash\                         # rien n'est jamais supprimé directement
└─ logs\
```

**Trois idées structurantes**, inchangées : **CAS** (dédup, provenance, snapshots gratuits) · **révisions immuables + event log** (on ne réécrit jamais un état, on en crée un nouveau — rollback = revenir à une révision) · **"loadout as code"** (chaque loadout sérialisé en `loadout.toml` + `plugins.txt` dans un dépôt git interne, en plus de SQLite — diff et historique lisibles, Claude est excellent avec git).

**Toute mutation** suit le même cycle quel que soit le client (UI, CLI, MCP) :

```text
intention → PLAN (lisible) → APPROBATION → SNAPSHOT → APPLY → VÉRIFICATION → COMMIT | ROLLBACK auto
```

---

## 6. IA locale — déjà décidé et implémenté

Ce point est **verrouillé et déjà visible dans la maquette** (`shell-maquette.html`, à partir de la v21). Récap :

- **Le moteur de règles fait le diagnostic, le LLM ne fait que rédiger** — grammaire contrainte (GBNF) pour empêcher le modèle d'inventer un fait hors des données transmises.
- **Modèles au choix**, zéro config : Qwen2.5-1.5B-Instruct (défaut), Llama-3.2-3B-Instruct, Phi-3.5-mini-Instruct, Qwen3 8B (auto-suggéré si GPU ≥ 8 Go VRAM).
- **Cycle de vie** : chargement à la demande, déchargement après inactivité, **jamais chargé pendant qu'un jeu tourne** — les demandes sont mises en file d'attente et traitées automatiquement à la fin de la partie.

**Conséquence du choix Rust :** le binding technique change. **LLamaSharp était spécifique C#** — côté Rust, l'équivalent est un binding vers llama.cpp (pistes : `llama-cpp-2`, ou une alternative 100 % Rust comme `candle`). 🔬 **À valider par spike Phase 0** — le format GGUF et le choix des modèles ne changent pas, seule la bibliothèque d'inférence change.

Ce point confirme aussi l'esprit du nouveau document (§25-26) : la couche IA est indépendante du Core et interchangeable entre modèles locaux (Qwen/Llama/Phi) et externes (Claude) — déjà le cas dans la maquette (le sélecteur de modèle dans Réglages *est* cette abstraction, côté UI).

---

## 7. MCP : outils et permissions

Le MCP ModLab doit être **générique**, utilisable par n'importe quelle IA compatible MCP (Claude Code, Claude Desktop, ou autre) — pas une fonctionnalité "chatbot" posée à côté, une vraie couche de contrôle au même niveau que l'UI et la CLI.

### 7.1 Outils (fusion des deux propositions d'origine)

| Catégorie | Outils |
|---|---|
| **Lecture** | `inspect_game`, `inspect_mod`, `inspect_archive`, `inspect_profile`, `search_mods`, `get_dependencies`, `find_conflicts`, `get_load_order`, `get_logs`, `get_file`, `get_game_state` |
| **Mutation réversible** | `download_mod`, `install_mod`, `uninstall_mod`, `enable_mod`, `disable_mod`, `set_load_order`, `create_profile`, `run_tool`, `repair_mod` |
| **Action dangereuse** | `delete`, `overwrite`, `modify_system_files`, `execute_arbitrary_commands` |
| **Transactionnel** | `modlab_plan` (construire un plan), `modlab_apply` (exécuter un plan approuvé), `modlab_history` (révisions, diff, rollback), `modlab_kb` (base de connaissances perso), `modlab_job` (suivre une opération longue) |

### 7.2 Permissions — modèle à 3 niveaux (adopté du nouveau document, plus précis que la version d'origine)

| Niveau | Exemples | Confirmation |
|---|---|---|
| **Lecture** | `inspect_game`, `find_conflicts` | Automatique, aucune confirmation |
| **Mutation réversible** | `install_mod`, `enable_mod`, `set_load_order` | Configurable (policy "auto si réversible et sans conflit", ou toujours demander) |
| **Action dangereuse** | `delete`, écriture hors du périmètre ModLab, commandes arbitraires | **Toujours** une confirmation explicite |

L'IA n'a **jamais** d'accès direct au filesystem brut ou à un shell arbitraire — toujours `IA → MCP → permissions ModLab → outil ModLab → Core Rust → filesystem`. Le terminal humain (utilisateurs avancés) reste plus puissant que ce que l'IA peut faire — deux niveaux d'accès séparés.

---

## 8. Stack technique

| Brique | Choix | Pourquoi |
|---|---|---|
| **Core** | **Rust** | Fit naturel pour fichiers/archives/téléchargements/process/FFI vers USVFS et libloot (déjà Rust) |
| **UI** | **React + TypeScript + Vite** | Maquette déjà en HTML/CSS/JS proche de ce stack, aucune perte de travail |
| **Shell desktop** | **Tauri** (WebView2 sous Windows) | Overhead plus faible qu'Electron, permissions fines, IPC natif vers le Core Rust |
| **Base de données** | **SQLite** (WAL) | Zéro serveur, un seul écrivain (le Core), cohérent avec "un moteur, plusieurs clients" |
| **Records Bethesda** | 🔬 xEdit orchestré (v1) → Rust maison ou sidecar Mutagen (voir §4) | Le point le plus incertain de la stack, plan de repli clair |
| **Tri load order** | **libloot** (déjà Rust) | Intégration native, pas de FFI complexe |
| **VFS Skyrim** | **USVFS** via FFI Rust | Éprouvé par MO2 ; FFI Rust vers ABI C plus simple qu'un P/Invoke C# |
| **Archives** | 🔬 `sevenz-rust` (piste, à confirmer en spike) | Éviterait la dépendance à `7z.dll` |
| **IA locale** | GGUF via binding Rust de llama.cpp 🔬 (`llama-cpp-2` ou `candle` à trancher en spike) | Qwen2.5-1.5B par défaut, cf. §6 |
| **MCP** | SDK MCP officiel (Rust si mûr, sinon wrapper) 🔬 | À vérifier en Phase 0 — le SDK C# existe et est mûr, l'équivalent Rust doit être vérifié |
| **Nexus / CurseForge** | Providers séparés, API officielles | `nxm://` pour Nexus (pas Premium), API CurseForge pour Minecraft |
| **Tests** | Fixtures "faux Skyrim/Minecraft", tests d'intégration sur le Core | La sécurité du moteur passe par là, pas par les tests UI |
| **Versioning** | `git init` local | Pas de remote GitHub pour l'instant (§2) |
| **Licence** | GPL-3 | Cohérent avec USVFS/libloot/l'écosystème |

---

## 9. Outils externes à orchestrer (Skyrim, inchangé)

| Outil | Rôle | Intégration |
|---|---|---|
| SKSE | Script extender | Détection version, lancement, diagnostic DLL |
| LOOT | Tri du load order | **Natif** — libloot est Rust |
| xEdit | Conflits au niveau des records | Orchestré par arguments/scripts (§4) |
| Nemesis / Pandora | Animations | Lancement, capture en "mod généré" |
| DynDOLOD / TexGen / xLODGen | Génération LOD | Idem + suivi de job |
| BodySlide | Meshes corps/tenues | Idem |
| Wrye Bash | Bashed patch | Lancement |
| Crash Logger SSE / Trainwreck | Logs de crash | Parsing → hypothèses classées, croisé avec l'IA locale (§6) |
| ENB / ReShade / Root Builder | Fichiers à la racine du jeu | Backend "racine" dédié, ownership tracé |

Minecraft (Phase 2) : Java, Fabric/Forge/NeoForge/Quilt, `.jar`, CurseForge Provider. Witcher 3 (Phase 3+) : Script Merger, REDmod.

---

## 10. Design system & UI

**La maquette HTML est la source de vérité visuelle** — ce document ne la reformule pas. Style à conserver tel quel (thème sombre en couches façon GitHub/Claude Code/Cursor, cadre/feuille/flottants, tokens définis dans la maquette). Règles UI actées séparément et à respecter par tout futur agent front :

- Les noms longs ne passent jamais sur deux lignes et ne sont jamais tronqués par "…" — ils s'estompent (fondu) sous ce qui est à leur droite.
- Les cartes d'instance sont des lignes basses (~58 px) avec une vignette carrée à gauche (réservée à une image future).
- L'étoile favori est exposée partout où une instance apparaît ; les favoris sont toujours épinglés en premier.
- Le header de jeu (dashboard) est plein cadre, sans carte ni bordure — juste le dégradé couleur du jeu.

💬 **Q18 (table de mods) reste ouverte à dessein :** l'idée à creuser plus tard est une UX inspirée d'un gestionnaire de tâches (Asana/ClickUp) pour la table de mods et les espaces associés — mais **la base de départ reste le tableau actuel de la maquette HTML**, à ne pas changer avant d'avoir exploré cette piste posément.

---

## 11. L'équipe virtuelle → agents Claude Code (mis à jour pour Rust)

| Agent | Poste | Mission | Modèle |
|---|---|---|---|
| `pm` | Chef·fe de projet | ClickUp, jalons, décisions à trancher, point hebdo | Sonnet |
| `architect` | Architecte / tech lead | Contrats Rust, ADR, revue de PR, garde-fou "base top" | Opus |
| `core-dev` | Dev moteur **Rust** | Store, transactions, révisions, backends de déploiement, API | Opus |
| `skyrim-dev` | Dev intégration jeu & outils | Adaptateur Skyrim, orchestration xEdit/LOOT/Nemesis/DynDOLOD, FFI USVFS | Opus |
| `mcp-dev` | Dev MCP & automatisation | Serveur MCP, permissions 3-niveaux, recettes | Opus |
| `ui-designer` | Designer UI/UX | Maquette HTML, tokens, revue visuelle | Opus |
| `frontend-dev` | Dev front | React/TS, composants, IPC Tauri, tests | Opus |
| `qa` | QA / test | Fixtures "faux Skyrim", tests moteur Rust | Sonnet (Opus si dur) |

Optionnels plus tard : `devops`, `scribe`. Budget modèles confirmé : **gros blocs en Opus**, Sonnet pour `pm`/`qa`.

---

## 12. Suivi de projet : ClickUp (confirmé)

Plan gratuit, compte déjà existant, connecté. Distinction par **tags** (`pm`, `arch`, `core`, `game`, `mcp`, `design`, `front`, `qa`), pas par champ personnalisé (limite 60 utilisations du plan gratuit). Granularité : **tâches "features" grosses + quelques sous-tâches + jalons**, point hebdo écrit par `pm`. Structure ClickUp inchangée depuis `00-cadrage-technique.md` §10.3 (Décisions & jalons, Roadmap, Phases, Bugs, Idées).

---

## 13. Roadmap par phases (mise à jour Rust)

| Phase | Contenu | Jalon de sortie |
|---|---|---|
| **Phase 0 — Fondations** | `git init` local, `CLAUDE.md`, agents, ClickUp, squelette Core Rust (domaine, SQLite, store, event log), CLI `modlab`, fixtures "faux Skyrim", **spikes : binding llama.cpp Rust, FFI USVFS, orchestration xEdit, `sevenz-rust`, SDK MCP Rust** | **"Squelette vivant"** : la CLI détecte Skyrim, importe une archive, crée un loadout vide, journalise une transaction |
| **Phase 1 — Skyrim jouable** | Adaptateur Skyrim, installation d'archive → mod, loadout, déploiement USVFS + lancement SKSE, conflits fichiers, snapshot/rollback, **import d'instance MO2** | **"Je joue à Skyrim lancé par ModLab avec ma vraie modlist"** |
| **Phase 2 — UI façon IDE + Minecraft** (en parallèle) | Shell React/Tauri, table de mods virtualisée, panneau IA locale, Adaptateur Minecraft (Fabric/Forge, CurseForge Provider) | **"Je n'ouvre plus MO2"** + Minecraft prouve la généralité de l'Adapter |
| **Phase 3 — MCP + CLI parité** | Tools/permissions 3-niveaux, `.mcp.json`, panneau Claude dans l'UI | **"Claude installe un mod et m'explique un conflit"** |
| **Phase 4 — Expert Skyrim** | Conflits de records (évaluer Rust maison vs sidecar, §4), Nexus API complète, diagnostics avancés | **"ModLab voit plus de choses que MO2 + LOOT + xEdit réunis"** |
| **Phase 5 — Superpouvoirs** | Bisect, recettes, KB, session recorder — voir `00-cadrage-technique.md` §8 (inchangé) | **"Je résous un crash sans ouvrir un log à la main"** |
| **Phase 6 — Witcher 3 / autres** | Valide la généralité au-delà de la famille Bethesda | **"Ajouter un jeu n'a rien changé au noyau"** |

---

## 14. Risques (mis à jour)

| Risque | Impact | Mitigation |
|---|---|---|
| **Pas d'équivalent Rust à Mutagen** | Analyse de conflits record-level limitée en v1 | Orchestrer xEdit (§4), réévaluer en Phase 4 |
| Écosystème Rust MCP/llama.cpp moins mûr que C# | Spikes plus longs que prévu en Phase 0 | Time-boxer les spikes, fallback sidecar si un binding bloque |
| USVFS via FFI Rust plus dur que prévu | Bloque le déploiement "propre" | Spike Phase 0, backend hardlink/CoW en plan B, lire le code de MO2 |
| Perte de fichiers par bug moteur | Confiance détruite | Tests d'intégration sur fixtures, corbeille interne, jamais de suppression directe |
| Deux langages (Rust + TS) fatiguent | Vélocité | Génération de types automatisée (Tauri le permet nativement), contrats stables |
| Scope infini (superpouvoirs) | Jamais de v1 | Jalons stricts, feature flags |
| Skyrim se met à jour (versions exe/SKSE) | Casse | Détection de version + diagnostics dès la Phase 1 |

---

## Annexe A — Réponses aux 30 questions (verrouillées le 15 septembre 2026)

| # | Réponse |
|---|---|
| Q1 | Skyrim AE 1.6.1170, Steam, ~400 mods, MO2 utilisé aujourd'hui |
| Q2 | NVMe + SSD (projet actuellement dessus) + 2 HDD |
| Q3 | Pas Nexus Premium — `nxm://` + éventuellement WebView pour la navigation |
| Q4 | Gros blocs d'abord, puis sessions courtes ; quotidien tant qu'il reste du budget Claude |
| Q5 | Pas besoin de lire/toucher le code personnellement ; stack = Rust + Tauri + React |
| Q6 | Anglais natif dans le code, pensé pour une traduction facile |
| Q7 | Tous les jeux moddables à terme, priorité Skyrim puis Minecraft |
| Q8 | Import MO2 oui, sauf contradiction avec l'architecture retenue (aucune) |
| Q9 | Layout compatible MO2 oui (aucune contradiction) |
| Q10 | Collections Nexus intéressantes mais pas prioritaires ; envisager un système ModLab natif |
| Q11 | FOMOD guidé complet, mais l'utilisateur expert doit toujours pouvoir faire ses propres choix |
| Q12 | Tous les outils de modding utiles ; accès rapide depuis le tableau de bord si pas d'intégration profonde |
| Q13 | GPL-3 dès maintenant (reco actée) |
| Q14 | La maquette ne doit pratiquement pas changer ; tout écart nécessite validation ciblée |
| Q15 | Thème sombre uniquement pour l'instant |
| Q16 | Maquette HTML déjà en place, c'est la base |
| Q17 | Style de la maquette conservé tel quel ; nom "ModLab" provisoire mais utilisé pour l'instant |
| Q18 | 💬 Ouvert — inspiration future façon Asana/ClickUp pour la table de mods, mais base = table actuelle de la maquette |
| Q19–21 | Web/React (Tauri, pas WebView2+WPF) ; USVFS par défaut ; loadout-as-code (git interne) — oui |
| Q22 | Racine ModLab : **`F:\ModLab`** |
| Q23 | Spikes jetables autorisés, dans `spikes/`, jamais mergés |
| Q24–25 | Permissions à 3 niveaux (§7.2) ; bisect/lancement automatique = mutation réversible opt-in par recette |
| Q26 | MCP générique, utilisable par Claude Code, Claude Desktop, et toute IA compatible MCP |
| Q27 | ClickUp gratuit, compte déjà existant et connecté |
| Q28 | Tâches features + sous-tâches + jalons, point hebdo par `pm` |
| Q29 | Repo GitHub existant mais **local pour l'instant**, `git init` local dès la Phase 0 |
| Q30 | Gros blocs en Opus, Sonnet pour `pm`/`qa` |

---

## Annexe B — Glossaire

Voir `00-cadrage-technique.md` Annexe A pour les termes déjà définis (ESP/ESM/ESL, Master, Load order, Record, BSA/BA2, FOMOD, VFS/USVFS, CAS, Loadout, Révision, Transaction…) et pour GGUF/Quantization/VRAM/GBNF (ajoutés le 15 septembre). Nouveaux termes :

| Terme | Définition |
|---|---|
| **Game Adapter** | Composant qui traduit les primitives génériques du Core dans le vocabulaire d'un jeu précis (Skyrim, Minecraft…). Le Core ne connaît aucun jeu directement. |
| **Provider** | Composant qui traduit les primitives génériques du Core vers une plateforme de mods (Nexus, CurseForge…). |
| **FFI** (Foreign Function Interface) | Mécanisme permettant à du code Rust d'appeler une bibliothèque écrite dans un autre langage (C/C++) — utilisé pour USVFS. |
| **Sidecar** | Petit processus séparé, dans un autre langage, appelé en local par le Core pour une tâche précise qu'il ne peut pas faire lui-même (ex. un futur sidecar .NET/Mutagen). |

---

## Annexe C — Sources

Cumul de `00-cadrage-technique.md` Annexe B et des sources IA locale ajoutées le 15 septembre 2026, plus :

- `Modlab — Document de cadrage & architecture.md` (document de réflexion, 15 septembre 2026)
- esplugin (Rust, lecture de plugins Bethesda) : https://github.com/loot/esplugin
- libloot réécrit en Rust (2025) : https://blog.ortham.net/posts/2025-04-24-porting-libloot-to-rust/
- sevenz-rust : https://github.com/dyz1990/sevenz-rust
