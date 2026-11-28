---
titre: Vision
description: Le rêve à long terme de ModLab et ses grandes directions. S'écrit quand le cap lointain change ; le cap concret est dans PROJET.md.
maj: 2026-11-28
---

# Vision

## En une phrase
ModLab est **un gestionnaire complet d'environnements de jeu : une plateforme centralisée de modding**, avec une interface épurée et moderne.

## La promesse
« Je peux laisser ModLab automatiser le travail, mais je peux comprendre et contrôler absolument tout quand j'en ai besoin. »

| Outil | Sa promesse |
| --- | --- |
| MO2 | Je contrôle précisément mon installation. |
| Vortex | Le gestionnaire automatise mon installation. |
| CurseForge | J'installe et je maintiens facilement un environnement moddé. |
| **ModLab** | J'automatise, et je garde le contrôle de tout. |

## Ce qui guide tout
- Personnel d'abord : l'outil de Benjamin avant un produit grand public.
- MO2 comme référence de maîtrise.
- Rien n'est caché : chaque action dit quoi, pourquoi, quels fichiers, et comment revenir en arrière.
- Tout est réversible.
- La vérité vient des faits (fichiers, journaux), pas des suppositions.
- Le créatif garde le volant.

## Les grandes directions
Ordre et conditions de départ : `ROADMAP.md`.

1. **Plusieurs jeux par construction.** Skyrim SE puis Minecraft aujourd'hui ; d'autres jeux ensuite, chacun branché par son adaptateur.
2. **Une IA qui comprend vraiment le jeu moddé**, et pas seulement qui en discute à côté : elle explique un conflit, installe un mod, diagnostique un plantage. Principe : les règles trouvent la cause, le modèle ne fait que rédiger. Elle ne passe jamais par-dessus les permissions de ModLab.
3. **Les superpouvoirs** qu'un outil personnel peut se permettre :
   - recherche automatique du mod fautif (désactiver la moitié, relancer, recommencer) ;
   - expériences par branches, avec différences lisibles et historique (« loadout as code » : chaque branche en fichiers texte versionnés) ;
   - patchs générés à la demande (« double les prix des marchands » → un patch Mutagen/Synthesis créé, exécuté, rangé comme un mod réversible) ;
   - copies instantanées (CoW, Dev Drive) pour les outils qui refusent le système de fichiers virtuel ;
   - analyse native des journaux de plantage, croisée avec les derniers changements ;
   - enregistrement des sessions de jeu (durée, FPS, plantage) et comparaison entre révisions ;
   - analyse des sauvegardes (plugins manquants, compatibilité avec une branche) ;
   - différences de plugins lisibles par un humain ;
   - recettes d'actions enchaînées (après une mise à jour : instantané, outils, tri, contrôle, retour arrière si régression) ;
   - comparateur visuel avant/après ;
   - mode « je sais ce que je fais » : écriture directe, mais toujours tracée et réversible.
4. **La communauté** : profils, modpacks, patches, place de marché, économie cosmétique. Cœur gratuit, local d'abord, données partagées seulement avec accord. Un serveur de données communautaire possible : un manifeste par jeu et des remontées volontaires, désactivé par défaut, fonction secondaire. Direction validée mais pas cadrée. Les écrans Profil, Boutique et Atelier de patchs sont des essais. Étude complète : archive (`deep-research-report.md`).
