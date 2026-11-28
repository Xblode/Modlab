# ModLab

ModLab est un gestionnaire de mods personnel : un atelier où l'IA comprend et manipule vraiment le jeu, sans jamais agir en cachette (chaque action dit quoi, pourquoi, quels fichiers, et comment revenir en arrière).
Il vise d'abord Skyrim SE, puis d'autres jeux (Minecraft ensuite) dans la même app.
L'idée : « Cursor pour le game modding ».

## Où sont les choses

| Dossier | Contenu |
|---|---|
| `crates/` | Le moteur Rust, les adaptateurs de jeu (Skyrim, Minecraft), la couche IA, la CLI et l'hôte Tauri : voir `crates/README.md` |
| `ui/` | L'interface React (Vite) et le shell : voir `ui/README.md` |
| `fixtures/` | De faux jeux (Skyrim, Minecraft) pour les tests : voir `fixtures/README.md` |
| `spikes/` | Essais techniques jetables (binding llama, 7z) : voir `spikes/README.md` |
| `.bframe/` | Toute la documentation et la mémoire du projet : voir `.bframe/PROJET.md` |

Le code fait foi. Règles de travail : `CLAUDE.md`. Architecture : `.bframe/ARCHITECTURE.md`. Règles d'interface : `.bframe/DESIGN.md`.

## Lancer et tester

### Ce qu'il faut avoir installé

- Rust (avec `cargo`).
- Node.js (avec `npm`) : le projet utilise npm, pas pnpm ni yarn.
- Sous Windows : les outils de compilation C++ de Visual Studio et WebView2 (déjà présent sur Windows 11), requis par Tauri 2.
- Seulement pour l'option d'IA locale (`local-llm`, désactivée par défaut) : LLVM et CMake.

### Lancer le logiciel (application complète)

Depuis la racine du dépôt, la première fois, installer les dépendances de l'interface :

```powershell
npm --prefix ui install
```

- `npm --prefix ui install` télécharge les dépendances de l'interface (une seule fois, ou quand `ui/package.json` change).

Puis, toujours depuis la racine du dépôt, lancer l'application :

```powershell
npm --prefix ui run tauri:dev
```

- `npm --prefix ui run tauri:dev` compile l'hôte Tauri et ouvre la fenêtre ModLab, avec l'interface qui se recharge à chaque modification. La première compilation est longue.
- Toujours lancer ces commandes dans le dossier ModLab : ailleurs, npm peut tomber sur un autre projet.

### Lancer seulement l'interface

```powershell
npm --prefix ui run dev
```

- La commande ci-dessus, depuis la racine du dépôt, sert l'interface seule dans le navigateur sur `http://localhost:1420` (sans le moteur : les appels à l'hôte Tauri ne répondent pas). Arrêt avec `Ctrl+C`.

### La CLI

Depuis la racine du dépôt, afficher l'aide :

```powershell
cargo run -p modlab-cli -- --help
```

- `--help` liste toutes les commandes.

Chercher les jeux installés :

```powershell
cargo run -p modlab-cli -- detect
```

- `detect` cherche les jeux installés (lecture seule).

Lister les mods du faux jeu de test :

```powershell
cargo run -p modlab-cli -- mc-scan fixtures/fake-minecraft
```

- `mc-scan` liste les mods d'une instance Minecraft ; ici, sur le faux jeu de test (lecture seule).
- Sans `--data-root`, la CLI écrit dans le dossier d'essai `.modlab-dev`, jamais dans `F:\ModLab`.
- Attention : `usvfs-check` et `skse-launch` agissent sur le vrai Skyrim (`skse-launch` ouvre le jeu) ; ne les lancer que volontairement.

### Tester

Depuis la racine du dépôt :

```powershell
cargo test --workspace
```

- Lance tous les tests Rust du projet.

Depuis la racine du dépôt, vérifier les types TypeScript et construire l'interface :

```powershell
npm --prefix ui run build
```

- La commande ci-dessus vérifie les types TypeScript puis construit l'interface (il n'y a pas encore de tests d'interface).

Avant de valider un changement (`cargo fmt -- --check`, `cargo clippy`, `cargo check && cargo test`, `npm --prefix ui run build`) : suivre la section « Avant de committer » de `CLAUDE.md`.

Les tests tournent sur les faux jeux de `fixtures/`, jamais sur le vrai jeu ni sur `F:\ModLab`.

## Données

Les données réelles des mods vivent dans `F:\ModLab`, jamais dans ce dépôt.
