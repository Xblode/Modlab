# Couche IA locale

## À quoi sert ce dossier

Ce dossier produit les réponses de l'assistant de ModLab à partir de faits fournis par l'hôte. Le moteur de règles établit le diagnostic. Le modèle local, optionnel, ne fait que le reformuler.

## Fonctionnalités

- **Diagnostic :** instance vide, mods tous désactivés, mods écrasés et dossiers illisibles, avec trois niveaux de gravité.
- **Réponse sans modèle :** une réponse en français construite à partir des faits. C'est la réponse par défaut.
- **Reformulation par un modèle local :** passe par llama.cpp, avec l'option `local-llm`, désactivée par défaut. Sa compilation demande LLVM et CMake.

## Où aller ensuite

- Appel depuis l'interface : commande `ai_ask` dans `crates/modlab-ui`.
- Décisions : `docs/01-architecture-decidee.md` §6 et `docs/adr/0004-binding-llama-cpp-2.md`.

## Ce qu'il ne faut pas y faire

- Ajouter un accès au Core ou au disque : les faits viennent toujours de l'hôte.
- Laisser le modèle décider ou ajouter un fait. Cette règle n'est encore qu'une consigne dans le texte envoyé au modèle : la sortie contrainte n'existe pas encore.
- Lancer le modèle pendant qu'un jeu tourne.
