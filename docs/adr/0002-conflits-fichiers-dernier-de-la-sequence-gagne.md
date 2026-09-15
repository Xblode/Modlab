# ADR 0002 — Conflits de fichiers : « le dernier de la séquence fournie gagne », et rien de plus

- **Statut :** accepté (rétroactif — décision prise en Phase 1, commit `c530fee`, confirmée empiriquement par `c1b508a`, 15 septembre 2026)
- **Portée :** `crates/modlab-core/src/conflicts.rs` (Core), vérification dans `crates/modlab-game-skyrim/src/usvfs.rs`
- **Références :** `docs/01-architecture-decidee.md` §3 (le Core ne connaît aucun jeu), §5, §7.1 (`find_conflicts`), §13 (Phase 1 : « conflits fichiers »)

---

## Contexte

La Phase 1 demande la détection des conflits de fichiers : plusieurs mods fournissent le même chemin relatif, un seul est réellement vu par le jeu. Deux questions se posent, et elles sont **distinctes** :

1. **Quels fichiers sont en conflit, et lequel gagne ?** — mécanique, générique, vraie pour n'importe quel jeu dont les mods sont des dossiers de fichiers.
2. **Dans quel ordre de priorité faut-il classer les mods ?** — spécifique à la source de cet ordre. Et l'import MO2 a fait remonter une vraie inconnue : **on ne sait pas encore dans quel sens l'ordre des lignes de `modlist.txt` correspond à la priorité affichée dans l'UI de MO2** (haut-de-fichier vs. haut-de-liste). Ça n'a pas pu être vérifié contre l'UI de MO2 pendant la Phase 1.

Le piège était donc de laisser la question 2 contaminer la réponse à la question 1.

## Options considérées

1. **Un module de conflits qui calcule lui-même la priorité**, en lisant l'ordre MO2 et en devinant sa direction. Rapide à écrire, donne tout de suite un résultat « complet »… et inscrit une hypothèse non vérifiée au cœur du Core. Si la direction est fausse, **chaque gagnant affiché est inversé**, silencieusement, et la correction ultérieure invalide tout ce qui aura été construit dessus.
2. **Attendre d'avoir tranché la direction MO2** avant d'implémenter quoi que ce soit sur les conflits. Bloque un livrable de Phase 1 sur une vérification manuelle dans une UI tierce.
3. **Séparer strictement les deux questions** : le Core répond à la question 1 en prenant l'ordre comme une **entrée**, et refuse explicitement de répondre à la question 2.

## Décision

**Option 3.** `modlab_core::conflicts::find_conflicts` implémente exactement un modèle : **le dernier mod de la séquence fournie par l'appelant gagne.** Il ne détermine pas cette séquence, il la reçoit. Le contrat est écrit dans le doc du module et dans celui de la fonction : « `mods` doit déjà être en ordre de priorité, priorité la plus faible en premier ».

`FileConflict` conserve **tous** les contributeurs dans l'ordre reçu, pas seulement le gagnant — l'information nécessaire à l'UI (« qui perd contre qui ») est là sans que le Core ait à décider quoi que ce soit de plus.

La question de la direction de `modlist.txt` reste **ouverte et documentée comme telle**, dans `mo2.rs` (module doc) et dans `conflicts.rs` (module doc). Elle sera tranchée quand elle sera réellement nécessaire, en vérifiant contre l'UI de MO2 — pas avant.

## Conséquences

**Ce que ça nous donne.**

- Le module est **générique et conforme à §3** : aucune notion de Skyrim, de MO2, d'ESP ou de `Data/` n'y entre. Minecraft (Phase 2) le réutilisera tel quel.
- L'hypothèse s'est trouvée **confirmée de façon indépendante au niveau du moteur de déploiement**, ce qui n'était pas gagné d'avance : le test `two_mods_overlay_onto_one_destination_last_linked_wins` (dans `usvfs.rs`, exécuté contre la vraie `usvfs_x64.dll`) établit que `VirtualLinkDirectoryStatic` suit lui aussi un « dernier lié gagne ». Le modèle du Core et le comportement réel d'USVFS coïncident donc — la détection de conflits prédit ce que le jeu verra vraiment, ce n'est plus une convention interne.
- Séparer les deux questions rend la seconde **corrigeable sans rien casser** : le jour où la direction MO2 est établie, on inverse une séquence à la frontière de l'adaptateur, et aucun code du Core ne bouge.

**Ce que ça nous coûte.**

- L'UI ne pourra pas afficher « ce mod gagne » avec une confiance totale **tant que la direction MO2 n'est pas tranchée** pour les loadouts issus d'un import MO2. C'est un manque assumé et explicite, préférable à un affichage faux et confiant.
- Chaque appelant porte la responsabilité de fournir le bon ordre. Aujourd'hui `loadout::entries` renvoie les lignes en `position` ascendante, ce qui fait du mod de position la plus haute le gagnant — cohérent, mais c'est une convention à respecter par tout nouvel appelant, pas une garantie du type.

**Un affinage réel, trouvé et non deviné.** L'import de la vraie instance MO2 de Benjamin a fait apparaître un faux positif massif : MO2 écrit un `meta.ini` dans **chaque** dossier de mod qu'il gère, donc tous les mods « entraient en conflit » deux à deux sur ce fichier. `list_files_relative` ignore désormais une courte liste de fichiers de bookkeeping (`meta.ini`, `thumbs.db`, `desktop.ini`), avec un test de non-régression. La liste n'est **pas exhaustive** : tout autre outil déposant un fichier de service dans les dossiers de mods produira le même bruit et devra être ajouté.

## Ce que cette décision ne tranche pas

- **La direction de priorité de `modlist.txt`** — reste ouverte, c'est le point à reprendre en premier sur ce sujet.
- **Les conflits de *records*** (niveau xEdit/Mutagen, §4) : sans rapport. Ce module ne parle que de fichiers. Les conflits de records restent au programme de la Phase 4.
- **La résolution** des conflits (règles, priorités par extension, patchs) : hors périmètre Phase 1, on détecte et on explique, on ne réarbitre pas.
