# MANQUANT — ce que le chargeur de structures réclame et que le port n'a pas

Écrit à la main pour ce module. Les autres `MANQUANTS.md` du dépôt sont générés par un
script ; celui-ci ne l'est pas, et chaque entrée dit **ce qui manque, d'où ça vient, et ce
que ça coûte** — jamais « à faire plus tard » sans mesure.

Les quatre fichiers de `src/structures/` n'appellent aucune fonction de l'hôte : ils lisent
`crate::ai::WorldFacts`, `crate::building::{Kind, Footprint}` et rien d'autre. Tout ce qui
suit est donc un fait qui doit venir du **dehors**, plus une règle qui ne peut pas être
écrite avec les types présents.

---

## 1. L'ancre du template (`primary_offset`) n'existe plus

**Ce qui manque.** `Placing::anchor`, l'offset du bloc primaire à l'intérieur du template.
Java le lit sur le blueprint (`Blueprint.getPrimaryBlockOffset`,
`strukturize-src/…/blueprints/v1/Blueprint.java:577-584`), et c'est lui qui place la boîte :
`zeroPos = blockPos.subtract(primaryBlockOffset)`
(`minecolonies-src/…/placementhandlers/main/SurvivalHandler.java:296-303`).

**D'où ça vient.** La conversion `.blueprint` → `.nbt` a rendu l'ancre dans
`Conversion::anchor` mais ne l'a pas écrite dans le fichier : `optional_data.structurize.
primary_offset` → **rien** (`RAPPORT_BP2NBT.md` §2, tableau des correspondances). Mesuré :
les 9 515 `.nbt` du corpus n'ont que cinq tags à la racine — `DataVersion`, `size`, `palette`,
`blocks`, `entities` (relevé sur `original/fundamentals/builder1.nbt` et
`colonial/fundamentals/townhall1.nbt`).

**Ce que ça coûte.** Un appelant doit fournir l'ancre ; `Placing::new` la met à `[0, 0, 0]`,
c'est-à-dire « le bloc primaire est le coin du template ». C'est faux pour tous les
bâtiments dont le hut block n'est pas au coin — probablement la majorité. La boîte rendue
par `Placing::boxed` reste **cohérente avec la pose** (elle utilise la même transformation),
donc la vérification de terrain ne ment pas *par rapport à ce que l'appelant a déclaré* ;
c'est la déclaration qui peut être fausse.

**La sortie.** Réécrire les 9 515 fichiers avec un tag d'ancre (hors de ce module : c'est
`port-blueprint`, et ça change le corpus pour tout le monde), ou la relire depuis les
`.blueprint` d'origine. Les deux sont hors de `src/structures`.

---

## 2. `Block` ne distingue pas le sol faible du sol porteur

**Ce qui manque.** La différence entre « ça porte » et « ça porte mal ». Java l'a :
`BlockUtils.isWeakSolidBlock` (`strukturize-src/…/util/BlockUtils.java:808-822`) reconnaît la
terre, le sable et le gravier par un tag (`ModTags.WEAK_SOLID_BLOCKS`) plus
`canBlockSurviveWithoutSupport`, et l'étape `WEAK_SOLID`
(`BuildingProgressStage.java:17`, `AbstractEntityAIStructure.java:403-413`) les remplace par
un bloc plein avant de monter les murs.

**D'où ça vient.** `crate::ai::Block` est volontairement exhaustif à cinq valeurs — `Air`,
`Fluid`, `Solid`, `Barrel`, `Undestroyable` (`src/ai/facts.rs:89-102`) — et `Solid` y veut
dire « un bloc ordinaire ». Il n'y a pas de case pour « ordinaire mais mou », et ce module
n'a pas le droit de modifier `src/ai/`.

**Ce que ça coûte.** Un terrain posé sur du sable sort `Verdict::Ready` alors que le builder
Java aurait d'abord posé une fondation. Le refus n'est donc pas dans le bon sens : le port
est **plus permissif** que Java, jamais plus strict. `terrain::survey` ne peut pas le dire,
et le documenter vaut mieux que d'inventer une liste de blocs « mous » que rien dans
`WorldFacts` ne confirmerait.

**La sortie.** Une sixième valeur dans `crate::ai::Block` (ou une carte `Pos → Solidness`
dans `WorldFacts`), décidée par le module `ai`, pas ici.

---

## 3. Aucun accès au disque : l'index est construit par l'appelant

**Ce qui manque.** L'énumération de `data/minecolonies/structure/`. `Index::from_files`
prend une liste de triplets `(style, chemin, taille)` ; personne dans ce module ne lit un
dossier, et un mod wasm ne le peut pas (pas de système de fichiers, et
`crate::wiring` documente déjà ce trou pour la sauvegarde du monde).

**D'où ça vient.** Les 9 515 fichiers et leurs tailles sont un fait du corpus, pas un fait
du jeu. `size` est un tag du fichier (`NOTES_TEMPLATE_NBT.md` §1.3) : le seul moyen de le
connaître est de lire le fichier, une fois, au moment où l'on construit l'index.

**Ce que ça coûte.** Un `.hmod` de contenu peut livrer la liste comme donnée (un JSON de
9 515 lignes, ou un index binaire) ; en attendant, c'est l'appelant qui la produit. Un
`Index` vide répond `Refusal::UnknownStyle` pour **tous** les styles, ce qui est le bon
refus mais se lit comme un bug si on ne sait pas que rien n'a été chargé — d'où
`Index::is_empty` et `Index::len`.

**La sortie.** Générer `data/minecolonies/structure/index.json` depuis `port-blueprint`
(c'est du ressort de l'outil de conversion, pas de ce module).

---

## 4. La hauteur du monde n'est pas dans `WorldFacts`

**Ce qui manque.** `Limits`, passé en paramètre à `survey_within` / `site_within`, avec les
bornes du moteur en défaut : `-64` inclus et `320` exclu
(`crates/horizon-world/src/level/mod.rs:913-919`).

**D'où ça vient.** `WorldFacts` porte ce qu'un hook a observé — un citoyen, un inventaire, un
bâtiment, des blocs, un métier (`src/ai/facts.rs:801-813`). Aucun champ ne dit à quelle
hauteur le monde s'arrête, et un mod wasm ne peut pas appeler `Level::min_y`.

**Ce que ça coûte.** Deux niveaux de monde différents (un datapack, un monde custom) ne
peuvent pas être décrits sans passer `Limits` à la main. C'est un paramètre, pas une
supposition : la valeur par défaut est celle du moteur, et elle est testée
(`place::tests::a_paste_above_the_build_height_is_refused_not_clamped`).

**La sortie.** Un champ `world_height` dans `WorldFacts`, décidé par `src/ai/`.

---

## 5. `postbox` et `stash` n'ont aucun blueprint, et ne peuvent pas en avoir

**Ce qui manque.** Rien à charger : `Kind::Postbox` et `Kind::Stash` existent
(`src/building/kind.rs`), Java leur donne une place dans la colonie, et le corpus n'a
**aucun** fichier pour eux (mesuré : `find . -name 'postbox[1-5].nbt' -o -name 'stash[1-5].nbt'`
→ 0 résultat sur les 23 styles).

**D'où ça vient.** Java pose ces deux-là sans plan : `PostBox.getMaxBuildingLevel()` rend
`1` en le forçant (`"We set the PostBox to level 1 by force to enable the minimum stock
feature"`, `workerbuildings/PostBox.java:63-67`) et `Stash` rend `0`
(`workerbuildings/Stash.java:54-57`). Un niveau 0 ne nomme aucun fichier.

**Ce que ça coûte.** `Index::resolve(Kind::Postbox, …)` rend `Refusal::UnknownStructure` —
le bon refus, avec le bon nom, mais un appelant qui boucle sur `Kind::ALL` doit savoir que
deux kinds n'ont pas de plan. Une bannière de colonie ou un coffre de ravitaillement ne se
posent pas ici.

**La sortie.** Aucune : c'est fidèle. Ce qui manque serait plutôt un `Kind::has_blueprint`
côté `src/building/`.

---

## 6. Les deux carrières n'ont qu'un niveau, et leurs puits sont ailleurs

**Ce qui manque.** Les niveaux 2 à 5 des carrières. Le corpus n'a que
`infrastructure/mineshafts/simplequarry1.nbt` et `mediumquarry1.nbt` (mesuré : niveaux
`[1]` pour les deux, dans 10 styles sur 23).

**D'où ça vient.** Une carrière ne grandit pas : c'est **la mine** qui descend, par des
blueprints nommés `<carrière>shaft<n>` — `"infrastructure/mineshafts/" + quarry.getSchematicName() + "shaft1.blueprint"`
(`core/entity/ai/workers/production/EntityAIQuarrier.java:162`), et
`new BuildingProgressStage[] {BUILD_SOLID, DECORATE, CLEAR}` pour chaque puits (`:230`).

**Ce que ça coûte.** `Index::levels(style, Kind::SimpleQuarry)` rend `[1]`, ce qui est exact
et complet pour le bâtiment. Les fichiers `simplequarryshaft<n>.nbt` sont présents dans le
corpus mais ne sont nommés par aucune règle de ce module : ils ne sont pas
`<bâtiment><niveau>`, et `naming::candidates` ne les cherche pas.

**La sortie.** Une règle de nommage des puits, du même genre que `Named::alt`, quand le
chantier « carrière » s'ouvrira. Le corpus a de quoi (`find . -name '*shaft*'`).

---

## 7. Java n'a pas de nom pour quatre des styles du corpus

**Ce qui manque.** Une entrée dans `BlueprintMapping.styleMapping` pour `ancientathens`,
`cavern`, `colonial` et `shire`. Ces quatre dossiers existent dans le corpus (23 styles) et
pas dans la table Java (20 clés, `BlueprintMapping.java:26-53`).

**D'où ça vient.** La table a été écrite pour la compatibilité des sauvegardes d'avant 1.14 ;
ces packs sont plus récents que la migration.

**Ce que ça coûte.** Rien côté chargement : `Style::parse` accepte le nom du dossier, donc
ces quatre styles se résolvent normalement, et c'est vérifié par un test
(`naming::tests::the_twenty_three_folders_are_the_twenty_three_measured_ones`). Ce qui
manque est l'inverse : une sauvegarde Java qui contiendrait `ancientathens` **en tant que
clé de style** ne serait pas reconnue par Java, donc ce cas ne se présentera pas.

**La sortie.** Aucune.

---

## 8. `RotationMirror.calcDifferenceTowards` n'est pas porté

**Ce qui manque.** La composition inverse : « quel couple rotation+miroir transforme l'état
A en l'état B » (`RotationMirror.java:180-190`). Le port a `Orientation::rotated` (ajouter un
quart de tour, `:110-119`) mais pas `calcDifferenceTowards`.

**D'où ça vient.** Elle sert à `Blueprint.setRotationMirrorRelative` (`:628-631`) pour
tourner **en place** un blueprint déjà chargé, du client vers le serveur. Un mod wasm ne
tient pas de blueprint chargé — il tient un id et une taille (`Entry`) — donc la question
« de quel delta ai-je besoin » n'a pas d'objet ici.

**Ce que ça coûte.** Un appelant qui voudrait animer une rotation progressive doit calculer
le delta lui-même. Une rotation finale, elle, s'exprime très bien : `Orientation { turn,
mirror }` est l'état, pas la transition.

**La sortie.** À porter le jour où le module tiendra un blueprint chargé.

---

## 9. Deux niveaux de détail que `Kind` ne donne pas

**Ce qui manque.** Le **nom de schéma** quand il diffère du nom de kind. Mesuré : `stable`
(`BuildingStable.getSchematicName` → `ModBuildings.STABLE_ID`, `workerbuildings/BuildingStable.java:85-88`),
`gatehouse` (`BuildingGateHouse.java:49-52`), `mysticalsite` (`BuildingMysticalSite.java:34-37`)
et `DefaultBuildingInstance` (`:44-47`) ont leur propre nom. Dans le corpus, ces quatre
noms valent le nom de kind — donc `Kind::name()` suffit **aujourd'hui**, et c'est ce que
`naming::named` utilise.

**D'où ça vient.** `getSchematicName()` est une méthode de `ISchematicProvider`
rendue par défaut ailleurs (elle n'a pas d'implémentation unique visible :
`grep "public String getSchematicName"` ne trouve que les quatre surcharges et la tile
entity). Le port n'a pas de table kind → nom de schéma, et n'en a pas besoin tant que les
deux coïncident.

**Ce que ça coûte.** Un pack qui renommerait un schéma (un `stable` nommé `horses` par
exemple) sortirait `Refusal::UnknownStructure` en citant le nom du kind, alors que le bon
nom est celui du pack. Le refus reste nommé, donc le diagnostic est possible.

**La sortie.** Un champ `schematic` dans `Kind` (donc dans `src/building/kind.rs`), rempli
par les surcharges de `getSchematicName`. Ce module le consommerait sans changer de forme :
`naming::candidates` prend déjà une chaîne, pas un `Kind`.

---

## 10. Un bâtiment ne se souvient ni de son style ni de son chemin

**Ce qui manque.** Les deux tags par lesquels Java retrouve le fichier d'un bâtiment déjà
posé : `TAG_PACK` (`AbstractSchematicProvider.java:168`) et `TAG_PATH` (`:169`), plus
`TAG_MIRROR` (`:172`) et `TAG_ROTATION` (`NbtTagConstants.java:160`). `crate::building::Building`
porte `type`, `level`, `parent`, `isBuilt`, `deconstructed` et ses modules
(`src/building/instance.rs:50-63`) — pas le style, pas le chemin, pas la rotation.

**D'où ça vient.** Java garde ces quatre tags sur le *block entity* autant que sur le
bâtiment (`TileEntityColonyBuilding.java:378-422`), et c'est ce qui permet à un hut posé avec
un blueprint choisi à la main — `SurvivalHandler.java:242-243`, `building.setStructurePack(packName)`
puis `setBlueprintPath(blueprintPath)` — de retrouver **le même fichier** après un rechargement.

**Ce que ça coûte.** Aujourd'hui le module sait répondre « quel fichier pour `(style, kind,
level)` » ; il ne sait pas répondre « de quel fichier ce hut-là a été posé ». Pour un pack dont
le nom de fichier ne suit pas la règle, ou pour un hut posé depuis un `scans/…` (que
`SurvivalHandler.java:142` accepte), la réponse par défaut est fausse — et rien ne le signale.
`Placing` porte bien `id`, `anchor` et `orientation`, donc **l'information existe au moment de
la pose** ; c'est la persistance qui lui manque.

**La sortie.** Quatre clés sur `Building` (`style`, `path`, `rotation`, `mirrored`) et leur
lecture/écriture dans `src/building/nbt.rs` — un autre module, donc pas ici. Ce module fournit
déjà `Placing` et `turn_from_count` pour relire ces valeurs sans les réinventer.
