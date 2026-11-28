# Crates Rust

Le moteur de ModLab et ses clients, en workspace Cargo (`Cargo.toml` à la racine). Architecture et décisions : `.bframe/ARCHITECTURE.md` et `.bframe/DECISIONS.md`.

| Crate | Rôle |
|---|---|
| `modlab-core` | Le moteur : source unique de l'état, contrats, transactions |
| `modlab-game-skyrim` | Adaptateur Skyrim (tout ce qui est propre à Bethesda) |
| `modlab-game-minecraft` | Adaptateur Minecraft (loaders, jars, lancement, compte Microsoft) |
| `modlab-ai` | Couche IA locale : diagnostic par règles, reformulation optionnelle |
| `modlab-cli` | La CLI `modlab`, client du moteur |
| `modlab-ui` | Hôte Tauri de l'interface (`ui/`), client du moteur |

## Lancer et tester

Depuis la racine du dépôt :

- `cargo check --workspace` : vérifie que tout compile.
- `cargo test --workspace` : lance les tests. Ils tournent sur `fixtures/`, jamais sur un vrai jeu ni sur `F:\ModLab`.
- `cargo run -p modlab-cli -- --help` : liste les commandes de la CLI.
- L'application complète se lance depuis `ui/` (voir `ui/README.md`).

Avant de committer : la section « Avant de committer » de `CLAUDE.md`.

## `modlab-core`

Moteur Rust unique : **SQLite + journal d'événements = vérité**. L'UI, la CLI et le MCP n'écrivent l'état que par lui. Aucune logique Skyrim, Minecraft, Nexus ou CurseForge ici, seulement des primitives et des contrats (`GameAdapter`, transactions, loadouts).

- `transaction` : cycle plan → snapshot → apply → rollback. `store` : magasin de contenu adressé par hash (CAS).
- `loadout`, `instance_root`, `install`, `archive`, `download` : profils, racines d'instance, installation, archives, téléchargements.
- `game_adapter` : le trait d'adaptateur de jeu, implémenté par les crates `modlab-game-*`.
- `conflicts`, `mod_meta`, `mod_config`, `mod_measure`, `filename_meta`, `files`, `data_root` : conflits, métadonnées, taille et date d'installation, nom de fichier de téléchargement, fichiers, racine des données.
- Tests : `tests/fixtures.rs`. La sécurité des fichiers est non négociable ; l'intégration se fait sur fixtures uniquement.

À ne pas faire : ajouter une règle propre à un jeu ou à une plateforme ; contourner le cycle de transaction pour écrire sur disque ou en base ; tester contre une installation réelle.

## `modlab-game-skyrim`

Les règles propres à Skyrim, que le Core ne connaît pas.

- `engine` : le moteur de Skyrim (SKSE). `fomod` : installateurs FOMOD. `graphics` : réglages graphiques. `installer` : classement des fichiers d'une archive.
- `mo2` : import d'un profil MO2 (lecture seule). `usvfs` : déploiement par USVFS.
- `archive_meta` : infos lues dans `fomod/info.xml` et `meta.ini`. `categories` : liste fixe de catégories Skyrim. `dependencies` : dépendances manquantes ou désactivées d'un mod.
- `plugins` : ordre de chargement et `plugins.txt`. `skse` : détection et rapport de session SKSE. `tool_archive` : reconnaît SKSE parmi les mods. `vanilla` : ce qui n'appartient pas à un `Data/` vanilla.

## `modlab-game-minecraft`

Tout ce qui est **propre à Minecraft** : détection du jeu, jars dans `mods/`, dossier d'instance `.<slug>`, loaders par branche. Branché sur le contrat `GameAdapter` du Core (les jars sont déployés tels quels, sans extraction).

- `launch` : chemin **Jouer**. `prepare_launch` (vanilla et loader choisi installés, libs, assets, runtime et **Java**) → `spawn_launch` → `wait_launch_cancellable` (flux stdout/stderr par callback, `game://log`). Authentification : compte Microsoft en ligne si une session est valide (`OnlineAuth`), sinon solo hors ligne (`player_name`). Aucune dépendance à CurseForge pour Jouer. Loader choisi mais absent : erreur en français qui renvoie à « Installer … ». Substitue aussi `${library_directory}` et `${classpath_separator}` (sinon NeoForge FML répond « installation is corrupted »).
  - **Tester le lancement** (smoke) : même préparation, lancement et journaux, avec `apply_smoke_launch_tweaks` (fenêtre 1×1, pas de GLFW headless sous Windows). `wait_smoke_launch` attend le marqueur de menu (`Sound engine started`, Fabric « completed loading », etc.), un crash ou un timeout de 120 s, puis tue la JVM.
- `msa` : OAuth **code d'autorisation + PKCE** (loopback `http://127.0.0.1:<port>/`) → Xbox → Minecraft Services (protocole HTTP seulement). Pas de device code (AADSTS70002 fréquent sur les comptes perso). Client id : **par défaut le client public Prism** (`c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb`, allowlisté par Mojang, aka.ms/AppRegInfo) ; surcharge possible dans Réglages → Compte ou par la variable `MODLAB_MSA_CLIENT_ID`. Une App Registration Azure perso (Graph User.Read) **ne peut pas** faire Xbox/MC. Les jetons ne sont **pas** stockés ici : chiffrement DPAPI dans `msa_store` (crate `modlab-ui`).
- `loader_install` : installation en un clic depuis les **maven et meta officiels**, dans le dossier d'instance (`.<slug>/versions`, `libraries`). Fabric/Quilt par profil JSON, Forge/NeoForge par l'installeur en ligne de commande, vanilla par le manifeste Mojang. Tout loader non vanilla passe par `ensure_vanilla` (même pipeline que Jouer) puis `assert_loader_version_for_mc` (la version du loader doit exister pour la version de Minecraft choisie).
  - Forge/NeoForge : cwd = `cache/minecraft-installers`, jar passé en nom relatif, chemins absolus **sans** préfixe Windows `\\?\` (sinon `java -jar` → ClassNotFoundException `SimpleInstaller`). Cela évite aussi les `*-installer.jar.log` dans `crates/modlab-ui` sous `tauri:dev`.
  - `versions_installed` : Forge = jars « installeur only » (URL vide) ; NeoForge moderne = `minecraft-client-patched` + `universal` (sortie des processors). `install(..., force)` : efface la version et les jars des processors, puis relance. Listes sur disque : `list_installed_game_versions` et `list_installed_loader_versions`.
  - **Java** : recherche dans `.minecraft/runtime`, CurseForge `Install/runtime` et `Install/java`, Adoptium/Oracle/Microsoft, `JAVA_HOME`/PATH ; si la version majeure manque, téléchargement d'Adoptium dans `cache/java-runtimes/major-<N>/`.
- `official_launcher` : **secours** (« Ouvrir le launcher »). Crée ou met à jour le profil ModLab (`gameDir` = instance) puis lance CurseForge `Install\minecraft.exe` ou `MinecraftLauncher.exe`. **Jamais** l'URI Store `minecraft:`. Les emplacements `launcher_exe` et `root` servent ce chemin, pas Jouer.
- `instance` : lecture des manifestes jar, import d'instance, copie de mod sans écrasement. Activer ou désactiver un mod : renommer `foo.jar` ↔ `foo.jar.disabled` dans `mods/` (convention des launchers, déjà lue par `scan_dir`), puis mise à jour de `source_path` et du drapeau de loadout. Case à cocher de l'UI → `set_mod_enabled` → `instance::set_mod_enabled`. Les lignes de loader virtuelles (`modlab:branch-mc-loader:…`, module `branch_loader`) ne touchent que la base.
- `install` (installe un fichier téléchargé), `archive_meta` (faits lus dans un jar), `tool_archive` (le mod est-il le loader ?), `categories`, `dependencies`, `filename` (ce que dit le nom d'un jar).
- Tests : `tests/fixtures.rs` ; les jars de test sont construits à l'exécution. Jamais sur une vraie installation (utiliser `fixtures/fake-minecraft`).

Chemins connus (machine de Benjamin) : app CurseForge `%LOCALAPPDATA%\Programs\CurseForge Windows\CurseForge.exe` ; launcher Minecraft embarqué (secours) `%USERPROFILE%\curseforge\minecraft\Install\minecraft.exe` ; profils et versions CurseForge `...\Install\launcher_profiles.json` et `...\Install\versions\`.

Pas encore : fournisseurs CurseForge et Modrinth ; désinstallation d'un jar (hors renommage de désactivation) ; multi-comptes et Realms.

À ne pas faire : remonter loaders, jars ou chemins de launcher dans le Core ; écraser un jar existant ; faire dépendre Jouer du launcher CurseForge ou officiel.

## `modlab-ai`

Produit les réponses de l'assistant à partir de faits fournis par l'hôte. Le moteur de règles établit le diagnostic ; le modèle local, optionnel, ne fait que le reformuler.

- **Diagnostic** : instance vide, mods tous désactivés, mods écrasés et dossiers illisibles, avec trois niveaux de gravité (`src/lib.rs`).
- **Réponse sans modèle** : une réponse en français construite à partir des faits. C'est la réponse par défaut.
- **Reformulation par un modèle local** (`src/llama.rs`) : passe par llama.cpp, avec l'option `local-llm`, désactivée par défaut (`cargo build --features local-llm` dans ce crate). Sa compilation demande LLVM et CMake.
- Appel depuis l'interface : commande `ai_ask` dans `modlab-ui`.

À ne pas faire : ajouter un accès au Core ou au disque (les faits viennent toujours de l'hôte) ; laisser le modèle décider ou ajouter un fait (aujourd'hui, ce n'est qu'une consigne dans le texte envoyé au modèle : la sortie contrainte n'existe pas encore) ; lancer le modèle pendant qu'un jeu tourne.

## `modlab-cli`

La CLI `modlab` (`src/main.rs`, `clap`), client du moteur. Commandes : `detect`, `mo2-profile`, `mod-import`, `mo2-import`, `loadout-create`, `loadout-conflicts`, `mc-scan`, `mc-import`, `mc-install`, `archive-inspect`, `usvfs-check`, `skse-launch`.

- Sans `--data-root`, elle écrit dans le dossier d'essai `.modlab-dev` (ignoré par Git), jamais dans `F:\ModLab`.
- Essai sans risque : `cargo run -p modlab-cli -- mc-scan fixtures/fake-minecraft`.
- Attention : `usvfs-check` et `skse-launch` agissent sur le vrai Skyrim (`skse-launch` ouvre le jeu) ; ne les lancer que volontairement.

## `modlab-ui`

Fait tourner l'interface React (`ui/`) dans une application de bureau Tauri 2. Il relie l'interface au moteur et aux adaptateurs de jeu, et fournit un navigateur intégré (second WebView2, car Nexus envoie `X-Frame-Options`).

- `src/lib.rs` : assemblage de l'hôte (état, enregistrement des commandes). `src/main.rs` : point d'entrée.
- `src/commands/` : les commandes Tauri, un module par domaine (`assistant`, `browser`, `downloads`, `launch`, `loadouts`, `mc_loader`, `mods`, `msa`, `paths`, `settings`, `shell`).
- `src/browser.rs` : navigateur intégré. `src/downloads.rs`, `src/download_progress.rs`, `src/source_page.rs` : téléchargements, progression, page source.
- `src/paths.rs`, `src/ipc_paths.rs` : chemins du jeu et bornage des chemins reçus de l'interface. `src/msa_store.rs` : session Microsoft chiffrée.
- `tauri.conf.json`, `capabilities/`, `build.rs`, `icons/` : configuration de Tauri.
- `.modlab-dev/` (données d'essai) est ignoré par Git.

Lancement : `ui/README.md`.
