# Adaptateur Minecraft

## À quoi sert ce dossier

Ce dossier porte tout ce qui est propre à Minecraft : loaders, jars et dossier `mods/` d'une instance. Il branche ce jeu sur le contrat `GameAdapter` du Core.

## Fonctionnalités

- **Détection :** cherche une installation dans les emplacements habituels (launcher officiel, Prism Launcher, Modrinth, CurseForge).
- **Lecture des mods :** liste les jars actifs et désactivés, et lit leur manifeste (Fabric, Quilt, Forge, NeoForge). Un jar illisible reste listé par son nom.
- **Import d'une instance :** crée un loadout sans rien modifier dans le dossier du jeu.
- **Installation d'un jar :** copie le fichier sans jamais écraser un jar existant, et annule la copie si l'enregistrement en base échoue.

Pas encore présents : CurseForge et Modrinth, le lancement du jeu et la désinstallation d'un jar. Les sections Packs, Mondes et Serveurs sont déclarées, mais ce dossier ne gère que les mods.

## Où aller ensuite

- Contrat d'adaptateur : `crates/modlab-core`.
- Autre adaptateur de jeu : `crates/modlab-game-skyrim`.
- Données de test : `fixtures/fake-minecraft`.
- Décisions : `docs/01-architecture-decidee.md` §3.1 et ADR 0005 dans `docs/adr/`.

## Ce qu'il ne faut pas y faire

- Déplacer une notion de loader, de jar ou de launcher vers le Core.
- Remplacer un jar existant lors d'une installation.
- Deviner le loader d'un jar : s'il ne le dit pas, il reste « inconnu ».
- Tester contre une vraie installation : les tests passent par `fixtures/`.
