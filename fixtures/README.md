# Fixtures

Les faux jeux des tests : jamais le vrai jeu, jamais `F:\ModLab`. Règles de test : section « Tests » de `CLAUDE.md`.

## `fake-skyrim/`

Un squelette minimal : `SkyrimSE.exe` et un dossier `Data/` avec un plugin (`Fake Plugin.esp`) et une DLL SKSE (`SKSE/Plugins/FakePlugin.dll`), de quoi tester les chemins et la structure.

**Limite actuelle, voulue :** le « plugin » et la « DLL » sont des textes de remplacement, pas de vrais formats binaires. C'est suffisant pour tout ce qui ne regarde que les *chemins et la présence des fichiers* (import dans le store, détection). Ce ne l'est **plus** dès que l'analyse réelle des ESP existera : il faudra alors régénérer de vraies fixtures binaires minimales (sortie de Mutagen, ou construites à la main d'après la spécification ESP), sinon les tests au niveau des enregistrements passeront sur du vide. Celui qui écrit l'analyseur de plugins doit les remplacer.

## `fake-minecraft/`

Même esprit : un squelette de `.minecraft` (`launcher_profiles.json`, `versions/<id>/<id>.json`, `mods/`, `resourcepacks/`, `saves/<monde>/level.dat`), de quoi tester `MinecraftAdapter::detect` et les chemins.

**Même limite, voulue :** les `.jar` et `.zip` d'ici sont des textes de remplacement, pas de vraies archives zip. Les tests d'extraction de métadonnées de `modlab-game-minecraft` (`fabric.mod.json`, `quilt.mod.json`, `mods.toml`) construisent donc de vrais petits zips **à l'exécution**, en Rust (crate `zip`), et non des binaires versionnés ici. Un format binaire réellement lu exige une fixture régénérée d'après ce format, pas un texte avec la bonne extension.

## Utilisation

Essai sans risque avec la CLI : `cargo run -p modlab-cli -- mc-scan fixtures/fake-minecraft` (voir `crates/README.md`). Les tests Rust s'y appuient : `cargo test --workspace`.
