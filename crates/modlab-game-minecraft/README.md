# Porte d'entrée — adaptateur Minecraft

## À quoi sert ce crate

Tout ce qui est **propre à Minecraft** : détection du jeu, jars dans `mods/`, dossier d'instance `.<slug>`, loaders par branche. Branché sur le contrat `GameAdapter` du Core (deploy-as-is, pas d'extraction des jars).

## Fichiers / modules clés

- `launch` : **chemin Jouer** — `prepare_launch` (ensure vanilla + loader installé si choisi, libs/assets/runtime + **Java**) → `spawn_launch` → `wait_launch_cancellable` (stream stdout/stderr via callback / `game://log`). Auth = MSA en ligne si session valide (`OnlineAuth`), sinon solo hors-ligne (`player_name`). Pas de dépendance CurseForge pour Jouer. Loader choisi mais absent → erreur FR pointant « Installer … ». Substitue aussi `${library_directory}` / `${classpath_separator}` (sinon NeoForge FML → « installation is corrupted »). **Tester le lancement** (smoke) : même prepare/spawn/logs, `apply_smoke_launch_tweaks` (fenêtre 1×1 — pas de headless GLFW sous Windows), `wait_smoke_launch` jusqu’au marqueur menu (`Sound engine started` / Fabric completed loading / …) ou crash / timeout 120 s, puis kill JVM.
- `msa` : OAuth **auth-code + PKCE** (loopback `http://127.0.0.1:<port>/`) → Xbox → Minecraft Services (protocole HTTP seulement). Pas de device code (AADSTS70002 fréquent sur comptes perso). Client id : **défaut = client public Prism** (`c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb`, allowlisté Mojang — aka.ms/AppRegInfo) ; surcharge optionnelle Réglages → Compte ou env `MODLAB_MSA_CLIENT_ID`. Une App Registration Azure perso (Graph User.Read) **ne peut pas** faire Xbox/MC. Jetons **non** stockés ici — chiffrement DPAPI dans `msa_store`.
- `loader_install` : installation en un clic depuis les **maven / meta officiels** dans le dossier d'instance (`.<slug>/versions`, `libraries`) — Fabric/Quilt (JSON profile), Forge/NeoForge (installeur CLI), vanilla via manifest Mojang. Tout loader non-vanilla passe par `ensure_vanilla` (même pipeline que Jouer) puis `assert_loader_version_for_mc` (la version loader doit exister pour la MC choisie). Forge/NeoForge : cwd = `cache/minecraft-installers`, jar passé en nom relatif, chemins absolus **sans** préfixe Windows `\\?\` (sinon `java -jar` → ClassNotFoundException `SimpleInstaller`) ; évite aussi les `*-installer.jar.log` dans `crates/modlab-ui` sous `tauri:dev`. `versions_installed` : Forge = jars « installeur only » (URL vide) ; NeoForge moderne = `minecraft-client-patched` + `universal` (sortie processors). `install(..., force)` : wipe version + jars processors puis re-run. Listes disque : `list_installed_game_versions` / `list_installed_loader_versions`. **Java** : recherche `.minecraft/runtime`, CurseForge `Install/runtime` + `Install/java`, Adoptium/Oracle/Microsoft, `JAVA_HOME`/PATH ; si la majeure manque → téléchargement Adoptium dans `cache/java-runtimes/major-<N>/`.
- `official_launcher` : **secours** (« Ouvrir le launcher ») — upsert profil ModLab (`gameDir` = instance) puis spawn CurseForge `Install\minecraft.exe` ou `MinecraftLauncher.exe`. **Jamais** l’URI Store `minecraft:`. Les path slots `launcher_exe` / `root` servent ce chemin, pas Jouer.
- Détection launcher, lecture des manifestes jar, import d'instance, copie de mod sans écrasement.
- Activer / désactiver un mod : rename `foo.jar` ↔ `foo.jar.disabled` dans `mods/` de l'instance (convention launcher, déjà lue par `scan_dir`), puis mise à jour `source_path` + flag loadout. Case UI → `set_mod_enabled` → `instance::set_mod_enabled`. Les lignes loader virtuelles (`modlab:branch-mc-loader:…`) ne touchent que la base.

## Chemins connus (machine de Benjamin)

- CurseForge app : `%LOCALAPPDATA%\Programs\CurseForge Windows\CurseForge.exe`
- Launcher MC embarqué : `%USERPROFILE%\curseforge\minecraft\Install\minecraft.exe` (secours)
- Profils / versions CF : `...\Install\launcher_profiles.json`, `...\Install\versions\`

## Pas encore

- Providers CurseForge/Modrinth ; désinstallation jar (hors rename disable).
- Multi-comptes / Realms.

## Ne pas

- Remonter loaders, jars ou chemins launcher dans le Core.
- Écraser un jar existant ; tester sur une vraie install (utiliser `fixtures/fake-minecraft`).
- Faire dépendre Jouer du launcher CurseForge / officiel.

## Ensuite

- Core : `crates/modlab-core/README.md`.
- Décisions : `.bframe/DECISIONS.md`.
- UI shell : `ui/src/shell/README.md`.
