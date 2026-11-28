# ADR 0003 — Import MO2 en lecture seule, par référence au chemin source, hors du store CAS

- **Statut :** accepté (rétroactif — décision prise en Phase 1, commits `326515e` puis `a9a2bb8`, 15 septembre 2026)
- **Portée :** `crates/modlab-game-skyrim/src/mo2.rs`, `modlab_core::loadout` (`ImportEntry`, `import_mods`)
- **Références :** `docs/01-architecture-decidee.md` §2 (décision 3), §5 (CAS, révisions, cycle transactionnel), §13 (Phase 1 : « import d'instance MO2 »), Annexe A Q8/Q9

---

## Contexte

Le jalon de Phase 1 exige de jouer à Skyrim **avec la vraie modlist de Benjamin** — soit ~400 mods déjà installés et ordonnés dans son instance MO2, qui reste pendant toute cette phase son outil de production réel. L'import MO2 est donc simultanément une fonctionnalité produit et l'unique source de données de test grandeur nature.

Deux propriétés s'opposent à première vue :

- `01-architecture-decidee.md` §5 fait du **CAS** (`store/blobs/`) une des trois idées structurantes : chaque contenu unique stocké une fois, dédup, provenance, snapshots gratuits.
- Mais l'instance MO2 de Benjamin est **son installation qui marche**. Le premier import qui la modifierait — ou qui en dupliquerait plusieurs dizaines de Go dans `F:\ModLab\store\` avant même que le reste de la chaîne soit prouvé — serait un très mauvais échange.

## Options considérées

1. **Import « déménagement » : ingérer tout le contenu dans le CAS** dès l'import (hash de chaque fichier, écriture en blobs, dossiers de mods reconstruits depuis le store). Architecturalement « pur » et conforme à la cible §5. Mais : volume énorme avant toute valeur démontrée, durée d'import majeure, et surtout une chaîne CAS → déploiement → lancement à prouver d'un seul bloc, sans pouvoir isoler la panne.
2. **Import « adoption » : reprendre en main les dossiers MO2** (les déplacer, réécrire `meta.ini`, gérer les profils MO2 depuis ModLab). Casse l'instance MO2 de Benjamin, qui n'a plus de filet.
3. **Import par référence, en lecture seule** : ModLab lit `modlist.txt` / `plugins.txt` / `loadorder.txt`, crée ses **propres** lignes `mod_` et `loadout_entry` en base, et chaque mod **pointe vers son dossier source d'origine**. Rien n'est copié, rien n'est écrit côté MO2.

## Décision

**Option 3.** L'import MO2 est **strictement en lecture seule et non destructif** :

- `mo2::read_profile` ne fait que parser. Rien dans `mo2.rs` n'installe, ne déploie, ni ne touche au dossier `mods/` de MO2.
- `to_import_entries` traduit vers le modèle **générique** du Core (`modlab_core::loadout::ImportEntry`) — le Core ne sait pas ce qu'est MO2, conformément à §3. Le mapping est volontairement séparé de l'écriture (`loadout::import_mods`), pour que le parsing/mapping soit testable et fiable indépendamment du chemin d'écriture.
- Chaque mod importé est enregistré avec son **`source_path` d'origine**. **Le contenu n'entre pas dans le store CAS.** Le déploiement USVFS (ADR 0001) lie directement ces chemins sources, ce qui ne demande aucune copie.

## Conséquences

**Ce que ça nous donne.**

- L'instance MO2 de Benjamin reste intacte et utilisable ; ModLab peut échouer sans dommage. C'est la condition qui a permis de tester chaque brique de la Phase 1 contre les vraies données plutôt que contre des fixtures.
- Import quasi instantané, quelle que soit la taille de la modlist.
- La chaîne a pu être prouvée **maillon par maillon** (import → conflits → déploiement → lancement SKSE) au lieu d'un seul bloc opaque.
- Le format MO2 a été documenté **contre un vrai profil, pas supposé** : `+Nom` / `-Nom` pour un mod géré (activé / désactivé), `*Catégorie: Nom` pour de l'informatif (Creation Club, DLC, « Unmanaged ») qui n'a **jamais** de dossier sous `mods/` et que les appelants ne doivent pas chercher à résoudre ; `plugins.txt` reprend la convention native du jeu (`*` = actif) ; `loadorder.txt` est déjà trié.

**Ce que ça nous coûte — et c'est le point que toute session future doit lire avant de supposer le contraire.**

1. **Le CAS n'est pas câblé pour les mods importés.** `store` existe et fonctionne (Phase 0, écritures en flux), mais l'import MO2 ne passe pas par lui. Il n'y a donc, pour ces mods, **ni dédup, ni provenance par hash, ni snapshot de contenu**. Ne pas partir du principe que « le CAS est branché » parce qu'il existe.
2. **Conséquence directe sur le cycle transactionnel (§5).** Un snapshot d'un loadout importé ne capture aujourd'hui que **l'état en base** (quels mods, quel ordre, activés ou non), pas le contenu des fichiers. Le rollback restaure une configuration, pas des octets. C'est acceptable en Phase 1 parce que le déploiement USVFS n'écrit rien (ADR 0001) — donc il n'y a rien à restaurer côté disque — mais ça cesse de l'être dès qu'un backend écrit réellement dans le jeu (matérialisation, fichiers racine ENB/Root Builder). **Ce backend-là ne doit pas être livré avant l'ingestion CAS.**
3. **ModLab dépend du chemin source.** Si Benjamin déplace ou supprime son instance MO2, les mods importés pointent dans le vide. Aucune détection de ce cas n'existe encore.
4. **Aucun verrou entre les deux outils.** Rien n'empêche MO2 et ModLab de tourner en même temps sur les mêmes dossiers. En lecture seule et sans écriture au déploiement, le risque est faible, mais il n'est pas nul et il n'est pas géré. (À noter : USVFS exige une *instance nommée* distincte de celle de MO2 — voir ADR 0001.)
5. **Les profils MO2 ne sont pas synchronisés.** L'import est un instantané, pas un lien vivant : une modification dans MO2 après l'import n'est pas reflétée.

## Chemin de sortie (à ne pas faire maintenant)

L'ingestion CAS reste la cible de §5 et arrivera — mais comme une **étape explicite et distincte de l'import** (« adopter le contenu dans le store »), pas en la glissant dans le chemin d'import existant. Elle devient obligatoire avant tout backend de déploiement qui écrit réellement sur disque. Tant que la Phase 1 ne l'exige pas, ce n'est pas un blocage : c'est une frontière de périmètre, écrite ici pour qu'elle ne soit pas franchie par inadvertance.
