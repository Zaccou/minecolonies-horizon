# MANQUANT — ce que `src/ai_work_06` voulait faire et qui n'était pas possible

Écrit pour les deux buts d'éducation : le **chercheur** (`EntityAIWorkResearcher`) et le
**professeur** (`EntityAIWorkTeacher`). Chaque entrée dit **ce que je voulais faire**, **ce
qui existe réellement** (avec la déclaration lue), **ce que j'ai fait à la place**, et
**ce que ça coûte**. Rien ici n'est contourné en silence : le module ne compile que sur
`core`, `crate::` et le SDK, et il n'importe pas `mod_api`.

Les déclarations citées sont dans
`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/` (SDK épinglé `4dc6455d`, celui de
`Cargo.toml`).

---

## 1. `Machine<S, F>` est monomorphe : le délai de base ne peut pas vivre dans la machine

**Voulu.** Que la machine d'éducation contienne **tout** le temps d'un métier : la cible
de métier, l'attente (`setDelay`) et le vidage d'inventaire, pour qu'un appelant n'ait
qu'un seul objet à faire tourner.

**Ce qui existe.** `crate::ai::state::Machine<S, F>` est génériqué sur **un** état et **un**
type de scène (`src/ai/state.rs:516-540`). `crate::ai::worker::register_base` enregistre
les quatorze cibles de `AbstractEntityAIBasic.java:204-259` sur
`Machine<WorkerState, Scene>` — un autre couple `(S, F)` que `(EduState, EduScene)`.

**Fait à la place.** Deux conséquences, assumées et mesurées :

1. `register_handover` (`src/ai_work_06/mod.rs`) enregistre **cinq** transitions de
   raccord, une par état de base que ces métiers traversent, chacune citant la ligne de
   base qui la possède. Ce n'est pas une deuxième `register_base` : c'est la façon de
   *revenir*.
2. La machine démarre dans `IDLE`, comme `GatherAi::new` (`src/ai_gather/mod.rs:451-458`),
   et non dans `INIT` : `INIT` et `initSafetyChecks` (`AbstractEntityAIBasic.java:446-465`)
   appartiennent à `crate::ai`.

**Coût — c'est le seul coût réel du module, et il est de temps, pas de justesse.**
`setDelay(STUDY_DELAY)` (`EntityAIWorkResearcher.java:103`) **est enregistré** mais pas
**dépensé** : `waitingForSomething` (`AbstractEntityAIBasic.java:496-517`) est une cible
de la table de base. Un appelant qui ne fait tourner que la machine d'éducation voit un
chercheur enchaîner une passe par 4 ticks machine au lieu d'une par minute. Un appelant qui
fait tourner les deux tables — `JobAi` puis `EduAi` sur le **même** `WorldFacts` — retrouve
le rythme exact de Java, parce que `memory.base.delay` est le champ que
`waitingForSomething` décrémente. Le test
`the_researcher_sets_a_one_minute_delay_the_base_table_owns` épingle la valeur (1200) que
l'appelant doit respecter.

---

## 2. La moitié « métier » de `WorldFacts` est un enum fermé

**Voulu.** Poser les faits du chercheur et du professeur **dans** `WorldFacts`, à côté de
ceux du composter et du builder.

**Ce qui existe.** `crate::ai::facts::JobFacts` a exactement trois variantes — `Composter`,
`Builder`, `Unported` (`src/ai/facts.rs:780-797`) — et ce fichier est hors de la frontière
d'écriture de cet agent (règle 1).

**Fait à la place.** `EduScene` **enveloppe** `WorldFacts` au lieu de l'étendre ; `facts`
est la **même valeur** que celle que `crate::ai` consomme, rien n'est recopié. C'est
exactement la solution que `src/ai_gather` a retenue, pour la même raison.

**Coût.** Un troisième type de scène (`GatherScene`, `EduScene`, et `Scene`). Le jour où
l'on voudra une scène unique, il faudra cinq variantes de plus dans `JobFacts` — décision
d'orchestrateur, pas la mienne.

---

## 3. Aucune API de compétence ou d'expérience de citoyen

**Voulu.** Appliquer les deux gains d'expérience en place, comme Java :
`worker.getCitizenExperienceHandler().addExperience(...)`
(`EntityAIWorkResearcher.java:100`, `EntityAIWorkTeacher.java:169`) et
`pupil.getCitizenData().getCitizenSkillHandler().addXpToSkill(Skill.Intelligence, xp, …)`
(`EntityAIWorkTeacher.java:188`).

**Ce qui existe.** Côté SDK, rien pour un citoyen : `entities.rs` expose
`set_experience_orb` (orbes au sol), et `levels.rs:1478` expose `level_award_experience`
(l'XP **vanilla** d'un joueur). Les six compétences de MineColonies
(`Knowledge`, `Mana`, `Intelligence` — `api/entity/citizen/Skill.java`) sont une donnée du
mod, pas une donnée du jeu.

**Fait à la place.** Les gains sont des **ordres** : `Effect::Studied { xp, research }`,
`Effect::Taught { pupil, pupil_xp, teacher_xp }`. L'arithmétique est portée et testée
(`teach_xp`, `XP_PER_STUDYPOS`, `TEACHER_XP`) ; l'écriture dans le citoyen revient à
`crate::citizens`.

**Coût.** Un appelant doit lire l'ordre, pas seulement l'état. Aucun risque de divergence :
c'est le même calcul, avec ses trois facteurs et ses coupures, et les tests le tiennent.

---

## 4. L'inventaire d'un citoyen n'est pas atteignable par `ItemPlace`

**Voulu.** Transférer une feuille de papier du professeur vers l'élève en une transaction,
comme `transferXOfFirstSlotInItemHandlerWithIntoNextFreeSlotInItemHandler`
(`EntityAIWorkTeacher.java:155-159`).

**Ce qui existe.** `ItemPlace` (`horizon-mod-vocab/src/block/item_place.rs:23-110`) a bien
un `EntitySlot { entity, slot }` lisible en écriture, mais sa documentation dit qu'il passe
par `Capabilities.Item.ENTITY` et nomme les cas couverts : **wagonnet de coffre, bateau de
coffre, âne/mule/lama bâté**. Un citoyen n'enregistre pas ce capability dans ce port : son
inventaire est `InventoryCitizen`, une donnée du mod (`src/inventory/`).

**Fait à la place.** `Effect::GivePaper { pupil, count }` : la décision dit *qui*, *quoi* et
*combien*, jamais *comment*. Le déplacement réel appartient à `crate::inventory`.

**Coût.** Le port ne peut pas prouver la transaction ; il prouve la décision de la faire, y
compris sa condition (`paper_in_inventory > 0 && !pupil.carries_paper`,
`EntityAIWorkTeacher.java:151-160`), qui est la partie où Java se trompe le plus facilement.

---

## 5. `SittingEntity` n'existe pas dans Horizon

**Voulu.** Asseoir le professeur : `SittingEntity.sitDown(worker.blockPosition(), worker,
maxSittingTicks * 20)` (`EntityAIWorkTeacher.java:136`), et `worker.stopRiding()` pour le
relever (`:145-149`).

**Ce qui existe.** `entities.rs:55` `summon(dimension, kind, pos, nbt)` — pour un type
**enregistré** ; `entities.rs:807` `entity_start_riding(entity, vehicle, force)`, et
`entities.rs:330` `entity_vehicle(entity)`. Rien ne donne l'entité `minecolonies:sitting`
de `core/entity/other/SittingEntity.java:31`, qui est une entité du mod.

**Fait à la place.** `Effect::Sit { at, ticks }`, avec `ticks = maxSittingTicks * 20` — donc
la multiplication est portée et testée (200 ticks pour une leçon de 10 s), et la durée est
la même des deux côtés. `teacher_seated` (`worker.vehicle != null`, `:130`, `:145`) est un
**fait** que l'appelant rapporte, ce qui permet à la décision de re-asseoir le professeur
quand le siège a disparu — la branche que Java a et qu'un simple compteur perdrait.

**Coût.** `Effect::Sit` restera un ordre tant qu'Horizon n'a pas d'entité de siège. Le jour
où il en aura une, la seule chose à changer est le corps de l'appelant, pas la décision.

---

## 6. Le système de requêtes et la recherche ne sont pas des API du SDK

**Voulu.** Créer la requête de papier (`worker.getCitizenData().createRequestAsync(new
Stack(new ItemStack(Items.PAPER, PAPER_TO_REQUEST)))`,
`EntityAIWorkTeacher.java:214`) et avancer une recherche
(`research(...)` puis `building.onSuccess(research)`,
`EntityAIWorkResearcher.java:88-94`), depuis le SDK.

**Ce qui existe.** Rien de tel dans `host/` : la recherche et les requêtes sont des données
du mod. Dans ce crate, les deux existent déjà :
`crate::research::Progress::advance(tree, id, effects)` (`src/research/progress.rs:291-318`,
qui répond exactement le booléen dont `onSuccess` dépend) et `crate::requests`.

**Fait à la place.** `Effect::AdvanceResearch { id, concludes }` et
`Effect::RequestPaper { count }`. Le champ `concludes` est
`progress + 1 >= ticks_total` (`LocalResearch.java:56-75`, `InProgress::next_concludes`), et
un test le tient ; l'appelant n'a plus qu'à passer l'`id` à `Progress::advance`.

**Coût.** Aucun : c'est le découpage que `src/research/mod.rs:60-64` annonce déjà (« Java's
researcher picks any in-progress research at random … `progress::Progress::advance` takes
the id, because a port with no clock and no RNG cannot pretend to choose »).

---

## 7. Aucun tirage aléatoire — par conception, pas par manque

**Voulu (et refusé).** Tirer dans la machine, comme `worker.getRandom()`.

Trois tirages décident du comportement de ces métiers : la recherche avancée
(`EntityAIWorkResearcher.java:86`), l'élève choisi (`EntityAIWorkTeacher.java:110`) et la
durée de la leçon (`:134`). Ils sont **des faits** de la scène (`draw`, `pupil_draw`,
`sit_roll`), fournis par l'appelant. Une décision qui dépend d'un tirage ne se rejoue pas,
donc ne se teste pas — et c'est exactement ce que le test de scénario demande : la même
suite d'états reproductible.

**Coût.** L'appelant doit tirer. C'est déjà le cas pour tout le port (voir
`WorkerFacts::coin`, `GatherFacts::draw_100`).

---

## 8. Ce que je n'ai **pas** eu besoin de demander

Pour être précis sur l'autre bord : ces lectures sont des **faits** que l'appelant remplit,
et aucune ne demande d'API nouvelle — `getRandomBookShelf`
(`BuildingUniversity.java:115-128`), `hasBuildingEnoughElseCount`
(`InventoryUtils.java:782-805`), `WorldUtil.getEntitiesWithinBuilding` (`:321-337`),
`walkToBuilding` / `walkToWorkPos` (`AbstractEntityAIBasic.java:757-808`) et
`isLivingAtSite` (`api/util/EntityUtils.java:301-305`, portée par
`ai::facts::Pos::is_living_at_site`). Un fait coûte une ligne chez l'appelant, pas une API
inventée.
