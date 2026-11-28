# Spikes

Des explorations jetables, limitées dans le temps, pour lever les risques signalés dans `.bframe/ARCHITECTURE.md`. Leur code n'est jamais fusionné dans `crates/`. Les décisions en vigueur sont dans `.bframe/DECISIONS.md`.

> Note du 2026-11-28 : les résultats ci-dessous sont l'historique de septembre. Depuis, l'USVFS est implémenté (`crates/modlab-game-skyrim/src/usvfs.rs`) et `rmcp` n'a pas été adopté (absent de `Cargo.lock`).

## Contenu

- `sevenz-rust/` : extraction d'une archive `.7z` en Rust pur (`src/main.rs`).
- `llama-binding/` : le binding `llama-cpp-2` (`src/main.rs`).

Chaque spike est son propre workspace Cargo, hors du workspace racine. Pour en lancer un : `cargo run` depuis son dossier. Leurs `target/` sont ignorés par Git.

## sevenz-rust : résolu, 2026-09-15

**Question :** peut-on extraire des `.7z` en Rust pur, sans dépendre de `7z.dll` ?

**Résultat :** oui. `spikes/sevenz-rust/` extrait proprement une vraie archive SKSE (129 fichiers) avec le crate `sevenz-rust`, sans DLL externe. Le `.zip` est un problème résolu à part (crate `zip`). Le RAR n'a pas été essayé : part plus faible des archives Nexus, pas bloquant.

**Décision :** `sevenz-rust` + `zip` pour les archives ; revoir le RAR seulement si cela compte en pratique.

## USVFS FFI : faisable, 2026-09-15

**Question :** Rust peut-il appeler `usvfs_x64.dll` (C++, utilisée par MO2) sans couche FFI pénible ?

**Résultat :** `dumpbin /EXPORTS` sur la vraie `usvfs_x64.dll` montre deux familles d'exports : des symboles C++ mangés (non appelables directement) et une **API C claire** (`usvfsCreateVFS`, `usvfsConnectVFS`, `usvfsCreateParameters`, `usvfsCreateHookContext`, `CreateVFS`, `ConnectVFS`, `InitHooks`, `InitLogging`…), une vraie surface publique voulue.

**Décision :** FFI en `extern "C"` avec `libloading` (dynamique) ou une bibliothèque d'import générée. Le spike prouvait seulement la faisabilité ; l'implémentation existe depuis.

## MCP Rust SDK : résolu, 2026-09-15

**Question :** existe-t-il un SDK MCP Rust assez mûr ?

**Résultat :** oui, `rmcp` (SDK officiel, `github.com/modelcontextprotocol/rust-sdk`, runtime tokio). `rust-mcp-sdk` (communautaire) est une alternative solide.

**Décision de l'époque :** utiliser `rmcp`. Non adopté à ce jour (voir la note ci-dessus).

## Binding llama.cpp : résolu, 2026-09-15

**Question :** `llama-cpp-2` (binding fin) ou `candle` (réimplémentation en Rust pur) ?

**Résultat :** `llama-cpp-2` compile et se lie proprement (build C++ complet de llama.cpp, environ 2 min depuis zéro). Il a fallu deux dépendances machine, installées via winget : **LLVM** (`libclang.dll`, pour `bindgen`) et **CMake**.

**Décision :** `llama-cpp-2`. Conséquence : toute machine qui compile les binaires de release de ModLab avec `local-llm` doit avoir LLVM et CMake (dépendances de build seulement, pas d'exécution : `llama-cpp-sys-2` lie statiquement).

## xEdit : bloqué, 2026-09-15

xEdit n'est pas installé seul sur la machine de Benjamin (LOOT et MO2 y sont, pas xEdit). L'orchestration (arguments `-autoload -autoexit -veryquickshowconflicts` + scripts Pascal, capture de la sortie) suit le même schéma que MO2, mais ne peut pas être vérifiée de bout en bout sans le binaire.
