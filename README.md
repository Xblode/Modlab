# ModLab

ModLab est un gestionnaire de mods personnel : un atelier où l'IA comprend et manipule vraiment le jeu, sans jamais agir en cachette (chaque action dit quoi, pourquoi, quels fichiers, et comment revenir en arrière).
Il vise d'abord Skyrim SE, puis d'autres jeux (Minecraft ensuite) dans la même app.
L'idée : « Cursor pour le game modding ».

## Où sont les choses

| Dossier | Rôle |
|---|---|
| `crates/modlab-core/` | Le moteur : source unique de vérité, l'UI, la CLI et le MCP en sont des clients |
| `crates/modlab-cli/` | La CLI `modlab`, client du moteur |
| `crates/modlab-ui/` | L'hôte Tauri de l'interface, client du moteur |
| `crates/modlab-ai/` | La couche IA locale : un moteur de règles diagnostique, un LLM optionnel ne fait que reformuler |
| `crates/modlab-game-skyrim/` | L'adaptateur Skyrim (tout ce qui est spécifique à Bethesda) |
| `crates/modlab-game-minecraft/` | L'adaptateur Minecraft (loaders, jars, fournisseurs) |
| `ui/` | L'interface React (Vite) |
| `docs/` | La documentation, l'architecture et les décisions (ADR) |
| `fixtures/` | De faux jeux (Skyrim, Minecraft) pour les tests |
| `spikes/` | Essais techniques jetables (binding llama, 7z) |
| `.bframe/` | La mémoire du projet (cap, décisions) |
| `.claude/agents/` | Les agents spécialisés de Claude Code |

## Ce qui fait foi

- Architecture : `docs/01-architecture-decidee.md`
- Interface : `docs/design/shell-maquette.html`

## Lancer et tester

### Ce qu'il faut avoir installé

- Rust (avec `cargo`).
- Node.js (avec `npm`) : le projet utilise npm, pas pnpm ni yarn.
- Sous Windows : les outils de compilation C++ de Visual Studio et WebView2 (déjà présent sur Windows 11), requis par Tauri 2.
- Seulement pour l'option d'IA locale (`local-llm`, désactivée par défaut) : LLVM et CMake.

### Lancer le logiciel (application complète)

Depuis le dossier `ui/` (la première fois, installer les dépendances de l'interface) :

```
npm install
npm run tauri:dev
```

- `npm install` télécharge les dépendances de l'interface (une seule fois, ou quand `package.json` change).
- `npm run tauri:dev` compile l'hôte Tauri et ouvre la fenêtre ModLab, avec l'interface qui se recharge à chaque modification. La première compilation est longue.

### Lancer seulement l'interface ou la maquette

```
npm --prefix ui run dev
python -m http.server 5502
```

- La première commande, depuis la racine du dépôt, sert l'interface seule dans le navigateur sur `http://localhost:1420` (sans le moteur : les appels à l'hôte Tauri ne répondent pas). Arrêt avec `Ctrl+C`.
- La seconde, depuis la racine du dépôt, sert la maquette sur `http://localhost:5502/docs/design/shell-maquette.html` (c'est le serveur `maquette` de `.claude/launch.json`). Arrêt avec `Ctrl+C`.

### La CLI

Depuis la racine du dépôt :

```
cargo run -p modlab-cli -- --help
cargo run -p modlab-cli -- detect
cargo run -p modlab-cli -- mc-scan fixtures/fake-minecraft
```

- `--help` liste toutes les commandes.
- `detect` cherche les jeux installés (lecture seule).
- `mc-scan` liste les mods d'une instance Minecraft ; ici, sur le faux jeu de test (lecture seule).
- Sans `--data-root`, la CLI écrit dans le dossier d'essai `.modlab-dev`, jamais dans `F:\ModLab`.
- Attention : `usvfs-check` et `skse-launch` agissent sur le vrai Skyrim (`skse-launch` ouvre le jeu) ; ne les lancer que volontairement.

### Tester

Depuis la racine du dépôt :

```
cargo test --workspace
```

- Lance tous les tests Rust du projet.

Depuis le dossier `ui/` :

```
npm run build
```

- Vérifie les types TypeScript puis construit l'interface (il n'y a pas encore de tests d'interface).

Avant de valider un changement (`cargo fmt -- --check`, `cargo clippy`, `cargo check && cargo test`, `npm run build`) : suivre la section « Before committing » de `CLAUDE.md`.

Les tests tournent sur les faux jeux de `fixtures/`, jamais sur le vrai jeu ni sur `F:\ModLab`.

## Données

Les données réelles des mods vivent dans `F:\ModLab`, jamais dans ce dépôt.
