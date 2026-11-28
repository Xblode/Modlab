# ADR 0005 — Le shell React est piloté par les capabilities déclarées des GameAdapters

- **Statut :** accepté (rétroactif — décision prise en Phase 2, commits `4eba8a1` → `148f3a6`, 16–17 septembre 2026)
- **Portée :** `crates/modlab-core/src/game_adapter.rs` (`GameCapabilities`), `crates/modlab-ui/src/lib.rs`, `ui/src/gameChrome.ts`, `ui/src/App.tsx`, `ui/src/app/dashboard/page.tsx`
- **Références :** `docs/01-architecture-decidee.md` §3 (le Core ne connaît aucun jeu), §13 (Phase 2 : « Minecraft prouve la généralité de l'Adapter »), `CLAUDE.md` (contract-first multi-jeux)

---

## Contexte

La Phase 2 amène le premier second jeu (Minecraft) à côté de Skyrim dans l'UI réelle. La maquette HTML codait les jeux en dur dans son tableau `games` — chaque écran savait implicitement ce qu'un jeu a le droit d'afficher. Porter ça tel quel dans le shell React voudrait dire des `if (gameId === "skyrimse")` dans chaque composant, exactement la fuite de logique jeu que §3 interdit au Core, déplacée côté UI.

## Options considérées

1. **Branchement par identifiant de jeu** : l'UI connaît `skyrimse` et `minecraft` et adapte ses écrans. Rapide, mais chaque nouveau jeu = retouche de tous les composants, et l'UI peut afficher des choses que l'adaptateur ne sait pas faire.
2. **Contrat de capabilities** : chaque `GameAdapter` déclare ce qu'il offre — `sections` (les onglets), `deployment_label`, `script_extender`, `load_order_tool`, `import_source` — et le shell ne rend **que** ce qui est déclaré. Les couleurs, l'art du header et le glyphe restent de la pure donnée de présentation (`gameChrome.ts`), jamais des faits.
3. **Schéma de capabilities découvert à l'exécution** (introspection, plugins dynamiques). Beaucoup plus cher, aucun besoin réel à deux jeux.

## Décision

**Option 2.** Le shell est un moteur de rendu du contrat `GameCapabilities`, sérialisé par l'hôte Tauri :

- Les onglets d'un jeu sont générés par `tabsFor(capabilities)` depuis `sections`, dans l'ordre déclaré par l'adaptateur — l'UI ne décide de rien.
- Le header unifié, les compteurs de sections et l'entrée d'import (`importSource`, ex. `mo2`) ne s'affichent que si l'adaptateur les déclare ; un jeu sans script extender n'a pas de pastille SKSE, un jeu sans outil de tri n'a pas d'entrée LOOT.
- Ajouter un jeu = un adapter + ses capabilities + une entrée de chrome visuel. **Aucun composant du shell ne change.** Minecraft l'a prouvé : son adaptation n'a demandé que du côté adaptateur.

## Conséquences

**Ce que ça nous donne.**

- L'UI ne peut plus mentir : ce qui n'est pas déclaré n'existe pas à l'écran. La convention inverse — afficher un écran vide en prétendant qu'il marche — est explicitement bannie (`StubTab` dit *pourquoi* il est vide).
- Le vocabulaire français des onglets reste centralisé (`TAB_META` dans `gameChrome.ts`) : l'adaptateur déclare des identifiants de sections, pas des libellés.
- Le sélecteur de révision « r.— » et autres placeholders honnêtes : ce que le Core n'expose pas encore est rendu comme placeholder explicite, jamais comme donnée.

**Ce que ça nous coûte.**

1. **Le contrat évolue avec précaution** : ajouter un champ à `GameCapabilities` est un changement d'API multi-crates (Core, hôte Tauri, `api.ts` côté TS) — c'est voulu (contract-first), mais chaque ajout doit être justifié par un besoin réel d'affichage, pas par spéculation.
2. **Ce que la maquette montre et qu'aucun adaptateur ne déclare reste invisible** dans le shell réel (ex. presets, canaux alpha/beta/stable affichés dans la maquette sans source Core). Ce n'est pas une régression : c'est le rappel qu'il manque encore l'API Core correspondante.
3. Deux vocabulaires à ne pas laisser diverger : `GameSection` côté Rust et `GameSection` côté TS sont maintenus à la main. La génération de types depuis Rust est la sortie de secours documentée si la dérive devient réelle.
