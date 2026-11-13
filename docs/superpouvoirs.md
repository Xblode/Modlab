# Superpouvoirs privés

> Extrait de `00-cadrage-technique.md` (§8 entière) le 2026-11-13. Texte recopié mot pour mot.

## 8. Superpouvoirs privés

Ce que ChatGPT a listé (forensics, transaction engine, time machine, auto-diagnose, graphe de dépendances, experiment lab, recettes, self-healing contrôlé, KB, intégrations système) reste valable. J'ajoute :

| 💡 Superpouvoir | Pourquoi c'est réservé à un outil perso | Comment |
|---|---|---|
| **`git bisect` pour mods** | Un outil distribué n'oserait pas relancer ton jeu 10 fois tout seul | Recherche dichotomique automatique du mod fautif : désactive la moitié, lance, détecte le crash (process exit + crash log), itère. |
| **Loadout as code (git)** | Trop "dev" pour le grand public | Branches = expériences, diff = review, `git log` = timeline, et Claude sait tout faire avec git. |
| **Patchers Mutagen à la volée** | Il faut un dev sous la main | "Claude, fais un patch qui met les prix de tous les marchands ×2" → patcher Synthesis généré, compilé, exécuté, sortie capturée comme mod, réversible. |
| **Analyse de crash log native** | Les analyseurs (CLASSIC) sont des outils séparés | Parser Crash Logger/Trainwreck : DLL impliquées, plugin/form IDs → mods ; croiser avec les **derniers changements** (révisions) → hypothèses classées avec preuves. |
| **Session recorder** | Télémétrie = tabou en distribué | À chaque lancement : révision, durée, FPS (PresentMon), crash oui/non, logs Papyrus. Comparaison A/B automatique entre révisions. |
| **Analyse des sauvegardes** | Niche | Parse `.ess` : plugins manquants, scripts orphelins, taille de la pile → "cette save est-elle compatible avec ce loadout ?" |
| **Diff de plugins lisible** | Spriggit est un outil dev | ESP → YAML → diff humain (et Claude peut *expliquer* ce qu'une mise à jour de mod change réellement). |
| **CoW / Dev Drive** | Dépend de ton matériel précis | Copies instantanées, isolation totale, zéro VFS pour les outils réfractaires. |
| **Recettes** | Scripts perso | "Après une MAJ graphique : snapshot → Nemesis → DynDOLOD → LOOT → santé → bench → rollback si régression" — écrites en YAML, exécutées par le moteur, éditables par Claude. |
| **Comparateur visuel** | Créatif | Capture avant/après (ENB, textures) alignées, curseur de comparaison. |
| **Tâches planifiées** | — | Chaque nuit : vérifier les MAJ Nexus, produire un digest des changelogs (Claude Code peut le faire en tâche planifiée). |
| **Mode "je sais ce que je fais"** | Grand public = danger | Écriture directe dans le dossier du jeu, édition d'INI, suppression forcée — mais **toujours** tracées et réversibles. |

⚠️ Ce qu'on **ne** fait pas même en perso : contourner les protections Nexus (téléchargements automatisés sans Premium, scraping agressif). Ce n'est pas une question de morale, c'est que c'est fragile et ça finit banni.

### 8.1 ✅ Diagnostic local par petit modèle embarqué (IA locale, sans abonnement)

**Décidé le 15 septembre 2026.** Pour des actions légères (diagnostiquer le dernier crash, résumer un log, reformuler un diagnostic) : un LLM local, embarqué dans l'app, sans serveur externe, sans abonnement, **zéro configuration** pour toi.

**Le principe : les règles font le diagnostic, le modèle ne fait que rédiger.** Un LLM (même gros) hallucine facilement sur des détails techniques précis (FormID, noms de plugins exacts). Le pipeline retenu :

1. **Moteur de règles déterministe** (parsing Crash Logger/Trainwreck, base de signatures connues, croisement avec les dernières révisions — cf. §8 "Analyse de crash log native") → identifie la vraie cause et produit un JSON structuré déjà correct.
2. **Le petit LLM local** reçoit ce JSON et le reformule en français clair — il ne décide rien, il rédige.
3. **Grammaire contrainte (GBNF, native à llama.cpp)** : force le modèle à ne produire que les champs qu'on lui fournit, dans une structure fixée — empêche littéralement le modèle d'inventer un fait hors des données transmises. Double verrou anti-hallucination (règles + grammaire), pas juste un prompt "sois honnête".

**Stack technique :** **LLamaSharp** (binding C#/.NET officiel de llama.cpp) — backends CPU, CUDA et Vulkan sans compilation à gérer, format de modèle = simple fichier `.gguf`. Cohérent avec le choix moteur C# (§5.1).

**Modèles proposés au choix, dans Réglages :**

| Modèle | Taille (Q4) | RAM/VRAM | Profil |
|---|---|---|---|
| **Qwen2.5-1.5B-Instruct** ✅ *(défaut)* | ~1 Go | ~3 Go | Le plus rapide, universel, tourne sur n'importe quelle machine |
| Llama-3.2-3B-Instruct | ~2 Go | ~3-4 Go | Meilleur équilibre qualité/vitesse |
| Phi-3.5-mini-Instruct (3.8B) | ~2,4 Go | ~4 Go | Meilleure qualité de rédaction des trois "légers" |
| Qwen3 8B *(palier "Performance")* | ~5,7 Go | 8 Go VRAM+ | Auto-suggéré si le GPU détecté a 8 Go de VRAM ou plus (ex. RTX 3070 Ti) — tient entièrement en VRAM, 55-70 tokens/s |

⚠️ **Plafond utile sur 8 Go de VRAM : ne pas dépasser 8-9B.** Au-delà (12-14B), le modèle déborde sur la RAM système et la vitesse s'effondre (4-11 tokens/s) — moins bon, pas juste plus lent. Le palier "Performance" s'arrête donc à 8-9B par conception, pas par limitation arbitraire.

**Sélection du modèle :** téléchargement auto du modèle par défaut (Qwen2.5-1.5B) au premier lancement, les autres modèles se téléchargent à la demande depuis Réglages (~5,4 Go pour les trois "légers" au total si on veut tout avoir sous la main). Détection VRAM au premier lancement → suggestion (jamais imposée) d'un palier supérieur si le GPU le permet.

**Cycle de vie — pour ne jamais gêner le jeu ou les perfs :**

1. **Chargement à la demande (lazy load)** : le modèle ne charge en VRAM/RAM que quand une action IA locale est réellement demandée — jamais résident en permanence.
2. **Déchargement automatique après inactivité** (llama.cpp gère nativement un timeout d'inactivité).
3. **Jamais chargé pendant qu'un jeu tourne** : le GPU doit rester 100 % dédié au jeu. Si une action locale est demandée pendant une session de jeu, elle est **mise en file d'attente** (pas de refus, pas de blocage) et **traitée automatiquement dès la fin de la partie** — sans que tu aies à relancer quoi que ce soit.

**Mis en place visuellement dans la maquette** (`docs/design/shell-maquette.html`, v21) : le bouton "Diagnostique le dernier crash" du composer passe en état "En attente de fin de partie" si le jeu tourne au moment du clic, puis se déclenche seul (panneau Claude ouvert, toast de confirmation) dès que tu cliques "Arrêter" ; le sélecteur de modèle est accessible depuis Réglages (⚙, panneau gauche) avec le GPU détecté affiché en tête de liste ; le panneau Claude affiche en permanence quel modèle local a traité la demande, avec un rappel visuel du principe "pré-mâché par des règles, le modèle ne fait que rédiger".
