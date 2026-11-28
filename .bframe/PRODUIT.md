---
titre: Produit
description: Ce que fait ModLab aujourd'hui (concepts, écrans, actions possibles, essais), décrit d'après le code ; on le met à jour quand une fonction apparaît, change ou disparaît.
maj: 2026-11-28
---

# Produit

Ce fichier décrit ce que le code fait aujourd'hui. Le cap est dans `PROJET.md`, la vision dans `VISION.md`, l'avancement dans `ETAT.md`, les choix dans `DECISIONS.md`.

## À quoi sert ModLab

Une application Windows (Tauri + React) qui range les mods d'un jeu dans des instances, laisse les essayer, les annuler et lancer le jeu avec. Deux jeux sont branchés : Skyrim Special Edition et Minecraft. Les données restent sur la machine.

## Concepts

- **Jeu** : une installation détectée sur la machine (Skyrim SE, Minecraft). Il décide des onglets et des outils disponibles.
- **Instance** : un environnement de jeu à soi, avec son nom et son dossier. Elle contient une ou plusieurs branches.
- **Branche** : une liste ordonnée de mods, propre à une instance. Une instance neuve a une branche `principale`.
- **Canal** : l'étiquette de maturité d'une branche, Stable, Beta ou Alpha. Il donne la lettre des révisions (`r.12`, `b.3`, `a.1`).
- **Révision** : un instantané enregistré de la branche. On peut enregistrer les changements en cours ou les annuler.
- **Mod** : une ligne d'une branche. Il est téléchargé, archivé ou installé, actif ou non, et a une priorité.
- **Archive** : le fichier téléchargé (zip ou 7z ; RAR n'est pas lu). Un mod « archivé » n'est pas encore installé. À ne pas confondre avec l'onglet Archives de Skyrim (fichiers `.bsa` / `.ba2`).
- **Téléchargement** : un fichier pris par le navigateur intégré quand l'humain clique. ModLab ne télécharge jamais seul.
- **Source** : d'où vient un mod (Nexus, Modrinth, CurseForge… ou « Manuel » pour un ajout depuis le disque).
- **Conflit** : deux mods qui fournissent le même fichier. Le dernier de la séquence gagne (voir `DECISIONS.md`).
- **Corbeille** : ModLab ne supprime rien d'un coup. Les fichiers vont dans le dossier `trash` de la racine des données, les instances supprimées restent restaurables.
- **Journal** : la liste des transactions (installer, désinstaller, enregistrer, annuler) gardées par le moteur.
- **Racine des données** : le dossier où ModLab range base, instances, téléchargements, cache et corbeille (`F:\ModLab` en usage réel ; comment elle est choisie : `ARCHITECTURE.md` §5.2).

## Cadre de l'application

- **Barre de titre** : les onglets du jeu actif (donnés par le jeu, voir plus bas ; « Mon jeu » est à part), puis les boutons Web, Chat, Console, recherche (palette Ctrl+K) et les commandes de fenêtre.
- **Barre de gauche** : Explorateur, Recherche, Branches, Outils, et la roue des réglages.
  - Explorateur : favoris en tête, puis les jeux et leurs instances. Clic droit sur une instance : canal, favori, renommer, supprimer. Créer une instance depuis un jeu.
  - Carte de l'instance active : nom modifiable, canal, bouton Jouer / Arrêter.
  - Recherche : filtre la liste des mods.
  - Branches : les branches de l'instance, ses révisions récentes, la corbeille des instances.
  - Outils : vide, pas de registre d'outils.
- **Colonne de droite** : Info (détail du mod choisi), Chat, Console, pages Web. Une seule barre d'onglets. La Console a trois modes : Journal (transactions), Jeu (sortie du jeu lancé), PowerShell.
- **Panneau du bas** (Ctrl+J) : Transactions et Téléchargements.
- **Palette** (Ctrl+K) : nouvelle instance, importer, lancer, enregistrer, ouvrir un onglet ou une instance, trouver un mod. Ctrl+S enregistre.

## Mods (les deux jeux)

L'onglet Mods montre l'en-tête de l'instance, la barre de branche puis le tableau.

- **Barre de branche** : changer de branche ou de révision, créer une branche, ouvrir le dossier de l'instance, voir le nombre de mods et de mods actifs, Enregistrer / Annuler quand il y a des changements, Ajouter un mod.
- **Ajouter un mod** : ouvre une liste de sites (favoris par jeu) dans le navigateur intégré, ou « Importer… » depuis le disque.
- **Tableau** : activer / désactiver, glisser pour changer la priorité, trier par colonne (Mod, Cat., Taille, Date), choisir une catégorie, voir version, source, conflits et dépendances.
- **Actions sur un mod** : renommer, voir le dossier, voir l'archive, demander au Chat, déplacer vers une autre instance, installer depuis l'archive, désinstaller (retour à l'archive), supprimer.
- **Installation** : depuis l'archive, avec l'assistant FOMOD quand l'archive en a un. Les fichiers vont en corbeille, jamais effacés.
- **Info** (colonne de droite) : chemin, priorité, source, version, taille, dates, versions de jeu, dépendances, conflits, fichiers de config du mod en lecture seule.
- **Métadonnées** : remplies par couches (saisie, archive, page ouverte, nom de fichier) ; la saisie de l'humain prime.

## Mon jeu

- Version du jeu, nombre d'instances, mode de déploiement, dossier d'installation.
- Chemins du jeu (jeu, mods, téléchargements, configs) avec une valeur détectée et une valeur forcée au choix.
- Contrôle vanilla : résumé de ce qui est hors vanilla dans `Data`. Le bouton Nettoyer ouvre seulement un plan, il ne supprime rien.

## Skyrim Special Edition

Onglets : Mon jeu, Mods, Plugins, Archives, Répertoire, Sauvegardes, Santé.

- **Jouer** : lance Skyrim par SKSE, avec les mods de la branche vus par le jeu sans toucher au dossier `Data` (USVFS, chargé depuis l'installation de MO2). ModLab écrit `plugins.txt`, vérifie SKSE avant le départ et lit ses journaux après. ModLab reste utilisable pendant la partie.
- **SKSE** : cellule de l'en-tête. Absent : ouvre la page officielle. Présent : installer ou activer, ou choisir entre plusieurs versions. SKSE est un mod comme un autre.
- **Plugins** : ordre de chargement, masters, drapeaux Officiel / Mods, ESM / ESL. Signale master manquant ou désactivé, cycle, limites de plugins actifs, nom illisible. On peut activer et réordonner.
- **Archives** : les `.bsa` / `.ba2` fournis par les mods actifs, avec le mod qui les fournit. Pas de contenu, pas de vérification.
- **Répertoire** : parcours du dossier d'installation du jeu.
- **Sauvegardes** : liste des `.ess` du dossier de sauvegardes.
- **Importer** : un profil MO2, en lecture seule. Les mods pointent vers les dossiers MO2, rien n'est copié.
- Le Core sait lire et écrire les réglages graphiques de Skyrim (`skyrim_graphics_settings`), mais aucun écran ne les montre.

## Minecraft

Onglets : Mon jeu, Mods, Packs & shaders, Mondes, Serveurs, Santé.

- **Instance** : chaque instance a son dossier de jeu sous `instances/minecraft/.<slug>` (mods, config, resourcepacks, shaderpacks, saves). Le `.minecraft` du launcher n'est pas touché.
- **Création** : nom, version de Minecraft, loader (Vanilla, Fabric, Forge, NeoForge, Quilt), version du loader. Loader et versions se règlent ensuite par branche, dans l'en-tête.
- **Loader** : installé en un clic depuis les sources officielles. Tant qu'il manque, un bouton « Installer … » prend la place du sélecteur de branche.
- **Jouer** : lance Java directement, avec le Java trouvé ou téléchargé, et affiche la sortie du jeu dans Console > Jeu. Hors ligne par défaut (pseudo dans Réglages > Compte) ; avec un compte Microsoft connecté, en ligne. Un bouton Arrêter coupe la partie.
- **Tester le lancement** : lance le jeu en fenêtre minimale (1×1) et dit s'il atteint le menu (Minecraft seulement).
- **Activer / désactiver** un mod : renomme `foo.jar` en `foo.jar.disabled`.
- **Importer** : l'instance Minecraft détectée, ou un jar posé à la main dans `mods/` (synchroniser).
- **Packs & shaders**, **Mondes** : listes des dossiers de l'instance. Rien n'est lu dans les mondes (graine, mode, dernière partie).
- **Serveurs** : dit seulement si `servers.dat` existe.

## Réglages

Préférences : Général, Compte (connexion Microsoft pour Minecraft), Apparence (densité Compact / Confortable), Navigateur (page d'accueil et moteur de recherche), Notifications, Design system. Plateforme : Jeux (chemins), Catégorie (liste par jeu), Modèles, MCP & Outils, Confidentialité.

## Chat et IA locale

Le Chat répond sur la branche active. Un moteur de règles diagnostique d'après les faits du moteur (conflits, dépendances, plugins). Un modèle local `.gguf` peut reformuler la réponse s'il est déposé dans `<racine>/models` (ou `MODLAB_LLM_MODEL`). Sans modèle, les règles répondent seules. Pendant une partie, l'IA est suspendue.

## Essais et limites connues

Écrans sans moteur (visuels seulement) :
- **Profil**, **Boutique**, **Atelier de patchs** : essais, sans moteur derrière. Ils n'apparaissent que si « Mises à jour communautaires » est activé dans Réglages > Général. Leurs boutons annoncent « pas encore branché ».
- **Santé** (les deux jeux) : « Santé indisponible », aucun diagnostic d'instance.
- **Outils** de la barre de gauche, onglet **Recherche** de la colonne de droite : vides ou annonce « s'ouvrira ici ».
- **Modèles** : le choix du modèle n'est qu'une préférence d'écran ; le moteur prend le premier `.gguf` trouvé. Les modes Plan / Debug / Multi-tâches du Chat sont affichés mais n'ont aucun effet sur la réponse.
- **MCP & Outils** : texte « Phase 3 », aucun serveur MCP n'existe.

Boutons et menus sans effet (ils affichent « pas encore branché au Core ») :
- **Partager** (carte de l'instance).
- Menu hamburger : tout sauf Nouveau, Nouveau loadout et Importer (Ouvrir, Enregistrer, Exporter, Dupliquer, Renommer, Trier avec LOOT, Ouvrir xEdit, Terminal, Diagnostiquer, Affichage…). « Supprimer » y est grisé.
- Réglages > Apparence > Thème Clair / Sombre / Auto, et Réglages > Jeux > Ajouter un jeu.
- Réglages > Général > Mutations réversibles (interrupteur figé).
- Importer sur un jeu sans source d'import connue.

Limites :
- Pas de fournisseur ni d'API Nexus, Modrinth, CurseForge : tout passe par un clic humain dans le navigateur intégré.
- LOOT et xEdit sont nommés mais pas lancés.
- Pas de lecture des mondes ni de la liste des serveurs Minecraft (NBT binaire).
- Le lancement de Skyrim n'existe que sous Windows.
- La CLI (`modlab-cli`) existe mais n'est pas un produit suivi ici.
