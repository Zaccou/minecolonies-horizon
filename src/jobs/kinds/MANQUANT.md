# MANQUANT — ce que les règles des métiers auraient voulu et que le port n'a pas

Rien de manquant **du SDK** : ce module n'appelle aucune API de l'hôte. Il n'utilise que `std`
et `crate::jobs` (`Job`, `ToolKind`, `Employment`, `Skill`, `experience`), et il compile sans
ajouter une ligne à `crates/`. Les manques ci-dessous sont des types **du port** — chacun est
contourné par une valeur nommée, jamais par une invention silencieuse.

## 1. Un identifiant d'item `const`

**Ce que je voulais faire** : écrire `Accepted { item: ItemName, .. }` dans une table `const`,
pour qu'un item refusé soit refusé à la compilation et non par un test.

**Ce qui existe** : `crate::inventory::ItemName` (`src/inventory/stack.rs:34`), qui valide le
format mais **possède** une `String` et se construit par `ItemName::new(&str) -> Option<Self>`,
donc pas dans une `const`.

**Ce que j'ai fait** : `item: &'static str`, avec la même écriture que Java (tag préfixé `#`,
`namespace:path` sinon). Les tests ne vérifient pas le format ; c'est le prix. Le jour où
`ItemName` aura un constructeur `const` — ou un type `ItemId` en `&'static str` validé une fois
au chargement — une ligne de `Accepted` changera de type sans changer de valeur.

## 2. Un type de point de recherche

**Ce que je voulais faire** : donner au chercheur un `produces` réel — « un point de recherche
par action d'étude » (`EntityAIWorkResearcher.java:96`, `LocalResearch.research()`).

**Ce qui existe** : rien. La recherche vit dans `crate::colony` comme un compteur de colonie, pas
comme un item.

**Ce que j'ai fait** : `produces: &[]` et la phrase dans le commentaire de la ligne. Un
`Output::Research` inventerait un type que personne ne consomme.

## 3. Un type d'entité nommée

**Ce que je voulais faire** : dire *quelle* monture le palefrenier produit et *quel* citoyen le
croque-mort ressuscite.

**Ce qui existe** : rien de typé ; le port n'a pas de registre d'entités.

**Ce que j'ai fait** : `Output::Entity(&'static str)` avec `"minecraft:horse"` et
`"minecolonies:citizen"` — deux noms, pas deux types. C'est suffisant pour lire la règle et
insuffisant pour la déclencher ; quand `crate::citizens` aura un constructeur d'entité, cette
variante prendra son type.

## 4. « Combien par requête »

**Ce que je voulais faire** : porter le nombre d'items qu'un métier demande quand il manque de
quelque chose — 64 composts pour le fermier (`EntityAIWorkFarmer.java:234`), 16 remèdes pour le
soignant (`EntityAIWorkHealer.java:48`), `8` de plus que la paire pour l'éleveur
(`AbstractEntityAIHerder.java:66`), 16 feuilles pour le professeur
(`EntityAIWorkTeacher.java:39`), 100 items d'étude pour l'étudiant (`EntityAIStudy.java:155`),
`ceil(pile/4)` pour le planteur (`AbstractPlantationModule.java:84`).

**Ce qui existe** : `crate::requests` sait porter une demande, mais `JobRules` n'a pas de champ
« quantité » et le type de requête n'est pas encore branché sur un métier.

**Ce que j'ai fait** : une constante nommée dans `durations` quand le nombre est un **compte**
(`Unit::Count` : `REQUEST_COUNT`, `PAPER_TO_REQUEST`, `EXTRA_BREEDING_ITEMS_REQUEST`…), et une
phrase dans le commentaire sinon. Le champ manquant est « la quantité d'une demande », qui est
l'affaire du système de requêtes, pas de la règle du métier.

## 5. Une valeur non entière dans une constante

**Ce que je voulais faire** : porter les coefficients qui ne sont pas des entiers —
`CAVALRY_DAMAGE_MULTIPLIER = 1.00`, `MOUNT_DAMAGE_SPLIT = 0.20`,
`PERCENT_CHANGE_FOR_GROWTH = 0.2`, `FEED_CHANCE = 0.1`
(`AbstractEntityAIHerder.java:108`), `RESURRECT_*_WEIGHT`.

**Ce qui existe** : `Constant { value: i32, .. }` — un entier, parce que la mission demandait des
durées en ticks.

**Ce que j'ai fait** : les coefficients qui pilotent une **formule** sont des `pub const f64`
nommés dans le fichier de famille (`CAVALRY_RANGE_MULTIPLIER`, `MOUNT_DAMAGE_SPLIT`,
`RESURRECT_BUILDING_LVL_WEIGHT`, `BUILDER_MINING_SPEED`, `FURNACE_ACCELERATION_PER_TEN_LEVELS`…) ;
ceux qui ne sont qu'un seuil restent en commentaire avec leur ligne. Aucun n'est devenu un
`i32` arrondi.

## 6. Un tick par citoyen

**Ce que je voulais faire** : rien — c'est hors périmètre, et c'est précisément pourquoi ce
module s'arrête où il s'arrête.

**Ce qui n'existe pas** : le hook de tick d'une entité. `entity_tick_pre` est **global**
(`crates/horizon-mod-vocab/src/hooks/entity_tick_pre.rs:19`), donc un citoyen ne peut pas encore
exécuter son métier. Ce module porte donc la règle et pas l'exécution : c'est la limite que
`src/jobs/mod.rs` documente déjà en tête, et elle n'a pas bougé.
