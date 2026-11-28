# ADR 0004 — `llama-cpp-2` retenu comme binding Rust de llama.cpp, au détriment de `candle`

- **Statut :** accepté (rétroactif — décision prise au spike Phase 0, 15 septembre 2026, voir `spikes/llama-binding/`)
- **Portée :** future couche IA locale (§6 de `01-architecture-decidee.md`), dépendance `llama-cpp-2` / `llama-cpp-sys-2`
- **Références :** `docs/01-architecture-decidee.md` §6 (IA locale, verrouillé), §8 (stack : « GGUF via binding Rust de llama.cpp 🔬, `llama-cpp-2` ou `candle` à trancher en spike »), `spikes/README.md`

---

## Contexte

§6 est verrouillé sur le *comportement* : moteur de règles qui diagnostique, LLM local qui ne fait que rédiger (Qwen2.5-1.5B-Instruct par défaut), GGUF, chargement à la demande, jamais pendant le jeu. Ce que §8 laissait ouvert, c'est la bibliothèque d'inférence côté Rust. Le choix historique LLamaSharp était spécifique au stack C# abandonné.

## Options considérées

1. **`candle`** — framework 100 % Rust (HF). Zéro toolchain C++ à installer, mais réimplémentation des modèles : couverture des architectures plus lente, performances CPU souvent en retrait, et aucun lien avec l'écosystème llama.cpp (grammaires GBNF, quantisations, serveurs).
2. **`llama-cpp-2`** — bindings Rust sûrs sur le vrai llama.cpp. Compile et link proprement le C++ (build complet ~2 min). Le format GGUF, les backends (CPU/CUDA/Vulkan) et l'API sont ceux de llama.cpp — l'approche la plus proche de ce que LLamaSharp apportait.

## Décision

**Option 2 — `llama-cpp-2`.** Le spike (`spikes/llama-binding/`) compile et tourne ; pas de motif de retomber sur `candle`. Les modèles visés restent ceux de §6 : Qwen2.5-1.5B par défaut, Llama-3.2-3B / Phi-3.5-mini / Qwen3 8B au choix — seule la bibliothèque d'inférence change, comme prévu.

## Conséquences

**Ce que ça nous donne.**

- Un seul moteur d'inférence, celui de référence de l'écosystème : quantisations et backends suivis sans travail de notre côté, et les contraintes de sortie (GBNF, demandé par §6 pour empêcher le modèle d'inventer des faits) sont natives chez llama.cpp.
- Le choix de modèles et le cycle de vie (lazy-load, idle-unload, file pendant le jeu) restent exactement ceux de §6, verrouillés — cet ADR ne décide que du binding.

**Ce que ça nous coûte.**

1. **Toolchain de dev plus lourde** : construire `llama-cpp-sys-2` exige **LLVM** (pour `libclang.dll`, requis par `bindgen`) et **CMake**. Les deux ont dû être installés sur la machine de dev (`winget`) — toute machine qui build ModLab en aura besoin aussi, à documenter dans le futur CI/build. Ce sont des dépendances de *build* uniquement : `llama-cpp-sys-2` lie statiquement ce qu'il construit, rien n'est requis à l'exécution.
2. **Temps de compilation** : ~2 min de build C++ au premier build (et après purge du cache), à intégrer dans les attentes de boucle de dev.
3. Une dépendance C++ dans un projet Rust — risque de friction sur les mises à jour de llama.cpp, assumé.

## Chemin de sortie (à ne pas faire maintenant)

Si `llama-cpp-2` devenait un jour un blocage (outilchain impossible à provisionner, ABI cassée), la sortie de secours est `candle` — mais elle emporte une réécriture de la couche d'inférence et une perte des grammaires natives. Tant que le spike tourne, cette question est close.
