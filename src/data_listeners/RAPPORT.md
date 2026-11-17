# RAPPORT — `src/data_listeners`, les neuf écouteurs de données

## 0. État de vérification

**VERT, 61 tests sur 61.**

1. `ssh deb '/home/zaccou/bin/port-check data_listeners'` — le dépôt entier, sous le verrou global.
   **Aucune des 16 localisations d'erreur ou d'avertissement n'est dans `src/data_listeners/`** (relevé
   par `grep '^ *--> '`) : elles sont dans `src/ai_work_11/`, `src/render/`, `src/tile_entities/`,
   `src/research/`, `src/colony/`, `src/manager_09/`, `src/ui_window_08/`, `src/commands/`. La
   compilation s'arrête donc avant les tests sur du code d'autres agents (`colony_flag` /
   `colony_sign` absents, une lifetime, un `From` non const, `f32: Eq`) ; ce n'est pas de mon périmètre
   et je n'y ai pas touché.
2. Faute de pouvoir lier le crate entier, le dossier a été compilé **seul**, sous le **même verrou**
   (`flock 9` sur `~/.port-build.lock`, comme `port-check`), dans un crate de deux lignes qui ne fait
   que `pub mod data_listeners;` : **`61 passed; 0 failed`**, aucun warning. Le seul import hors `std`
   est `crate::events::{Bus, Topic}` ; le crate de contrôle en porte un substitut de deux énumérations,
   et la résolution contre le **vrai** `src/events` est prouvée par le point 1 (aucune erreur de nom
   dans mon dossier).

Ce qui est mesuré, et non supposé : les 9 fichiers Java font **2 231 lignes** (`wc -l`), les 9
répertoires de données contiennent 22 + 163 + 29 + 3 + 1 + 74 + 52 + 327 + 2 fichiers, et les chiffres
de §3 viennent de `grep` sur les sources, pas d'une lecture en diagonale.

Deux tests m'ont contredit et ont été corrigés **contre la donnée**, pas contre l'attente : `feeds`
porte bien un `Fichier.java:ligne` (c'est sa forme utile), et trois lignes de la table citent désormais
le lecteur exact (`AbstractCraftingBuildingModule.java:296`, `CitizenDiseaseHandler.java:112`,
`ResearchManager.java:38`).

`src/events` n'a **pas** été modifié : le bus déjà porté (`bus.rs`, `topic.rs`) est la seule
implémentation, et ce module s'y raccorde par `use` et par deux constantes vérifiables — pas en créant
un second bus (§2, dernier paragraphe).

---

## 1. La table : neuf écouteurs, trois disciplines

La question « que recalcule chaque écouteur » n'a pas 9 réponses, elle en a **3**, et c'est la
discipline de remplacement qui décide ce qu'un fichier invalide **coûte**.

| Écouteur | Répertoire | Discipline | Ce qu'il alimente | Client |
| --- | --- | --- | --- | --- |
| `CitizenNameListener` | `citizennames` | clear-puis-remplit | `Colony.java:1949` (`nameStyle`) | clés seulement |
| `ItemNbtListener` | `compatibility` | clear-puis-remplit | `ItemStackUtils.java:580`, chaque comparaison d'objets | clés seulement |
| `CustomVisitorListener` | `visitors` | clear-puis-remplit | `TavernBuildingModule.java:182` | rien |
| `QuestJsonListener` | `colony/quests` | clear-puis-remplit | `QuestObjectiveEventHandler.java:76` | **JSON brut** |
| `CrafterRecipeListener` | `crafterrecipes` | reset-puis-remplit | les 163 recettes d'artisan | lignes lues |
| `ResearchListener` | `researches` | reset-puis-remplit | l'arbre de l'université | lignes lues |
| `DiseasesListener` | `colony/diseases` | construit-puis-échange | le tirage de maladie d'un citoyen | lignes lues |
| `StudyItemListener` | `study_items` | construit-puis-échange | `BuildingLibrary.java:53` | rien |
| `RecruitmentItemsListener` | `colony/recruitment_items` | construit-puis-échange | `TavernBuildingModule.java:196` | rien |

Les trois disciplines, mesurées sur le corps du `apply` :

- **clear-puis-remplit** — `store.clear()` **avant** la boucle (`CitizenNameListener.java:40`,
  `CustomVisitorListener.java:87`, `ItemNbtListener.java:47`, `QuestJsonListener.java:101`). Un refus
  coûte la copie précédente du fichier : elle était déjà effacée.
- **reset-puis-remplit** — un `reset()` qui fait partie du rechargement
  (`CustomRecipeManager.reset()`, `CrafterRecipeListener.java:48` ; `researchTree.reset()`,
  `ResearchListener.java:568`) et qui efface **aussi** les structures dérivées.
- **construit-puis-échange** — table locale, affectation à la fin (`DiseasesListener.java:180`,
  `StudyItemListener.java:148`, `RecruitmentItemsListener.java:169`). Un refus ne coûte rien.

En code : `mod.rs`, `Discipline`, et l'exécuteur unique `load`, qui implémente une seule fois les trois
comportements et les rend observables (`Report { accepted, refused, stopped, before, after }`).

---

## 2. L'ordre et la propagation

**L'ordre du rechargement est écrit par le mod lui-même** (`DataPackSyncEventHandler.java:24-40`) :

```
On /reload:
 -- JsonReloadListeners, TagsUpdatedEvent [server], OnDatapackSyncEvent [server],
    TagsUpdatedEvent [remote client], RecipesUpdatedEvent [client]
```

Les neuf écoutent donc **avant** tout le reste. Puis `discoverCompatLists` (`:52-57`) reconstruit quatre
caches qui ont besoin des tags, dont `resolveTemplates` — et c'est **là** que la table des recettes est
complète. Puis cinq paquets partent, dans un ordre fixe (`:68-73`) : compatibilité, recettes, arbre de
recherche, quêtes, maladies. Les trois écouteurs absents de cette liste (visiteurs, étude, recrutement)
n'ont aucune copie client.

Trois règles de propagation, chacune prouvée par un test :

1. **Un `remove` ne dépend pas de l'ordre du pack**, par deux mécanismes différents :
   - recettes : `removeRecipe` empile l'id (`CustomRecipeManager.java:115-122`) et la suppression réelle
     a lieu au **prochain `getRecipes`** (`:154`, `:249-275`) ;
   - recherches : `parseRemoveResearches` est une **passe séparée** (`ResearchListener.java:171`),
     commentée comme telle (`:168-171`).
2. **Un retrait à une profondeur emporte tout le sous-arbre**, et il faut parfois **deux passes** :
   `calcResearchTree` boucle `do { … } while (count changed)` (`:573-610`). Un enfant listé avant son
   parent survit à la passe qui tue le parent ; la suivante l'emporte. `research_plan::cascade` rend le
   nombre de passes, et le test du sous-arbre l'assure à 2.
3. **Une boucle est impossible dans l'arbre de recherche** : un cycle de parents demanderait
   `d = p + 1` et `p = d + 1`, ce que `depthOffset != 1` refuse (`:588-603`). Le seul garde-fou
   inventé est `MAX_PASSES` (port-only, nommé comme tel), et il ne peut pas être atteint : une passe
   qui change retire au moins une ligne.

**Et rien ne se propage par le bus.** Mesuré : aucun des neuf `apply` n'appelle `post`, aucun ne tient
une référence au bus. Les écouteurs sont en **pull** — ils remplacent une table que leurs lecteurs
consultent ensuite — donc `src/events` n'est pas sur leur chemin. Le seul événement du voisinage est
`CustomRecipesReloadedEvent`, et il ne part **pas** d'un rechargement : il est posté sur le bus **Forge**
depuis `CustomRecipeManager.handleCustomRecipeManagerMessage` (`:412`, poste à `:438`), c'est-à-dire
quand un **client** a reçu la table par le réseau. Le câbler à un rechargement annoncerait le mauvais
moment.

Le module se raccorde donc au bus existant **par référence et non par un second bus** : il importe
`crate::events::{Bus, Topic}`, et `timeline::POSTED_ON_RELOAD` (vide) plus `timeline::LOOKS_LIKE_RELOAD`
rendent ce fait vérifiable au lieu de le laisser en commentaire. Aucun `EventBus` n'est instancié ici.

---

## 3. Les refus, et ce que chacun coûte

La règle demandée — « une donnée invalide n'efface pas l'ancienne, elle est refusée par son nom » — est
**vraie pour un refus et fausse pour un abandon**, et le module sépare les deux (`Verdict::Refuse` /
`Verdict::Stop`).

Un refus est nommé `data_listeners.refusal.<règle>(<id>[.<valeur>])`, sans espace ni caractère exotique.
Chaque écouteur a son jeu fermé (`Refusal` local) et son `impl Refused`.

Les refus que Java **ne fait pas** et que le port ajoute, chacun avec sa raison :

| Refus | Ce que Java fait à la place | Ligne |
| --- | --- | --- |
| `unsupported_name_parts` | un citoyen nommé d'**un seul mot** | `CitizenData.java:655-658` |
| `empty_name_list` | `rand.nextInt(0)` au premier citoyen | `CitizenData.java:458-461` |
| `visitor_chance_out_of_range` | `2.0` gagne toujours, `-1` jamais, sans un mot | `:110-113` |
| `unknown_skill` | `Skill.valueOf` **au spawn**, non rattrapé : casse la taverne | `:200-205` |
| `unknown_study_item` | une entrée qui ne peut jamais correspondre | `:120`, `:84-88` |
| `unknown_compat_item` | une clé `null` dans `CHECKED_NBT_KEYS` | `:70-73` |
| `unknown_recipe_type` | aucun `default` : silence complet | `:53-66` |
| `declared_but_unhandled_recipe_type` | `recipe-multi-in` déclaré, jamais traité | `CustomRecipe.java:62` |
| `quest_order_index_out_of_range` | le contrôle de Java compare des **nombres**, pas des bornes | `:296-300` |
| `quest_order_not_operator` | `!` est construit, puis `evaluate` n'a pas de cas | `:335-338` vs `:401-452` |
| `malformed_research_remove` | `remove:false` est une **erreur** | `:548-552` |
| `recruit_tier_absent` | `NullPointerException` | `:100-101` |

Les refus que Java fait et que le port garde : item vide (`:146-150`), rareté hors 1-9 (`:152-156`),
pack vide (`:134-138`), `order` inconnu (`NameOrder.valueOf`), genre vide ou hors `m`/`f`
(`:140-144`), et les trois refus d'arbre de `research::tree` (parent manquant, écart de profondeur,
effet au-dessus du maximum), déjà portés là-bas et non dupliqués ici.

---

## 4. Le lien avec la colonie : aucun écouteur n'est par colonie

C'est la réponse mesurée, et elle corrige la prémisse de la mission. Les neuf écrivent une table
`static` ; **aucun ne touche une donnée sauvegardée d'une colonie**. `grep` sur les neuf fichiers ne
trouve qu'une **lecture** de colonie (`QuestJsonListener.java:399`, l'évaluation d'un déclencheur).

Donc : un rechargement remplace la **spécification**, et ce qu'une colonie voit changer est ce qu'elle
relit ensuite. Survivent intacts :

- un nom déjà donné (`CitizenData.name`),
- la progression de recherche d'une colonie (le `ILocalResearchTree` de
  `ResearchManager.java:38`), alors que l'arbre **global** est remis à zéro
  (`ResearchListener.java:568`),
- un visiteur déjà apparu et son coût de recrutement,
- une requête déjà déposée.

**Un seul endroit répond différemment par colonie, et c'est un bug.** La même table de neuf paliers
(`recruitment`) donne au niveau 4 de taverne une bande de tirage `1..=12` : trois paliers qu'aucun pack
ne déclare. `RECRUIT_COSTS.get(rarity)` rend `null`, la ligne suivante déréférence
(`RecruitmentItemsListener.java:100-101`), et `TavernBuildingModule.java:197-201` a bien écrit
`if (cost == null) return null;` — un garde placé là où il ne peut pas se déclencher. Le port rend
`Refusal::TierAbsent { rarity }`, ce que ce garde voulait dire.

---

## 5. Ce que la lecture a trouvé, et qui n'était pas dans la commande

Chacune est vérifiable sur la ligne citée.

1. **`recipe-multi-in` est déclaré et jamais traité.** `CustomRecipe.java:62` le définit ;
   `CrafterRecipeListener.java:53-66` n'a ni ce cas ni `default`. Un pack qui l'utilise obtient un
   artisan qui ne fabrique pas, sans un mot de log. Aucun des 163 fichiers livrés ne l'utilise
   (161 `recipe`, 2 `recipe-template`, comptés au `grep`).
2. **`raise` : la taverne niveau ≥ 4 plante par NPE** (§4). Le commentaire de
   `TavernBuildingModule.java:197` prouve l'intention.
3. **`Skill.valueOf` au spawn, non rattrapé** (`:200-205`) : une faute de frappe dans un
   `primaryskill` casse la taverne à chaque tirage, pas le chargement.
4. **Le seul `catch` étroit du paquet** : `CrafterRecipeListener.java:63` attrape `JsonParseException`
   là où ses huit voisins attrapent `Exception`. Une `IllegalArgumentException` de `CustomRecipe.parse`
   quitte `apply`, abandonne la fin du pack et sort du listener — le magasin étant **déjà** remis à
   zéro (`:48`). `caught(Failure)` garde l'asymétrie en valeur.
5. **Trois `return` qui veulent dire « abandonne le pack »** (`DiseasesListener.java:145`,
   `StudyItemListener.java:141`, `RecruitmentItemsListener.java:137`) : ils laissent l'ancienne table
   entière et jettent tous les fichiers suivants **en silence**. Ailleurs c'est `continue`.
6. **`remove:false` est une erreur** (`ResearchListener.java:548-552`) — le seul booléen faux qui soit
   une faute.
7. **Le contrôle de `triggerOrder` compare des nombres, pas des bornes** (`:296-300`) : `"1 || 3"` avec
   2 déclencheurs passe, et `triggerMap.get("3")` rend `null` puis NPE (`:445`). Idem pour l'indice `0`.
8. **`!` est un opérateur à moitié implémenté** : construit (`:335-338`), sans cas dans `evaluate`
   (`:401-452`) → NPE. Aucun des 4 ordres livrés ne l'utilise.
9. **`voiceprofile` est lu et jeté** (`:207-210`) : `getVoiceProfile()` sans setter.
10. **L'ordre du pack visiteur décide qui apparaît**, et cet ordre est celui d'une `HashMap`
    (`:87-90`, `:146`) : deux visiteurs à 0,5 ne se partagent pas 50/50, le premier dans l'ordre de
    hachage prend sa moitié d'abord. `visitors::first_chance` rend l'ordre explicite.
11. **`parts` n'est pas validé** : toute valeur autre que 2 ou 3 donne un nom d'un seul mot
    (`CitizenData.java:655-658`), et `generateName` se rappelle sans borne quand le nom est pris
    (`:660-670`) — donc une file de noms trop petite finit en `StackOverflowError`. `room_for` donne
    la borne.
12. **`CHECKED_NBT_KEYS` a deux producteurs** : le listener côté serveur (`ItemNbtListener.java:47`) et
    le lecteur de paquet côté client (`CompatibilityManager.java:263`). Une clé `null` traverse le
    réseau en id `-1` et revient en `null` : aucune comparaison ne la trouve
    (`ItemStackUtils.java:580`), sans trace.

---

## 6. Testabilité : 61 tests, dont aucun ne demande de serveur

Le module n'importe que `std::collections::BTreeSet`, et rien d'autre. Les tirages arrivent faits, les
registres arrivent en `&dyn Fn(&str) -> bool`, le niveau de taverne est un paramètre. C'est ce qui
permet de prouver §2.2 (deux passes), §4 (niveau 4 → palier 12) et §5.7 (`"1 || 3"` accepté par Java)
par assertion plutôt que par relecture.

Les deux tests qui ont dû être corrigés l'ont été contre la donnée, jamais contre l'attente : la forme
de `feeds` (§0) — le reste a passé du premier coup, y compris les cas où Java se trompe.

---

## 7. Mesures, et l'écart que je dois dire

| Mesure | Valeur |
| --- | --- |
| fichiers du module | 12 (`mod`, `timeline`, 8 domaines, `MANQUANT.md`, ce rapport) |
| lignes de code, hors tests et hors commentaires | **1 091** |
| lignes de tests | 851 |
| **tests** | **61**, tous verts |
| dépendances ajoutées | aucune |
| imports | `std::collections::BTreeSet`, `crate::events::{Bus, Topic}` |
| Java couvert | 2 231 lignes (les 9 écoutent + le modèle `Disease`, dont le modèle est déjà porté par `citizens::happiness::disease`) |
| modules du dépôt modifiés hors dossier | aucun |
| warnings sur le dossier | aucun |

**L'écart : la mission visait 600 à 900 lignes et j'en rends 1 091** (hors tests et commentaires), soit
un cinquième de plus. La cause n'est pas l'étalement mais la découpe en dix domaines, chacun avec sa
`Row`, son jeu de refus et son implémentation de `Refused` — et `mod.rs` porte à lui seul la machine de
rechargement (229 lignes) que les neuf partagent. Si c'est trop, les coupes possibles sont nommées et
sans perte de règle : `nbt_keys` (30 lignes de code) et `study_items` (45) pourraient disparaître au
profit de leurs domaines respectifs, et `timeline` (42) n'a aucun test dépendant d'un ordonnanceur.
Je ne les ai pas faites parce qu'elles déplacent des règles hors du fichier qui les documente.

Résultat des compilations : §0, tenu à jour. Les erreurs qui restent hors de `src/data_listeners/`
appartiennent à d'autres agents (`src/tile_entities/`) et ne sont pas de mon périmètre.
