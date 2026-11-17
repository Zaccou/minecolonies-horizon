# MANQUANT — ce que les buts non-métier veulent faire et que le SDK ne permet pas

Écrit pour `src/entity_ai`. Chaque entrée dit **ce que je voulais faire**, **ce que
le SDK offre réellement** (avec la déclaration), **ce que j'ai fait à la place**, et
**ce que ça coûte**. Rien n'est contourné en silence : le module n'importe pas
`mod_api` du tout, il ne dépend que de `core` et de `crate::`.

Les déclarations citées sont dans
`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/` (SDK épinglé `4dc6455d`) et
dans `.horizon/sdk/api/crates/horizon-mod-api/src/`. Les chemins de crate
(`crates/…`) renvoient au dépôt `arismoko/horizon`.

---

## 1. Déplacer un citoyen — `EntityNavigationUtils`

**Voulu.** Presque chaque but de ce module se termine par un déplacement :
`walkToBuilding` (`EntityAIEatTask.java:402`, `EntityAISleep.java:253`),
`walkToPos` (`:306`, `EntityAISickTask.java:401`), `walkToRandomPos` (`:156`,
`EntityAICitizenWander.java:289`), `walkToRandomPosWithin`
(`EntityAIMournCitizen.java:176`), `walkToPosInBuilding` (`EntityAISleep.java:196`,
`EntityAIMournCitizen.java:223`), `walkAwayFrom`
(`EntityAICitizenAvoidEntity.java:177`, `RangeCombatAI.java:170`),
`walkToUnSafePos` (`AbstractEntityAIGuard.java:429`, `:446`) et
`walkToSafePos` (`:514`, `:535`).

**Ce que le SDK offre.** Rien. Aucune méthode de `host/` ne prend une entité et une
position : recherche `fn walk`, `fn navigate`, `fn move` dans
`crates/horizon-mod-vocab/src/host/` → zéro résultat. Le seul déplacement exposé est
`entity_pos` et `set_entity_pos` **sur une entité MineColonies typée**, pas sur un
citoyen générique.

**Fait à la place.** Chaque règle qui décide d'un déplacement écrit **où** elle veut
aller et **à quelle distance** : `Scene::walked_to` (`guard.rs`, `cavalry.rs`),
`WanderScene::memory.walk_to` (`idle.rs`), `eat::Scene::memory.eat_pos`. La décision
est donc complète et testable, l'exécution est laissée à l'appelant.

**Coût.** Le module ne peut pas savoir si un chemin a échoué : Java le demande
(`!EntityNavigationUtils.walkToBuilding(...)` renvoie `false` et fait **rester** dans
l'état, `EntityAIEatTask.java:402-405`). L'appelant doit rapporter ce booléen, comme
il rapporte déjà `arrived`, `at_bed` et `at_hospital` dans les faits.

---

## 2. `trySleep`, `setPose(SLEEPING)`, `SittingEntity.sitDown`

**Voulu.** `citizen.getCitizenSleepHandler().trySleep(usedBed)`
(`EntityAISleep.java:208`), `setPose(Pose.SLEEPING)` (`:233`),
`SittingEntity.sitDown(eatPos, citizen, TICKS_SECOND * 60)` (`EntityAIEatTask.java:308`),
`SittingEntity.sitDown(worker.blockPosition(), worker, sleepTimer)`
(`AbstractEntityAIGuard.java:284`).

**Ce que le SDK offre.** Aucune notion de pose ni de monture. `host/` expose
`entity_pos`, `entity_health`, `entity_type`, `entity_remove` et
`set_entity_pos`. `Pose` n'apparaît nulle part.

**Fait à la place.** La décision « je m'assois / je me couche » est un booléen de la
scène : [`sleep::Scene::at_bed`], `eat::Memory::eat_pos`,
[`guard::Scene::sat_down`], [`visitor::Scene::sat_down`]. `try_sleep_ok` est un fait
fourni par l'appelant, ce qui rend testables les deux branches (réussi / refusé).

**Coût.** Le cadrage visuel est absent : un citoyen qui dort dans ce port ne
s'allonge pas. `MANQUANT` de `src/ai` §3 dit la même chose pour son propre périmètre.

---

## 3. Infliger des dégâts — `target.hurt(...)`

**Voulu.** `target.hurt(damageSources().source(DamageSourceKeys.GUARD, user), damage)`
(`MeleeCombatAI.java:204`), `:229` ; `arrow.setBaseDamage(...)` +
`level.addFreshEntity(arrow)` (`RangeCombatAI.java:218-221`) ;
`DruidPotionEntity.throwPotionAt(...)` (`DruidCombatAI.java:162`) ;
`trainingPartner.hurt(..., 0.0F)` (`EntityAICombatTraining.java:213`) ;
`sleepingCitizen.hurt(..., 1)` (`AbstractEntityAIGuard.java:255`).

**Ce que le SDK offre.** Aucune écriture sur une entité autre que la position :
ni `hurt`, ni `set_health`, ni `add_entity`, ni `spawn`. Recherche `fn hurt`,
`fn damage`, `fn add_entity` dans `host/` → zéro.

**Fait à la place.** [`combat::try_attack`] décide **quand** la frappe part, à quelle
distance et avec quel délai, et pose `Scene::swung` ; les dégâts eux-mêmes sont
calculés par des fonctions pures et testées :
[`combat::melee_attack_damage`], [`combat::range_attack_distance`],
[`combat::cavalry_attack_damage`], [`combat::druid_effect_duration`]. Le soin du
dormeur (`AbstractEntityAIGuard.java:301-304`) est décidé dans
[`guard::sleep_particles`] et appliqué par l'appelant.

**Coût.** Le module ne peut pas invoquer la mort d'une cible, donc pas
`onTargetDied` (expérience, statistiques du hut, `decreaseSaturationForContinuousAction`).
Ces effets sont **nommés** (`Scene::died`, `Scene::experience`) et laissés à
l'appelant.

---

## 4. Une primitive de ciblage — « objectif d'IA », « ciblage »

**Voulu.** `user.level.getEntitiesOfClass(LivingEntity.class, getSearchArea())`
(`TargetAI.java:142`), `user.getSensing().hasLineOfSight(entity)` (`:162`),
`getNearestPlayer(...)` / `getNearestEntity(...)`
(`EntityAICitizenAvoidEntity.java:149`, `LookAtEntityGoal.java:62-71`),
`level.getEntitiesOfClass(...)` pour l'attaque en zone (`MeleeCombatAI.java:264`),
`getEntitiesOfClass` du ciblage de raid
(`AbstractEntityAIGuard.isAttackableTarget`, `:811-847`).

**Ce que le SDK offre.** `levels::level_block_state(level, pos)` et
`levels::level_is_loaded(level, pos)` en lecture de bloc, et rien pour les entités :
aucune méthode ne liste les entités d'une zone. Recherche `Entities`, `nearest`,
`line_of_sight`, `raycast` dans `host/` → zéro.

**Fait à la place.** Le résultat du balayage est un **fait** :
`CombatFacts::entities`, `CombatFacts::nearest_valid`,
`CombatFacts::threat_target`, `MournFacts::nearest_citizen`,
`LookFacts::nearest`, `FleeFacts::closest`. La **forme** de la boîte de recherche est
portée exactement ([`combat::search_box`] reproduit le losange de `:188-201`, y
compris l'axe qui s'effondre à ±16) et `combat::in_search_box` répond pour une
position donnée.

**Coût.** L'appelant doit faire la requête et remplir le fait. La forme de la boîte
est **fournie** par le port, donc l'appelant ne peut pas se tromper sur la géométrie —
c'est le seul point où ce module est plus strict que la moyenne.

---

## 5. Lire les lignes d'un bloc-entity — la tombe nominative

**Voulu.** `((TileEntityNamedGrave) blockEntity).getTextLines()` puis comparer au nom
du défunt (`EntityAIMournCitizen.java:198-203`).

**Ce que le SDK offre.** Aucune lecture de `BlockEntity` typé : `levels` donne
`Option<BlockState>`, pas un `BlockEntity`. Aucune méthode de texte.

**Fait à la place.** Les tombes sont un fait : `MournFacts::named_graves:
Vec<(Pos, Vec<&'static str>)>`, et la comparaison (prénom + reste du nom, comme
`StringUtils.split` + `replaceFirst`) est portée et testée
([`facts::MournFacts::grave_of_deceased`]).

**Coût.** L'appelant doit lire les textes. Le port gagne au change : la règle de
comparaison est testable sans monde.

---

## 6. Ouvrir une porte, poser un bloc de porte

**Voulu.** `toggleAble.toggleBlock(entity, state, world, pos)`
(`EntityAIInteractToggleAble.java:455`) et
`toggleAble.toggleBlockClosed(...)` (`:360`, `:435`) : écrire la propriété `OPEN`
d'un `FenceGateBlock`, `TrapDoorBlock` ou `DoorBlock`.

**Ce que le SDK offre.** `block_states::set_block_property(pos, key, value) -> bool`
(`block_states.rs:69`) — exactement la primitive nécessaire, **mais** elle prend une
position et une propriété, sans notion d'entité : le module ne l'appelle pas, il
décide seulement **quelle** position ouvrir.

**Fait à la place.** [`avoid::toggle_tick`] rend un [`avoid::Toggled`] :
`kept` (les blocs encore sur le chemin), `open` (celui à ouvrir, ou `None`) et
`close` (ceux que ce but a ouverts et qu'il faut refermer en partant, `:394-401`,
`:427-441`). La propriété à écrire est `OPEN` et le sens dépend de
[`facts::TogglePos::open`] (`TrapToggle.canOpen` est `!OPEN`, `:556-559`).

**Coût.** Presque nul : c'est le seul but de ce module qui ait sa primitive d'écriture
dans le SDK. Il reste à l'appelant de constater la collision
(`ToggleFacts::collided`) qui déclenche le but, faute d'accès à l'état de l'entité.

---

## 7. Un tirage aléatoire

**Voulu.** `getRandom()` dans `shouldSleep` (`AbstractEntityAIGuard.java:278-282`),
`stare` (`EntityAIMournCitizen.java:249`), `searchRestaurant`
(`EntityAIEatTask.java:508`), `decide` (`EntityAICitizenWander.java:266`),
`tryGrowUp` (`EntityAICitizenChild.java:284`), `shouldFlee`,
`isReadyForActivity` (`:134`), `toggle_tick` (`:448`), `decide` du visiteur
(`EntityAIVisitor.java:171`), `attackProtect` (`MeleeCombatAI.java:123`),
`doAoeAttack` (`:262`), le critique (`:345`), `doAttack` du ranger
(`RangeCombatAI.java:167`), le druide (`DruidCombatAI.java:124`).

**Ce que le SDK offre.** Aucune source aléatoire : recherche `fn random`,
`RandomSource` dans `host/` → zéro.

**Fait à la place.** Chaque tirage est un **champ de faits** :
`CitizenFacts::roll`, `CitizenFacts::chance`, `GuardFacts::sleep_roll`,
`GuardFacts::sleep_chance`, `GuardFacts::nap_roll`, `GuardFacts::armour_roll`,
`GuardFacts::shuffle_roll`, `MournFacts::roll`, `MournFacts::stare_roll`,
`MournFacts::coin`, `WanderFacts::roll`, `WanderFacts::wander_roll`,
`WanderFacts::spot_roll`, `WanderFacts::book_roll`, `ChildFacts::ready_roll`,
`ChildFacts::delay_roll`, `ChildFacts::follow_roll`, `ChildFacts::grow_roll`,
`FleeFacts::offset_roll`, `LookFacts::probability_roll`, `ToggleFacts::choice_roll`,
`ToggleFacts::update_roll`, `CombatFacts::direction_roll`,
`CombatFacts::flee_roll`, `CombatFacts::double_arrow_roll`,
`TrainingFacts::roll`, `TrainingFacts::partner_roll`, `TrainingFacts::coin`,
`VisitorFacts::roll`.

**Coût.** L'appelant tire. C'est aussi ce qui rend chaque règle **rejouable** :
sans ça, aucun test de `shouldSleep` ni de `tryGrowUp` ne pourrait exister.

---

## 8. `callForHelp`, `notifyGuardsOfTarget`, `TeleportHelper`

**Voulu.** `citizen.callForHelp(newClosest, MAX_GUARD_CALL_RANGE)`
(`EntityAICitizenAvoidEntity.java:221`),
`CombatUtils.notifyGuardsOfTarget(user, target, PATROL_DEVIATION_RAID_POINT)`
(`MeleeCombatAI.java:423`) et `TeleportHelper.teleportCitizen(...)`
(`AbstractEntityAIGuard.java:442`).

**Ce que le SDK offre.** Rien pour atteindre une **autre** entité : pas de
`get_entities`, pas de `teleport`, pas de `set_entity_pos` sur une entité arbitraire.

**Fait à la place.** La portée de l'appel est calculée
([`combat::notifies_guards`] : raider **et** moins de quarante blocs au carré) et
l'intention est un booléen : `avoid::Scene::called_for_help`,
`combat::Scene::woke_comrades`, `guard::Scene::memory.last_action_pos` pour le
téléport de suivi.

**Coût.** La boucle sur les gardes du hut est à l'appelant. Le seuil, lui, est porté :
`PATROL_DEVIATION_RAID_POINT = 1600` est une distance **au carré**, pas quarante
blocs linéaires.

---

## 9. Les effets de potion et le statut visible

**Voulu.** `worker.addEffect(new MobEffectInstance(MobEffects.REGENERATION, 200))`
(`AbstractEntityAIGuard.java:379`), l'effet de vitesse du ralliement (`:475-479`),
`GLOW_EFFECT` (`:700`), `setVisibleStatus(VisibleCitizenStatus.SLEEP/SICK/SLEEP)`
(`EntityAISleep.java:130`, `EntityAISickTask.java:490`,
`MeleeCombatAI.java:244`) et `PotionUtils.setCustomEffects(...)`
(`DruidCombatAI.java:161`).

**Ce que le SDK offre.** Aucune notion d'effet ni d'icône : `host/` n'expose ni
`add_effect`, ni `visible_status`.

**Fait à la place.** Des booléens et des entiers d'intention :
`guard::Scene::regenerating`, `guard::Scene::speed_effect: Option<i32>` (l'amplitude
calculée), `guard::Scene::glowing`, `SickState`/`SleepState` étant déjà l'info
affichable.

**Coût.** L'icône au-dessus du citoyen est à l'appelant. L'amplitude, elle, est
calculée exactement : `Mth.clamp(Adaptability / 30, 0, 3)` (`:477`).

---

## 10. Les statistiques et l'expérience

**Voulu.** `user.getCitizenExperienceHandler().addExperience(EXP_PER_MOB_DEATH)`
(`MeleeCombatAI.java:436`), `building.getModule(STATS_MODULE).increment(MOB_KILLED +
";" + key)` (`:440`), `StatsUtil.trackStat(building, ARROWS_HIT, 1)`
(`EntityAIArcherTraining.java:224`), `happinessHandler.resetModifier(SLEPTTONIGHT)`
(`EntityAISleep.java:213`), `onWakeUp` (`EntityAISickTask.java:348`).

**Ce que le SDK offre.** Rien pour l'expérience ou les statistiques du hut ;
`crate::jobs::experience` existe dans ce dépôt mais n'est pas relié ici.

**Fait à la place.** `guard::Scene::experience`, `combat::Scene::experience`,
`training::Scene::experience` accumulent la valeur **calculée** ; le reset de
bonheur est le booléen `sleep::Scene::slept_tonight_reset`.

**Coût.** Aucun calcul n'est perdu ; seule l'écriture l'est.

---

## Ce qui n'est **pas** un manque

Trois choses que le brief laissait attendre et qui n'existent pas dans l'arbre Java —
elles ne sont donc pas portées, et ce n'est pas une lacune du SDK :

- `EntityAIChaseCitizen.java` : absent des 93 fichiers de `entity/ai/`.
- `EntityAIPlayerAction.java` : absent.
- `entity/ai/targeting/` : absent ; le ciblage est `combat/TargetAI.java`.

Détail et preuve dans `RAPPORT.md` §1.
