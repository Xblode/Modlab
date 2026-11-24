# Règles d'interface ModLab

Date : 2026-11-24

L'appli React `ui/` est la référence visuelle (décision du 24 nov.). Ce document dit comment la faire évoluer. Il prime sur la maquette `docs/design/shell-maquette.html`, qui est un historique.

## Décisions validées par Benjamin (2026-11-24)

- **Densité** : « Confortable » par défaut (lignes de liste et de table à 32 px, boutons et champs à 36 px). Réglage Compact / Confortable dans les Réglages (Compact : lignes à 28 px, contrôles à 32 px).
- **Texte** : corps à 14 px, petits libellés à 12 px, rien sous 12 px. Graisses 400 et 500, 600 pour les titres.
- **Finitions type shadcn** :
  - arrondis de 8 px pour les contrôles, 12 px pour les cartes, panneaux et dialogues, 4 à 6 px pour les étiquettes ;
  - bordures à 10 % d'opacité (15 % pour les champs) ;
  - moins de traits : les zones sont séparées par 3 à 4 nuances de fond (fond d'app, contenu, surface relevée, surcouche). Plus haut = plus clair en sombre ;
  - espaces : 8 px dans un groupe, 12 à 16 px entre groupes, 16 à 24 px aux bords des panneaux ;
  - grille de 4 px.
- **Hiérarchie** :
  - un seul bouton principal par vue (« Jouer ») ;
  - chaque bouton à icône seule a une infobulle courte avec son raccourci clavier, sans répéter un texte déjà visible ;
  - états vides courts : titre, une phrase, une action ;
  - barre de statut : à gauche ce qui concerne tout le jeu, à droite l'instance en cours. Peu d'éléments, pas de couleur maison.
- **Mise en page inchangée** : barre latérale à gauche, onglets en haut, contenu au centre, panneau d'inspection à droite, barre de statut en bas, palette Ctrl+K.

## Règles déjà validées avant (à garder)

- **Noms longs** : jamais sur deux lignes, jamais « … ». Ils s'estompent (utilitaire `.fade` / variable `--fade`). Les titres des panneaux de détail peuvent passer à la ligne.
- **Cartes d'instance** : lignes basses (environ 58 à 60 px) avec une vignette carrée à gauche (dégradé de la couleur du jeu + initiales, en attendant une image).
- **Favoris** : étoile de favori partout où une instance est listée. Les favoris sont toujours en tête. Pas de zone « Favoris » sur le tableau de bord.
- **En-tête du jeu** : pleine largeur, sans carte, sans bordure, sans arrondi. Lavis en dégradé de la couleur du jeu, filet en bas. Les actions rapides sont dedans, sous les tuiles de stats. L'image du jeu garde son ratio 460:215.
- **Puces moteur** (SKSE, ENB…) : elles ouvrent un vrai panneau de gestion.
- **Sections de barre latérale vides** : l'en-tête reste toujours visible et cliquable.
- **Méthode « unslop »** (github.com/heydamir/unslop, licence CC BY 4.0, Damir Sokolovsky), passée le 24 nov. :
  - pas de dégradés de teinte décoratifs (sauf la couleur du jeu) ;
  - pas de glow, pas d'emoji ;
  - un seul jeu d'icônes (`ui/src/Ico.tsx`) ;
  - pas de majuscules espacées, pas d'italique ;
  - animations limitées à couleur, fond, bordure, opacité et transform, avec une seule courbe.

## Ce que disent les références (consensus)

1. Grille de 4 px, base 8 (Fluent, Atlassian, Primer, shadcn).
2. Texte courant à 14 px, petits libellés à 12 px, rien sous 12 px (Fluent typographie : corps 14/20, légende 12/16 ; shadcn `text-sm`).
3. Arrondis petits sur les contrôles, plus grands sur les surfaces flottantes (Fluent 4/8 px ; Primer 6/12 ; shadcn 8/10).
4. Bordures discrètes en sombre (shadcn : blanc à 10 %, champs à 15 % ; Radix : pas 6 discret, 7 contrôles, 8 fort et focus).
5. Peu de niveaux de fond, plus clair = plus haut (Atlassian 4 niveaux ; Radix ; Geist ; Linear).
6. Contrôles en 3 tailles, la moyenne par défaut (shadcn 32/36/40 ; Primer 28/32/40).
7. L'espace sert à grouper : moins dans un groupe, plus entre groupes (Atlassian ; Fluent 8 px dans un groupe, 12 px entre zones, 16 px aux bords).
8. Un seul bouton principal par vue (Fluent 1 à 2 boutons ; Primer ; VS Code).
9. États vides courts : titre, phrase, action (Primer, VS Code).
10. Barres latérales peu profondes : 2 niveaux max (macOS), 3 à 5 vues (VS Code).
11. Infobulles sobres, obligatoires pour les boutons à icône seule, avec le raccourci (Fluent).
12. Barre de statut : gauche global, droite contexte, peu d'éléments (VS Code).

**Désaccords** :
- taille de contrôle par défaut : shadcn 36, Primer 32 ;
- arrondis : shadcn plus ronds que Fluent ;
- densité : Fluent aéré, Primer 3 densités, Linear teste du condensé au spacieux.

## Ce que disent les études (prudence)

- Aucune étude solide trouvée ne prouve qu'une interface plus aérée rend les experts plus rapides. Les tiers de densité des design systems sont des conventions. D'où le réglage Compact / Confortable.
- **Effet esthétique-utilisabilité** : une interface plus belle est perçue comme plus facile (Kurosu & Kashimura 1995, 252 participants ; Tractinsky et al. 2000), sans être plus efficace. Elle fait pardonner les petits défauts, pas les gros (NN/g).
- Des marges autour des blocs améliorent la vitesse et la compréhension de lecture. L'interligne seul joue peu (Chaparro et al. 2004, texte continu).
- **Zébrage des tableaux** : neutre pour les lectures simples (Enders 2008), utile pour les comparaisons complexes (Ji et al., IEEE VIS 2024).
- **Thème sombre** : la lecture dense y est un peu moins bonne qu'en clair (Piepenbrock et al. 2013). Il faut donc soigner le contraste et viser au moins 4,5:1 (WCAG) pour le texte courant.
- **Premières impressions** (Lindgaard 2006 ; Reinecke 2013) : la complexité visuelle pèse plus que la couleur. Ce sont des études sur des sites web, pas sur des outils.

## Sources (consultées le 2026-11-24)

- shadcn/ui : https://ui.shadcn.com/docs/theming ; code source button.tsx / input.tsx / table.tsx (github.com/shadcn-ui/ui, new-york-v4) ; https://ui.shadcn.com/docs/components/sidebar
- Radix Colors : https://www.radix-ui.com/colors/docs/palette-composition/understanding-the-scale
- VS Code UX Guidelines : https://code.visualstudio.com/api/ux-guidelines/overview (+ status-bar, sidebars, notifications, views)
- Windows / Fluent :
  - https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/typography
  - https://learn.microsoft.com/en-us/windows/apps/design/basics/content-basics
  - https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/geometry
  - https://learn.microsoft.com/en-us/windows/apps/design/controls/buttons
  - https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/tooltips
  - https://fluent2.microsoft.design/layout
- Apple HIG Sidebars : https://developer.apple.com/design/human-interface-guidelines/sidebars
- GitHub Primer : https://primer.style/product/components/data-table/ ; https://primer.style/product/primitives/size/ ; https://primer.style/product/ui-patterns/empty-states/
- Atlassian : https://atlassian.design/foundations/spacing ; https://atlassian.design/foundations/elevation
- Vercel Geist : https://vercel.com/geist/colors
- Linear : https://linear.app/now/how-we-redesigned-the-linear-ui
- NN/g : https://www.nngroup.com/articles/aesthetic-usability-effect/
- Tractinsky 2000 : https://academic.oup.com/iwc/article-abstract/13/2/127/898608
- Chaparro 2004 : https://portfolio.erau.edu/en/publications/reading-online-text-a-comparison-of-four-white-space-layouts/
- Enders 2008 : https://alistapart.com/article/zebrastripingdoesithelp/
- Ji et al. 2024 : https://www.ieeevis.org/year/2024/program/paper_v-full-1288.html
- Piepenbrock 2013 : https://informahealthcare.com/doi/abs/10.1080/00140139.2013.790485
- Reinecke 2013 : https://kgajos.seas.harvard.edu/papers/reinecke13aesthetics.pdf
- Contraste : https://web.dev/articles/color-and-contrast-accessibility

*Note : plusieurs études n'ont été lues qu'en résumé.*
