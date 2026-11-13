# Design system

> Extrait de `00-cadrage-technique.md` (§6 entière) le 2026-11-13. Texte recopié mot pour mot.

## 6. Design system — proposition v1.1 « cadre, feuille, flottants »

> Direction (tes mots) : se rapprocher de **GitHub, Claude Code, Cursor** — thème sombre avec de petits ombrages, des panneaux qui s'ouvrent « avec une couleur en plus », et les mises en page des IDE. Bonne intuition : la plupart des moddeurs ont déjà ouvert VS Code, GitHub ou xEdit ; ils reconnaîtront immédiatement l'explorateur, les onglets, le panneau « Problèmes » et la barre d'état. On garde de la v0 ce qui n'avait rien à voir avec Steam : la densité MO2, le « pourquoi » partout, le langage visuel de l'état. **v1.1** (14 sept.) : d'après ton retour, on passe au modèle exact de l'app Claude Code — un **cadre** sombre d'un seul tenant (titre, zone gauche, barre du bas), une **feuille** principale arrondie d'une seule couleur, et des **panneaux flottants** arrondis par-dessus (panneau droit, panneau du bas). Trois fonds, presque aucune bordure.

### 6.0 Analyse de référence : pourquoi Claude Code, Cursor et GitHub « marchent »

Ce que ces trois interfaces ont en commun, et qu'on reprend tel quel :

| Principe | Ce que ça donne chez eux | Ce qu'on en fait dans ModLab |
|---|---|---|
| **Surfaces en couches, pas de lignes lourdes** | 4–6 niveaux de gris très proches ; un panneau qui s'ouvre est *un cran plus clair* que son parent, avec une bordure de 1 px et une ombre douce. La profondeur se lit sans qu'on la remarque. | **Trois fonds** (§6.3) : `c1` cadre, `c2` feuille principale, `c3` panneaux flottants et cartes ; `c4`/`c5` survol/pressé. Pas de bordures entre zones : la différence de fond suffit. Ombre uniquement sur ce qui *flotte* (panneau droit, panneau du bas, palette, menus). |
| **Une seule couleur d'accent, avec parcimonie** | Le bleu GitHub, l'ambre Claude : sur les liens, la sélection, le focus — et presque rien d'autre. Tout le reste est gris. | `accent` = sélection/focus/liens. Les couleurs sémantiques (ok/warn/danger/conflits/généré/Claude) ne comptent pas comme accent et sont réservées à l'état. |
| **Petit texte, grand confort** | 12–13 px, contraste modéré (jamais de blanc pur), interlignage généreux, chiffres tabulaires. Police système : l'app a l'air *native*. | 13 px UI / 12 px listes, police système (Segoe UI Variable) + mono (Cascadia / JetBrains Mono) pour chemins, hashes, plugins. |
| **Tout est un panneau repliable** | Sidebar, panneau du bas, panneau latéral droit, chacun avec un raccourci. L'utilisateur compose son espace de travail. | `Ctrl+B` explorateur, `Ctrl+J` panneau, bouton pour le panneau droit. Les préférences de layout sont mémorisées par jeu. |
| **Vocabulaire connu** | Explorateur, onglets, palette de commandes, « Problèmes », barre d'état, contrôle de source. On sait où regarder. | Explorateur = jeux/loadouts/catégories/outils · Onglets = ce qu'on ouvre (un mod, un plugin, un diff, la santé) · **Problèmes = diagnostics** · **Contrôle de source = révisions** (r.142, branches = loadouts dérivés) · Extensions = outils · Terminal = console/jobs · Run = Jouer · État « debug » = **jeu en cours** (barre d'état colorée). |
| **Le clavier est un citoyen de première classe** | Raccourcis affichés partout (petits `kbd`), palette `Ctrl+K` / `Ctrl+Shift+P`. | Palette Ctrl+K avec commandes, mods, plugins **et actions Claude** (préfixe `›`). |
| **L'état s'exprime par de petites formes** | Points, pastilles, compteurs dans les onglets, une bordure colorée — jamais de grandes zones colorées. | Compteurs dans les onglets (Santé **1**), pastilles de santé dans l'explorateur, badges de conflit, barre d'état. |
| **Mouvement fonctionnel** | Panneaux qui glissent en 150–200 ms, rien de décoratif. | Idem. Une seule exception assumée : le passage en mode « jeu en cours ». |
| **Accessibilité de base sans y penser** | Focus visible, cibles ≥ 24 px, contrastes vérifiés, respect de `prefers-reduced-motion`. | Mêmes règles, vérifiées dans Storybook. |

Ce qui est spécifique à **Claude Code** et qu'on reprend : les panneaux de droite (diff, terminal, navigateur) qui s'ouvrent comme des surfaces distinctes avec leurs propres onglets → chez nous, le **panneau droit = Inspecteur / Claude**, exactement comme le chat de Cursor. Et la présence de Claude est *toujours* identifiable par une couleur réservée (`claude`, ambre), jamais confondue avec l'accent.

Ce qui est spécifique à **GitHub** : les « boxes » (en-tête un cran plus foncé + lignes), les badges arrondis à bordure translucide, le bouton primaire vert. → Pages de mod, cartes de diagnostic, bouton **Jouer**.

Ce qui est spécifique à **Cursor / VS Code** : barre d'activité à icônes, explorateur en arbre avec guides d'indentation, onglets fermables avec compteur, panneau du bas à onglets en capitales, barre d'état de 22 px qui change de couleur en debug. → Le squelette entier du shell.

### 6.1 Principes (mis à jour)

1. **Dense mais lisible** : lignes de 26 px (compact) / 34 px (confortable) ; deux niveaux de gris de texte par ligne maximum.
2. **Cadre, feuille, flottants** : le cadre (c1) est d'un seul tenant et sans bordure ; la feuille principale (c2) est arrondie (10 px) ; tout ce qui s'ouvre par-dessus (c3) est arrondi, ombré, et *flotte*. Trois fonds, c'est tout.
3. **L'état est un langage visuel réservé** : conflits (cyan/rose), généré (violet), Claude (ambre), santé (vert/jaune/rouge). Jamais réutilisés pour autre chose.
4. **Tout explique « pourquoi »** : inspecteur, tooltips, panneau Problèmes — chaque statut a sa phrase et sa preuve.
5. **Clavier d'abord** : palette Ctrl+K, navigation liste au clavier, raccourcis visibles.
6. **Composable et économe en fenêtré** : zone gauche, panneau droit et panneau du bas se replient (`Ctrl+B` / `Ctrl+L` / `Ctrl+J`) ; en fenêtre étroite le panneau droit passe *par-dessus* la feuille au lieu de la rétrécir, et la zone gauche se masque seule sous 980 px. Les onglets s'ouvrent et se ferment comme des fichiers.
7. **Sombre d'abord, natif d'aspect** : police système, thème clair par jeu de tokens (style GitHub light).

### 6.2 Le shell (v1.1 : cadre + feuille + flottants)

```
┌ cadre (c1, un seul tenant, aucune bordure) ────────────────────────────────────────────────┐
│ ◆ ModLab  Fichier Loadout Outils Affichage   [ 🔍 Skyrim SE — Main 2026  Ctrl K ]  ▣ ▤ ▥  ▶ Jouer │ 34 px
│ ▣ 🔍 ⑂ 🔧        ⊕ ⚙   ╭ feuille (c2, arrondie) ───────────────────────╮ ╭ panneau droit (c3, flotte) ─╮ │
│ LOADOUT ACTIF          │ Mods ✕ │ Plugins │ Santé ●1 │ SkyUI ✕         │ │ Inspecteur │ Claude 1     ✕ │ │
│ Skyrim SE · Main 2026  │ Skyrim SE › Main 2026 › Mods   [Conflits][MAJ]│ │ SkyUI                        │ │
│ 212 mods · 186 plugins │ FONDATIONS 5                                  │ │ ▾ APERÇU                     │ │
│ Snapshot Dériver       │ ☑ 001 Unofficial Patch     ⚑12  4.3.3         │ │ ▾ POURQUOI CE STATUT         │ │
│ JEUX                   │ ☑ 004 SkyUI                ⚡3/1 5.2SE ↑       │ │ ▾ CONFLITS 4                 │ │
│  ▾ Skyrim SE          ●│ ☐ 010 Lux – JK patch  ⚠ master manquant       │ │ ▾ HISTORIQUE                 │ │
│     Main 2026    actif │                                               │ │                              │ │
│     Labo météo         │                          ╭ panneau du bas (c3, flotte, à la demande) ─────────╮ │
│  ▸ Fallout 4          ●│                          │ Transactions 1 │ Problèmes 3 │ Jobs 2 │ Console 📌 ✕ │ │
│ CATÉGORIES             │                          │ ▣ Installer JK's Skyrim 1.7  plan #127  [Appliquer] │ │
│ OUTILS                 ╰──────────────────────────╰────────────────────────────────────────────────────╯ │
│ ▣ Skyrim SE · Main 2026 · r.142  USVFS    ● Installer JK's 1.7 · 3 conflits [Voir][Appliquer]    ! 1 ⚠ 2  ⬇ 1·77 %  ⌘  ✦ 1  🌐  184 Go │ 30 px
└────────────────────────────────────────────────────────────────────────────────────────────┘
   268 px                                  fluide                                 340 px
```

- **Cadre** (c1) : barre de titre, zone gauche et barre du bas partagent le même fond, sans aucune bordure. Comme dans l'app Claude Code.
- **Zone gauche** (268 px, `Ctrl+B`) : une rangée d'outils en haut (Explorateur · Recherche · Historique · Outils ; Nexus et Réglages à droite), puis l'espace « fichiers » : *Loadout actif* (titre + stats + Snapshot / Dériver / Comparer, sans carte), *Jeux* (arbre jeu → loadouts, pastille santé), *Catégories*, *Outils*. Sections repliables séparées par de l'espace, pas par des lignes.
- **Feuille** (c2, rayon 10 px, ombre 2 px) : onglets (Mods, Plugins, Santé, Accueil, + tout mod ouvert — fermables) sur une ligne, puis **une seule barre** qui combine fil d'Ariane à gauche et filtres/actions à droite (34 px, au lieu de deux lignes), puis le contenu. Un mod s'ouvre dans un onglet comme un fichier (double-clic / Entrée).
- **Panneau droit** (340 px, c3, rayon 10 px, ombre 32 px, `Ctrl+L`) : **flotte au-dessus de la feuille**. Au-delà de 1 280 px de large, la feuille lui laisse la place ; en dessous, il la recouvre (fenêtré). Onglets **Inspecteur / Claude** en pilules, sections repliables.
- **Barre du bas** (30 px, dans le cadre) — *l'innovation demandée* : une seule ligne qui remplace panneau + barre d'état. À gauche le contexte (jeu · loadout · révision · backend). **Au centre, l'élément « vivant »** : le plan en attente (ou le job en cours, ou le jeu en cours) avec ses deux actions inline (*Voir* / *Appliquer*) et une barre de progression de 2 px. À droite, des compteurs cliquables : Problèmes (`! 1 ⚠ 2`), Jobs (`⬇ 1 · 77 %`), Console, **Claude** (`✦ 1`), Nexus, store. Quand le jeu tourne, toute la barre passe en `run` avec le chrono, les FPS et *Arrêter*.
- **Panneau du bas** (c3, flottant, `Ctrl+J` ou clic sur un compteur) : s'ouvre **par-dessus la feuille, en bas à droite**, avec les onglets Transactions · Problèmes · Jobs · Console. Un bouton 📌 l'**ancre** sous la feuille pour ceux qui veulent un vrai panneau permanent ; `Esc` le ferme. Résultat : 0 px consommé au repos, tout reste à un clic, et en fenêtré on garde la place pour la liste de mods.
- **Palette de commandes** (`Ctrl+K`) : commandes, mods, plugins, réglages, actions Claude (`›`).
- **Accueil** (onglet fermable) : les jeux en cartes — ce qui reste de l'idée « bibliothèque ».

### 6.3 Tokens (v1)

**Fonds — thème sombre** (neutre, très légèrement chaud, comme Claude Code)

| Token | Valeur | Usage |
|---|---|---|
| `c1` | `#1b1a19` | **cadre** : barre de titre, zone gauche, barre du bas — d'un seul tenant |
| `c2` | `#232221` | **feuille** principale (arrondie) ; fond des cartes *dans* un panneau flottant |
| `c3` | `#2a2928` | **flottants** : panneau droit, panneau du bas, palette, menus ; cartes et boîtes sur la feuille |
| `c4` / `c5` | `#323130` / `#3c3a38` | survol / pressé, boutons secondaires |
| `line` / `line2` | `rgba(255,255,255,.055)` / `.10` | les seules « bordures » : lignes de tables, liserés de contrôles |
| `sel` | `rgba(90,156,245,.14)` + liseré `.55` | sélection (ligne, item d'arbre) |
| `t1` / `t2` / `t3` | `#e8e6e2` / `#aaa59e` / `#7a766f` | texte principal / secondaire / tertiaire |

**Sémantique**

| Token | Valeur | Usage |
|---|---|---|
| `accent` | `#5a9cf5` | sélection, focus, liens, onglet actif (liseré 1 px) |
| `green` / `green-btn` | `#3fb950` / `#238636` | santé OK · bouton **Jouer** et actions primaires (vert GitHub) |
| `warn` | `#d29922` | avertissements, mises à jour |
| `danger` | `#f85149` | bloquant, destructif |
| `conflict.win` / `conflict.lose` | `#39d0d8` / `#f778ba` | écrase / est écrasé (ni bien ni mal : une information de priorité) |
| `generated` | `#a371f7` | fichiers générés par un outil |
| `claude` | `#d97757` | **tout ce qui vient de Claude** (propositions, messages, révisions) |
| `run` | `#6e3a27` | barre du bas quand le jeu tourne |

**Ombres** — feuille : reflet 1 px en haut + `0 1px 2px rgba(0,0,0,.25)` ; flottants (panneau droit, panneau du bas, palette, menus) : `0 12px 32px rgba(0,0,0,.5)` + liseré translucide 1 px ; listes et tables : aucune ombre, lignes `line` seulement.

**Typographie** — UI : **police système** (`Segoe UI Variable Text`, puis `Segoe UI`, `system-ui`) 13 px, 12 px dans les listes, titres 15–20 px en 600 ; données : `Cascadia Code` → `JetBrains Mono` 12 px. Chiffres tabulaires partout dans les tables. Labels de section en 11 px capitales espacées.

**Rythme** — base 4 px : 4 · 8 · 12 · 16 · 24 ; rayons **10** (feuille, flottants) / 8 (cartes, boîtes) / 6 (boutons, chips) ; marge de 8 px entre le cadre et les surfaces ; lignes 26 / 34 px ; barres 34 (titre) / 36 (onglets) / 34 (fil d'Ariane + outils) / 30 (barre du bas) px ; mouvement 120 ms (micro) / 200 ms (panneaux).

**Thème clair** — même structure, valeurs GitHub-light (cadre `#e9ebee`, feuille `#ffffff`, flottants `#f6f8fa`, accent `#0969da`, vert `#1f883d`). Un jeu de tokens, zéro refonte.

### 6.4 Inventaire de composants (v1)

| Composant | Rôle |
|---|---|
| `Frame` | Le cadre : titre + zone gauche + barre du bas, un seul fond |
| `TitleBar` + `CommandCenter` | Menus, recherche/commande centrale, toggles de layout (gauche / bas / droite), **Jouer** |
| `SideTools` | Rangée d'outils en haut de la zone gauche (Explorateur, Recherche, Historique, Outils · Nexus, Réglages) |
| `SideBar` + `Section` + `TreeRow` | L'espace « fichiers » : sections repliables sans lignes, arbre jeu → loadouts |
| `ActiveLoadout` | Titre + stats + Snapshot / Dériver / Comparer (sans carte) |
| `Sheet` | La feuille principale arrondie qui contient onglets, barre et contenu |
| `TabBar` + `Tab` | Onglets d'édition fermables, compteur, liseré d'accent |
| `SheetBar` (`Breadcrumbs` + filtres/actions) | Une seule ligne : Jeu › Loadout › Vue à gauche, filtres et actions à droite |
| `ModTable` / `ModRow` / `Separator` | Table virtualisée ; badges conflit / plugins / MAJ / erreur ; séparateurs de catégories avec compteur |
| `PluginList` / `PluginRow` | Ordre de chargement, index hex, ESM/ESP/ESL, verrou, masters |
| `ModPage` + `Box` | Un mod ouvert en onglet : boîtes GitHub-like (Pourquoi, Conflits, Note, Fichiers) |
| `DiagnosticCard` / `ProblemRow` | Santé (cartes) et Problèmes (lignes compactes dans le panneau) |
| `FloatingPanel` → `Inspector` / `ClaudePanel` | Panneau droit flottant, arrondi ; sections repliables ; conversation + `PlanCard` (Approuver / Refuser) |
| `Tray` + `LivePill` + `TrayCounter` | La barre du bas : contexte, élément vivant (plan / job / jeu en cours) avec actions inline, compteurs cliquables |
| `TrayPanel` (flottant ou ancré 📌) → `TransactionCard` / `ProblemRow` / `JobRow` / `Console` | Le panneau du bas à la demande |
| `CommandPalette` | Ctrl+K : commandes, mods, plugins, actions Claude |
| `Badge` / `Chip` / `Kbd` / `Button` (default, primary, ghost, claude) / `Toast` / `EmptyState` | Atomes |

### 6.5 Où vit le design system

✅ **Le code est la source de vérité** : `ui/design/tokens.json` → CSS variables (+ thème clair par surcharge) → Storybook comme catalogue vivant. Figma/Pencil servent à **explorer** (MCP connectés).

**Maquette interactive du shell** (données d'exemple) : `docs/design/shell-maquette.html` — publiée en artifact : https://claude.ai/code/artifact/102c90d8-0fb5-438f-a8fd-d08b4e16a928 (version 3 = cadre + feuille + flottants). À essayer : `Ctrl+K` (palette), `Ctrl+B` / `Ctrl+L` / `Ctrl+J` (zone gauche / panneau droit / panneau du bas), les compteurs de la barre du bas (ouvrent le panneau flottant ; 📌 pour l'ancrer), double-clic sur un mod (onglet), onglet **Claude** du panneau droit (plan #128), bouton **Jouer** (barre du bas en mode jeu), icône « palette » (tokens), thème clair, densité. Réduis la fenêtre pour voir le mode fenêtré.

💬 → **Q14–Q18**.

### 6.6 État d'implémentation (18 septembre 2026)

Où en est ce design system dans le **shell réel** (`ui/`, React + Tauri) — la maquette reste la référence visuelle, le shell suit :

| Élément du §6 | État dans le shell React |
|---|---|
| **Tokens** | Miroir des valeurs de cette section dans `ui/src/shell.css` (variables CSS `--c1…--c5`, `--t1…--t3`, sémantique identique). `ui/design/tokens.json` n'est **pas encore extrait** : en attendant, `shell.css` est la source de vérité pratique, la maquette reste la référence visuelle. |
| **Structure « cadre, feuille, flottants »** | ✅ v2.0 : deux colonnes (panneau gauche `c1` + feuille centrale `c2`), flottants `c3` (panneau droit, menus, toasts). |
| **Atomes** | ✅ `Button`, `IconButton`, `FavoriteButton` (`ui/src/components/ui/`), `Ico`, toast, `EmptyState` — + `MenuPicker` (popover multi-variantes Fichier/Loadout/Outils/Affichage). |
| **Header unifié + onglets** | ✅ `GameDashboardHeader` + `tabsFor()` : les onglets viennent des `GameCapabilities` de l'adaptateur (ADR 0005), jamais codés par jeu. |
| **`ModTable`** | ✅ virtualisée (`@tanstack/react-virtual`), badges de conflits réels (wins/loses du scan Core). |
| **`FloatingPanel` → Inspector / ClaudePanel** | ✅ panneau droit flottant (info mod + onglet IA) ; le chat IA attend encore son branchement Core. |
| **`CommandPalette`** | 🚧 en cours (Phase 2) — le vocabulaire CSS `.cp` de la maquette est déjà reporté dans `shell.css`. |
| **`TrayPanel` (panneau du bas)** | 🚧 en cours (Phase 2) — classes `.tpanel` / `.tx` prêtes dans `shell.css`, branchement Core (transactions) à venir. |
| **Barre du bas `Tray` / compteurs** | 🚧 le shell réel a une barre de contexte simplifiée ; les compteurs cliquables arrivent avec le panneau du bas. |
| **Storybook / thème clair / densité** | ❌ pas commencés — la maquette les garde en démonstration. |

**Convention d'honnêteté** (identique partout) : ce que le Core n'expose pas est un placeholder qui se nomme (« pas encore branché au Core »), jamais une donnée inventée.
