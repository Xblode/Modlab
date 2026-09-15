# Plan — Shell v2 (maquette)

> Créé le 14 septembre 2026 à partir de ta liste de demandes. Chaque lot est indépendant et validable dans la maquette avant d'attaquer le suivant. Les lots sont ordonnés par dépendance : le **Lot 1 (squelette)** change la structure, tout le reste se pose dessus.
> État (14 sept., v2.0) : **Lots 0 → 7 et 10 livrés dans la maquette** · reste le Lot 11 (doc §6) et les notes 8–9 sont dans `docs/idees.md`.
> Décision prise : le groupe `[r.142 ▾] [Enregistrer] / [▶ Jouer]` vit dans le bloc branche du panneau gauche (Jouer pleine largeur en dessous). « Commit » est devenu **Enregistrer** ; l'état non enregistré se voit sur le sélecteur de révision (`+2 −1` ambre) et par un point sur Jouer.
> Maquette : https://claude.ai/code/artifact/102c90d8-0fb5-438f-a8fd-d08b4e16a928 · source `docs/design/shell-maquette.html`

## Lot 0 — Livré (v1.4)

- **Vue divisée** : `Maj+clic` sur un onglet ouvre l'onglet courant et l'onglet cliqué côte à côte (chaque moitié a son en-tête et son ✕ ; clic simple sur un onglet = retour à une seule vue ; fermer un onglet retire sa moitié).
- **Flottants** : ombre encore réduite (`0 4px 14px`, 22 %) et **bordure 1 px** sur panneau droit, panneau du bas, composeur, palette, menu ☰, tokens, toast.
- **Panneau droit** : commence sous la barre d'onglets (`top: 36px`).

## Lot 1 — Nouveau squelette : deux colonnes, plus de bandeau haut ni bas

**Pourquoi** : le shell ne doit plus être coupé en quatre. Tout ce qui était dans le bandeau haut et le pied de page va dans le panneau gauche ou dans la barre d'onglets.

- Grille : `268px | 1fr`, une seule rangée. Suppression de `.title` et `.tray`.
- **Panneau gauche** (pleine hauteur, fond c1, bordure droite 1 px) :
  - *Barre d'outils en haut* : `[☰] [Explorateur] [Recherche] [Historique] [Outils] … [Nexus] [Réglages]`. Le ☰ regroupe Fichier · Loadout · Outils · Affichage · Aide, plus (maquette) Densité · Thème · Tokens.
  - *Contenu* : Loadout (Lot 2) → Jeux → Outils → **Catégories** (déplacée sous Outils).
  - *Pied* : ce qui était à gauche du pied de page — `Skyrim SE · Main 2026 · r.142 · USVFS`. Se colore en mode « jeu en cours » (chrono, FPS, Arrêter).
- **Zone centrale** : barre d'onglets `[Mods] [Plugins] [Sauvegardes] [Santé] [SkyUI ✕]` et, **justifié à droite**, ce qui était à droite du pied de page : `! 1 ⚠ 2 · ⬇ 1 · 77 % · ⌘ · ✦ 1 · Nexus · 184 Go`.
- Les icônes *panneau gauche / bas / droit* **disparaissent** (Lot 10). Raccourcis conservés mais invisibles.
- Ce qui était **au centre** du pied de page (plan en attente « Installer JK's Skyrim 1.7 · Voir · Appliquer ») migre au-dessus du composeur (Lot 4).

## Lot 2 — Bloc Loadout sans accordéon, versions, canal, Commit, Jouer

- Plus d'accordéon ni de libellé « Loadout actif » : juste le contenu.
- Ligne 1 : **nom de la branche** + badge de canal **Alpha / Beta / Stable** (select compact, modifiable).
- Ligne 2 : `[r.142 ▾]` **select de version** (liste des révisions : message, date, auteur) → `[Commit]` → `[▶ Jouer]`. Le select est à gauche de Jouer, Commit entre les deux.
- **Commit** = enregistrer l'état actif/inactif des mods (et l'ordre) comme nouvelle révision locale de la branche, avec un message. Un point orange sur le bouton quand il y a des changements non commités ; Jouer lance l'état courant (avertissement discret si non commité).
- Stats sous la ligne (mods · plugins · Go · santé) conservées, en petit. Boutons Snapshot / Dériver / Comparer → dans le ☰ *Loadout* et dans la palette (pour alléger).
- Si les trois boutons ne tiennent pas dans 268 px : repli = `[r.142 ▾][Commit]` sur la ligne 2 et `[▶ Jouer]` pleine largeur en ligne 3.

## Lot 3 — Onglet « Sauvegardes »

- À droite de Plugins. Liste des `.ess` : nom, date, niveau, lieu, **révision qui l'a créée**, compatibilité avec la branche courante (plugins manquants, scripts orphelins), actions : *Charger avec sa révision*, *Analyser* (ReSaver-like), *Marquer*.
- Compteur dans l'onglet quand une sauvegarde est incompatible.

## Lot 4 — Composeur interactif et palette ancrée

- Le bouton **« Panneau »** entre **dans** la barre, à droite de la loupe (icône ⚙ compacte) ; clic → ouvre le panneau Transactions / Problèmes / Jobs / Console.
- **Palette ancrée** : cliquer la barre ouvre les recommandations / commandes / automatisations **au-dessus** de la barre, même largeur, même alignement (popover ancré, plus de modal en haut d'écran). `Ctrl+K` idem. `Échap` ferme.
- **Bandeau d'activité** au-dessus de la barre (chips, même largeur) : plan en attente `● Installer JK's Skyrim 1.7 · 3 conflits [Voir] [Appliquer]`, job en cours `⬇ 77 %`, propositions Claude `✦ 1`, jeu en cours `● 00:12 · 71 fps`. C'est la nouvelle place de « ce qui était au centre du pied de page ».
- **Sous la barre** : suggestions contextuelles discrètes (recettes : « Après MAJ graphique », « Bench A/B »), 1 ligne, masquées avec la barre au scroll.

## Lot 5 — Table des mods : icônes au survol, colonne Mod réduite, réglages de mod

- Au survol d'une ligne, des **icônes-outils** apparaissent **dans la cellule Mod, juste à droite du nom** : `⚙ Réglages` (si le mod a des fichiers de config) · `📁 Dossier` · `🌐 Nexus` · `✦ Claude` · `⋯`.
- Colonne Mod : `minmax(200px, 36%)`, dernière colonne flexible.
- **Réglages d'un mod** : le moteur associe à chaque mod ses fichiers de configuration (`Data/SKSE/Plugins/*.ini`, `*.json`, `*.toml`, MCM `Data/MCM/Settings`, `Data/SkyrimPrefs`-overrides) ; l'icône ⚙ ouvre un onglet « SkyUI — réglages » avec un éditeur clé/valeur (sections, valeurs par défaut, diff, surcharge par branche). Même mécanisme pour les INI du jeu (`Skyrim.ini`, `SkyrimPrefs.ini`, `SkyrimCustom.ini`) et, côté Minecraft, `config/*.toml|json`.

## Lot 6 — Indicateurs Skyrim : SKSE · ENB · ReShade · FG / DLSS / FSR / XeSS

- Pastilles dans la barre d'onglets (droite) ou sous le bloc Loadout : `SKSE 2.2.6 ✓` · `ENB Rudy ✓` · `ReShade ✗` · `DLSS 3 FG (PureDark) ✓` · `Community Shaders ✗`. **Rien n'est affiché si rien n'est détecté.**
- Détails de faisabilité et méthode de détection : voir `docs/idees.md` § « Indicateurs SKSE / ENB / FG ».

## Lot 7 — Contenu réaliste : mods et outils Skyrim SE, Minecraft, presets

- ~40 mods Skyrim SE parmi les plus connus, par catégorie (fondations, interface, visuel, monde, gameplay, animations, corps, PNJ, patchs, sorties générées) avec conflits/plugins plausibles.
- ~14 outils : LOOT, SSEEdit, Nemesis, Pandora, DynDOLOD/TexGen, xLODGen, BodySlide/Outfit Studio, Synthesis, Wrye Bash, BethINI Pie, Cathedral Assets Optimizer, NifSkope, Creation Kit, ReSaver, PACT.
- **Minecraft** (2ᵉ priorité) dans Jeux, avec ses outils (Prism Launcher, Modrinth App, CurseForge, Fabric / NeoForge / Quilt, Java 21) et un preset « Survie Fabric 1.21 · Stable ». Presets Skyrim : « Main 2026 · Stable », « Labo météo · Alpha », « Stable 2025 · Stable », « Bench FG · Beta ».
- Un preset = branche + canal + outils + réglages de lancement (Skyrim : SKSE, ENB ; Minecraft : loader, version, RAM/JVM).

## Lot 8 — LOOT intégré (note de faisabilité)

- Ton install : `F:\_Modding\Skyrim Modding\Loot` contient `LOOT.exe` + `libloot.dll` (Qt 6). Trois niveaux d'intégration possibles, détaillés dans `docs/idees.md`. Recommandation : lancement piloté (`--auto-sort`) en phase 1, `libloot` en process en phase 4.

## Lot 9 — Installation manuelle intelligente (archive → bon endroit)

- Glisser une archive (ex. `skse64_2_02_06.7z`) : ModLab analyse la structure, propose un **plan de placement visuel** (`skse64_loader.exe`, `skse64_*.dll` → racine du jeu ; `Data/Scripts` → Data), l'utilisateur voit exactement ce qui va où, applique, et tout reste désinstallable (fichiers racine tracés par le backend « racine »). Pourquoi ce n'est pas déjà standard : voir `docs/idees.md`.

## Lot 10 — Panneau droit fermé par défaut, plus de toggles de panneaux

- Fermé au démarrage. Clic sur un mod → s'ouvre avec ses informations. ✕ pour fermer. Les icônes panneau gauche / bas / droit sont retirées du shell (raccourcis `Ctrl+B / J / L` conservés, invisibles).

## Lot 11 — Documentation

- Mettre à jour `docs/00-cadrage-technique.md` §6 (shell v2, tokens, composants) et le popover Tokens de la maquette.
- Reporter les idées retenues dans ClickUp (liste « Idées ») quand l'espace sera créé.

## Ordre proposé et estimation (sessions de travail)

| # | Lot | Dépend de | Effort |
|---|---|---|---|
| 1 | Squelette 2 colonnes | — | 1 session |
| 2 | Bloc Loadout (versions, canal, Commit, Jouer) | 1 | ½ |
| 3 | Onglet Sauvegardes | 1 | ½ |
| 4 | Composeur + palette ancrée + bandeau d'activité | 1 | 1 |
| 5 | Table : icônes au survol, colonne, réglages de mod | — | 1 |
| 6 | Indicateurs SKSE/ENB/FG | 2 | ½ |
| 7 | Données : mods, outils, Minecraft, presets | — | 1 |
| 8–9 | Notes LOOT / installation manuelle | — | fait (idees.md) |
| 10 | Panneau droit fermé par défaut, toggles retirés | 1 | ¼ |
| 11 | Doc §6 + tokens | 1–10 | ½ |
