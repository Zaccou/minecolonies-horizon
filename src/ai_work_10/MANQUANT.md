# Ce que ce module ne peut pas écrire, et pourquoi

Deux familles : ce que l'API de mod ne donne pas (aucune n'est inventée dans le code), et
les constats mesurés sur le Java qui changent ce qu'un porteur doit attendre.

## 1. Le monde n'est pas joignable — donc les faits et les effets

Java exécute ces trois buts avec un `ServerLevel`, un `AnimalHerdingModule` et des entités
vivantes en main. Ici il n'y a rien de tout cela, et aucune des lectures suivantes n'est
un appel de l'API de mod : chacune est un champ de `facts.rs`, rempli par l'appelant.

| Ce que Java fait | Ligne | Ce que le port fait |
| --- | --- | --- |
| `WorldUtil.getEntitiesWithinBuilding(world, Animal.class, building, predicate)` | `AbstractEntityAIHerder.java:598` | `PenFacts::animals` |
| `WorldUtil.getEntitiesWithinBuilding(world, ItemEntity.class, …)` | `:613` | `PenFacts::ground_items` |
| `sheep.isSheared()`, `sheep.getColor()` | `EntityAIWorkShepherd.java:101`, `:145` | `PenAnimal::sheared`, `::dye` |
| `cav.hasCavalryRider()`, `hasReservation()`, `isInStable()` | `EntityAIWorkStablemaster.java:352`, `:571`, `:752` | `PenAnimal::has_cavalry_rider`, `::has_reservation`, `::in_stable` |
| `cav.needsHealthTraining()`, `getCombatCooldown()`, `getHealth()`, `getMaxHealth()` | `:363`, `:383`, `:378` | `PenAnimal::needs_health_training`, `::combat_cooldown`, `::health`, `::max_health` |
| `CavalryHorseEntity.createFromVanilla(...) != null` | `:281` | `PenAnimal::convertible` |
| `InventoryUtils.getItemCountInProvider`, `hasBuildingEnoughElseCount` | `:438`, `:440` | `StableFacts::feed_in_inventory`, `::feed_in_hut`, idem cuir |
| `building.getNextStallPosition()` | `BuildingStable.java:124-141` | `StableFacts::stall` |
| `building.getSetting(SHEARING / DYEING / BREEDING)` | `EntityAIWorkShepherd.java:79`, `:175`, `AbstractEntityAIHerder.java:280` | `ShepherdFacts::shearing`, `::dyeing`, `StableFacts::breeding_setting` |
| `building.hasWorkerOpenRequestsOfType(…, StackList.class)` | `EntityAIWorkStablemaster.java:506` | `StableFacts::open_stacklist_request` |
| `checkForToolOrWeapon(lead)` | `:158-170` | `StableFacts::lead_ok` |

**Les écritures sont nommées, pas appliquées.** Java appelle immédiatement
`sheep.setSheared(true)`, `sheep.setColor(...)`, `horse.setLeashedTo(worker, true)`,
`horse.dropLeash(...)`, `feedHorse(horse)`, `prepareForCombat(...)`,
`applyHealthTraining()`, `createFromVanilla(...)`. Le port rend un `HerdAct`
(`facts.rs`) que l'appelant applique : c'est le seul partage qui rende une décision
testable sans monde, et c'est le même choix que `crate::ai::worker::Refusal`.

**Ce qui n'est pas porté du tout, et pourquoi.**

- **Sons et particules** — `SoundEvents.SHEEP_SHEAR` (`EntityAIWorkShepherd.java:149`),
  `GENERIC_EAT` (`AbstractEntityAIHerder.java:559`), `HORSE_EAT` et
  `ParticleTypes.WAX_ON` (`EntityAIWorkStablemaster.java:653`, `:657`). Aucune décision
  n'en dépend ; c'est de la présentation, et lui inventer un type serait du code fictif.
- **Statistiques** — `StatsUtil.trackStat(building, ANIMALS_BUTCHERED / HORSES_TRAINED /
  MOUNTS_READIED / ROUNDUPS_COMPLETED, 1)` : deux sont des `HerdAct`
  (`RoundupDone`, `Readied { ready }`), les autres sont l'affaire de `crate::colony` et
  n'ont pas d'effet sur un état.
- **Expérience et saturation** — `addExperience(XP_PER_ACTION)` (`:103`),
  `decreaseSaturationForContinuousAction()` : la constante est exportée
  (`shepherd::experience_per_action`), l'écriture appartient à `crate::citizens`.
- **L'endommagement de l'outil** — `CitizenItemUtils.damageItemInHand(worker,
  MAIN_HAND, 1)` (`EntityAIWorkShepherd.java:153`, `AbstractEntityAIHerder.java:797`).
  La durabilité vit dans l'inventaire du citoyen, donc dans
  `facts.inventory` : le port dit *quand* l'usure a lieu, il ne la calcule pas.
  Conséquence visible : une cisaille qui casse entre deux ticks se manifeste par
  `shears_slot: None` au tick suivant, ce que `shear_sheep` traite comme
  `TradeRefusal::NoShears` → `PREPARING` — exactement la branche de `:120-123`.

## 2. Une entité n'a pas d'identité manipulable

`worker.getUUID()` et `horse.getUUID()` sont des clés stables que Java compare ; le port
les réduit à un `u32` que l'appelant fournit (`PenAnimal::id`). Deux conséquences :

1. **« ce citoyen tient cette bête en laisse »** n'est pas représentable : `leash_holder`
   est un booléen relatif au citoyen de la scène, ce qui suffit à `:677` mais ne dit rien
   d'un troisième larron.
2. **Un animal qui sort de l'enclos** entre deux ticks n'est pas distinguable d'un
   identifiant faux : les deux rendent `None` de `PenFacts::by_id`, et le port nomme
   `TradeRefusal::MountGone` / `::AnimalGone` au lieu de deviner.

## 3. Un résultat de chemin n'est pas une donnée rejouable

`walkToPos` / `walkToSafePos` rendent « arrivé » ou « je marche », et la réponse dépend du
pathfinding, donc de l'état du monde à ce tick. Le port juge l'arrivée **géométriquement**
(`facts::arrived`, `EntityNavigationUtils.java:169-176`) avec la portée que Java passe :

| Appel Java | Portée | Ligne |
| --- | ---: | --- |
| `walkToPos(worker, pos, 2, true)` (animal) | 2 | `AbstractEntityAIHerder.java:626` |
| `walkToSafePos(pos)` (bête, stalle) | 4 | `AbstractEntityAIBasic.java:830` |
| `walkWithProxy(pos, 1)` (item) | 1 | `AbstractEntityAIHerder.java:579` |

**Écart assumé et mesuré** : `crate::ai::facts::Pos::is_living_at_site` teste
`distSqr < range²` (`EntityUtils.java:301-305`, strict), alors que `walkToPos` teste
`dist <= range` (`EntityNavigationUtils.java:174`, inclusif). À exactement `range` blocs
les deux répondent différemment ; le port suit `walkToPos` et le documente
(`facts::arrived`), au lieu d'hériter de la nuance de l'autre helper par accident.

Conséquence pour l'appelant : une marche qui n'aboutit pas (chemin impossible) ne se
distingue pas d'une marche en cours. `walkToSafePos` ne dit rien de mieux en Java, donc le
port n'invente pas de refus `PathFailed` — mais l'appelant, lui, peut compter les ticks
dans l'état et forcer un `reset` ; `crate::ai::worker::Memory::delay` est là pour ça.

## 4. L'aléatoire n'est pas rejouable

`worker.getRandom()` (`EntityAIWorkRabbitHerder.java:61`, `EntityAIWorkShepherd.java:141`,
`:178`, `:183`, `EntityAIWorkStablemaster.java:238`, `:244`, `:250`) rend une suite que
rien ne rejoue. Le port prend donc sept tirages nommés (`facts::Draws`), un par usage.

**Ce que ça simplifie, et qu'il faut savoir** : Java consomme **un** tirage par condition
atteinte, donc la position d'un tirage dans la suite dépend de l'historique — un
`trainingCooldown` positif fait l'économie du tirage de `:238` (court-circuit de `&&`). Le
port nomme les tirages par usage, donc la décision ne dépend plus de l'ordre de
consommation. C'est ce qui rend le test de `:238` possible ; en contrepartie, le port ne
peut pas dire « le troisième `nextDouble()` de la vie de ce citoyen ».

## 5. Les requêtes et les modules ne sont pas ici

- `checkIfRequestForItemExistOrCreateAsync` (`AbstractEntityAIHerder.java:319`, `:324`,
  `:744`) et `createRequestAsync(new StackList(...))` (`EntityAIWorkStablemaster.java:509`)
  deviennent `HerdAct::RequestedItems { task }` : créer la requête est `crate::requests`.
- `AnimalHerdingModule::isCompatible`, `getBreedingItems`
  (`AbstractEntityAIHerder.java:210`, `:317`) ne sont pas rejoués : le port lit la liste
  d'animaux déjà filtrée et un booléen « il y a de quoi nourrir ». Rejouer le module
  demanderait de porter ses listes d'objets et son matching d'espèces pour aucun état de
  plus.

## 6. La couture avec `crate::ai_work_09`, et ce qu'elle coûte

`AbstractEntityAIHerder` (874 lignes) est **déjà porté**, une fois, par
`crate::ai_work_09::herder`, qui note explicitement que ces trois métiers « se raccordent
à lui et n'écrivent pas une cinquième copie ». Le raccordement ne peut pas être un appel
de fonction, et c'est un manque de la *crate*, pas de l'API de mod :

- `ai_work_09::WorkState` est une énumération **fermée** sur les onze états de ses trois
  métiers ; `SHEPHERD_SHEAR`, `HERDER_TRAIN`, `HERDER_READY_FOR_COMBAT` et
  `HERDER_GATHER_MOUNTS` n'y ont pas de place, et je n'écris pas dans son dossier.
- `ai_work_09::WorkScene` porte un `WorkJob` fermé sur `Planter(…)`, `ChickenHerder(…)`,
  `Cowboy(…)` : une écurie ne peut pas s'y déclarer sans mentir sur ce qu'elle est.
- `ai_work_09::herder::register` est typé `&mut Machine<WorkState, WorkScene>`, donc une
  machine d'un autre type d'état ne peut pas recevoir ses transitions.

La couture retenue est donc **la frontière que Java a lui-même posée** : la table du
parent tourne chez `ai_work_09`, rend une de ses cinq réponses (`HerdChoice`), et
l'appelant la dépose dans `PenFacts::base_answer` — c'est le
`final IAIState result = super.decideWhatToDo()` de `EntityAIWorkShepherd.java:75` et
`EntityAIWorkStablemaster.java:234`. Cette machine-ci possède la surcharge et les cibles
ajoutées, et rien d'autre.

**Ce qu'il reste à faire côté crate, si on veut un seul tick pour un seul citoyen** :
élargir `WorkState`/`WorkScene` (ou les rendre génériques) pour que les quatre états de
cette tranche y entrent, et supprimer alors la couture `base_answer`. Tant que ce n'est
pas fait, un appelant doit faire tourner les deux machines : la table de base jusqu'à
`DECIDE`, cette table-ci sur la surcharge.

## 7. Constats mesurés sur le Java

Ce ne sont pas des manques, ce sont des choses que le Java fait et qu'un porteur doit
savoir pour ne pas « corriger » en silence.

1. **`EntityAIWorkRabbitHerder` n'a aucune cible et aucune surcharge.** Mesuré :
   son constructeur (`:32-35`) ne fait que relayer `super(job)`, et la classe ne contient
   pas de `decideWhatToDo`. Ses 69 lignes sont une frappe et une icône. Un porteur qui
   « ajoute la machine du lapinier » ajoute une copie de la table du parent.
2. **`if (worker.getMainHandItem() != null)` (`EntityAIWorkShepherd.java:125`) est
   toujours vrai** : en 1.20.1 `getMainHandItem()` rend un `ItemStack`, jamais `null`.
   C'est `isEmpty()` qu'il faudrait tester. Le corps de ce `if` s'exécute donc toujours, et
   le port ne garde pas la branche morte.
3. **`getNextStallPosition()` peut rendre `null`** (`BuildingStable.java:128-131`), et
   `walkToSafePos(null)` (`EntityAIWorkStablemaster.java:591`) le déréférence : une écurie
   sans stalle étiquetée fait lever une `NullPointerException` en pleine boucle de tick. Le
   port nomme `TradeRefusal::NoStall` et rend `DECIDE`.
4. **`findNearbyUnstabledHorses` filtre la mauvaise variable** (`:753`) : le terme
   `!(horseToRetrieve instanceof CavalryHorseEntity cav && cav.hasReservation())` teste le
   champ, pas `horse`, la bête du flux. Le filtre de réservation ne mord donc jamais sur un
   candidat. Porté tel quel, et verrouillé par un test
   (`a_reserved_mount_outside_the_stable_is_still_rounded_up`).
5. **`walkToBuilding()` de `readyMount` n'est pas vérifié** (`:442`) et la prise se fait
   dans l'inventaire **de la hutte** (`:444`) : l'objet est retiré même si le citoyen est
   encore à vingt blocs. Porté tel quel.
6. **Les deux comptes à rebours se comptent en passages de `DECIDE`, pas en ticks** :
   `MAX_TRAINING_COOLDOWN = 50` (`:75`) et `DECIDING_DELAY = 80`
   (`AbstractEntityAIHerder.java:143`) donnent 4 000 ticks, `MAX_ROUNDUP_COOLDOWN = 100`
   en donne 8 000. `stablemaster::train_cooldown_ticks` et `::roundup_cooldown_ticks`
   portent ce calcul, avec un test.
7. **`readyMount` compte une action même quand `feedHorse` n'a rien soigné** : `didWork =
   true` est à `:501`, hors du `if (task == FEEDING)`, et la valeur de retour de
   `feedHorse` (`:470`) n'est pas lue. Le port rend `HerdAct::Fed { healed }` pour que
   l'appelant sache les deux choses, et compte l'action comme Java.
8. **La récupération de combat est nulle à l'habileté 0** : `:486` multiplie
   `4.0 * (primary / 20)`, donc `0.0` à zéro, et `prepareForCombat` soustrait zéro. Un
   palefrenier sans habileté peut brûler du cuir sans jamais préparer une monture.
   `stablemaster::leathers_to_ready` rend `None` dans ce cas au lieu d'une boucle infinie.
9. **`isFeedAble` n'est appelée nulle part** (`AbstractEntityAIHerder.java:268-271`,
   mesuré par `grep -rn isFeedAble src/main/java/`) : déjà relevé par
   `ai_work_09/MANQUANT.md`, non repris ici.
10. **`findNearbyUnstabledHorses` annonce 20 blocs dans sa javadoc** (`:726`) et en utilise
    **30** (`:737`). Le port suit le code.
