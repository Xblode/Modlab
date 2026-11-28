# ModLab

Gestionnaire d'environnements de jeu personnel, Skyrim d'abord et multi-jeux : une plateforme centralisée de modding, à l'interface épurée et moderne, pas un MO2 plus joli. Projet de long terme, construit avec Claude Code.

## Source de vérité : à lire avant toute action structurante

- **Le code fait foi.** Les docs le décrivent ; quand elles divergent, le code gagne et la doc est corrigée.
- **Toute la documentation vit dans `.bframe/`**, en français. Lis d'abord `.bframe/ETAT.md`, puis utilise la carte « Où est quoi » de `.bframe/PROJET.md` pour trouver le bon fichier (vision, produit, architecture, design, roadmap, idées, tâches) : ne recopie pas cette liste ici. Hors `.bframe/`, il ne reste que les README, `CLAUDE.md` et `AGENTS.md`.
- **`ui/`** (styles dans `ui/src/shell.css`) est la référence visuelle de l'interface ; les règles d'interface sont dans `.bframe/DESIGN.md`. On ne redessine pas au fil de l'eau : tout changement visuel passe d'abord par l'humain.
- **Les décisions** sont consignées dans `.bframe/DECISIONS.md`, qui fait office de journal d'ADR (pas de fichiers ADR séparés). Toute proposition qui s'écarte d'une décision enregistrée est signalée à l'humain avant validation.
- **`.bframe/archive/**` n'est jamais lu**, sauf demande explicite de l'humain. C'est un historique réservé à l'humain ; ne jamais l'implémenter comme s'il était d'actualité.
- Après toute avancée notable (livraison d'un worker, correction structurelle), l'orchestrateur **met à jour `.bframe/ETAT.md` dans le même tour** (ligne de sujet + `prochaine_action` si besoin) : ne pas dupliquer l'état du projet dans le chat.

### Portes d'entrée du code (à lire plutôt que de fouiller l'arbre)

- **Crates Rust** (Core, adaptateurs Skyrim et Minecraft, IA, CLI, hôte Tauri) : `crates/README.md`
- **Interface et shell UI** (dont le tableau « un fait = un propriétaire ») : `ui/README.md`
- **Faux jeux de test** : `fixtures/README.md`

## Stack (actuelle)

- **Core** : Rust. Il possède tout l'état (cible : SQLite via un seul processus écrivain, non garanti aujourd'hui, voir `.bframe/ARCHITECTURE.md` §7 ; « un moteur, plusieurs clients » : l'UI, la CLI et le MCP sont tous des clients du même Core et n'inventent jamais leur propre état).
- **UI** : React + TypeScript + Vite, dans une coque **Tauri** (WebView2 sous Windows).
- **Racine des données** : résolue par `data_root.rs` (`F:\ModLab` en usage réel) : les vraies données de mods (archives, mods déployés, base SQLite). Ce dépôt ne contient que le code source et les docs ; ne jamais mélanger les deux.
- **Risque connu** : il n'existe pas encore d'équivalent Rust mûr de Mutagen pour lire et écrire les enregistrements typés des plugins Bethesda. Le plan v1 est d'orchestrer le vrai xEdit (comme MO2 aujourd'hui) : voir `.bframe/ARCHITECTURE.md` §8 (risques) avant de toucher à l'analyse des plugins ou des enregistrements.

## Conventions

- **Code et commentaires en anglais.** Docs et toutes les chaînes affichées à l'écran en français.
- Git : le remote `origin` (GitHub) existe volontairement (2026-11-13). **Ne jamais pousser sans demande explicite** : c'est une contrainte permanente, pas une préférence ponctuelle.
- Les tâches vivent dans `.bframe/TACHES.md` (la seule liste de tâches). L'état du projet vit dans `.bframe/ETAT.md`, pas ici.
- Toute mutation suit le cycle transactionnel : plan → approbation → snapshot → application → vérification → commit (ou rollback automatique en cas d'échec). Ne jamais écrire directement dans les dossiers du jeu ni supprimer quoi que ce soit pour de bon : tout est réversible, avec une corbeille interne au lieu d'une vraie suppression.
- Contrat d'abord pour tout ce qui est multi-jeux : le Core ne connaît jamais un jeu précis, c'est un `GameAdapter` qui le connaît. Idem pour les plateformes de mods : le Core ne connaît ni Nexus ni CurseForge, c'est un `Provider`.

## Tests

Moteur d'abord : tests d'intégration Rust (équivalent xUnit) contre des fixtures (un « faux Skyrim » : dossier `Data/` minimal, plugins et archives synthétiques), jamais contre le vrai jeu. Les tests d'interface viendront plus tard. Un gestionnaire de mods qui perd un fichier est inutilisable : les tests de sécurité des fichiers du Core sont non négociables, le reste peut glisser.

## Avant de committer

- Rust : `cargo fmt -- --check`, `cargo clippy`, puis `cargo check && cargo test` : tout doit passer.
- Frontend : `npm install` (si les dépendances ont changé) puis `npm run build` : il doit passer.
- Ne jamais committer si `cargo check` ou `npm run build` échoue.
- Les messages de commit suivent Conventional Commits : `<type>(scope): description` (ex. `feat(profile): ...`, `fix(game-adapter): ...`).

## Ce qu'on ne fait jamais

- Ne jamais ouvrir, lire ni citer `.bframe/archive/**` pour décider ou implémenter : historique dépassé, réservé à l'humain sur demande. La réponse, c'est `.bframe/DECISIONS.md`.
- Ne jamais pousser sur GitHub, ne jamais installer ni mettre à jour d'outil global, ne jamais toucher à quoi que ce soit hors de ce dépôt ou de `F:\ModLab` sans demander d'abord.
- Ne jamais contourner les protections de Nexus (téléchargements automatisés sans Premium, scraping) : c'est fragile et ça fait bannir les comptes, ce n'est pas qu'une question de politique.
- Ne jamais donner à la couche IA/MCP un accès direct au système de fichiers ou au shell : toujours `IA → MCP → niveaux de permission ModLab → Core` (3 niveaux de permission).

## Équipe

bFrame pilote le travail. Cette conversation est l'orchestrateur et délègue à trois workers : `worker-sonnet`, `worker-haiku`, `worker-web`. Les huit prompts de spécialistes (`pm`, `architect`, `core-dev`, `skyrim-dev`, `mcp-dev`, `ui-designer`, `frontend-dev`, `qa`) ont été retirés le 2026-11-24. Les tâches sont suivies dans `.bframe/TACHES.md`.
