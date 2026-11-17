# Ce qui manque au module `entity`, et ce que je voulais faire

Chaque entrée dit **ce qui n'est pas possible**, **pourquoi**, et **quel code
aurait été écrit**. Rien de tout cela n'est du code fictif dans les `.rs` : ce
fichier est la liste des trous.

## 1. Trois prémisses de la commande sont fausses

Ce ne sont pas des trous du port mais des faits Java, et le code les dit au lieu
de les contourner.

**a. Le logement n'est pas dans le citoyen.** `CitizenData.serializeNBT`
(`:1316-1421`) n'écrit **aucune** clé de logement. C'est la hutte de résidence qui
écrit l'id dans son propre tableau `residents` (`LivingBuildingModule.java:28-34`,
`:53-66`) et la colonie la rend au citoyen au chargement
(`LivingBuildingModule.java:91` → `CitizenData.setHomeBuilding`). Le type de la
hutte de travail est dans le même cas : le compound du métier ne garde qu'une
**position** (`AbstractJob.java:200-203`). D'où `Citizen::home: Option<[i32; 3]>`,
`Citizen::work_at: Option<[i32; 3]>` et `Citizen::posting.hut: Option<Kind>` :
remplis par `wire_work` / `housed_at`, jamais par le codec, et `Citizen::check`
accepte l'état « métier + position sans type de hutte » comme l'état Java relit.

**b. Ce qu'un citoyen peut porter ne vient pas du métier.** 27 emplacements pour
tout le monde (`InventoryCitizen.java:40`), plus la recherche `citizen_inv_slots`
(`:549-553`, niveaux 9/18/27 dans `DefaultResearchProvider.java:86`). Un
transporteur ne porte pas plus qu'un constructeur. Ce qui **est** dérivé du métier,
c'est l'outil, et `crate::jobs::Job::tools()` possède déjà cette table —
`derived::held_tools` ne fait que pointer dessus.

**c. La vitesse ne vient du métier que pour trois métiers sur cinquante, et
seulement sur un passage de niveau.** Forestier : `(secondaire / 2) * 0.003`
(`JobLumberjack.java:90-91`) ; transporteur : primaire `* 0.003`
(`JobDeliveryman.java:95-96`) ; guérisseur : primaire `* 0.003`
(`JobHealer.java:68-69`). Un fermier niveau 99 marche à la vitesse de base (0,3),
et `getSpeed()` plafonne à 0,5 (`EntityCitizen.java:149`, `:1351-1354`).

En prime, **le transporteur de l'entrepôt** : `JobDeliveryman.onLevelUp` lit sa
compétence via `BuildingModules.COURIER_WORK` (la hutte de transporteur), module
absent de l'entrepôt (`crate::jobs::EMPLOYMENTS` ligne `warehouse_couriers`).
`derived::walk_speed` refuse avec `entity.refusal.no_skill` au lieu de reproduire
l'appel à un module nul.

## 2. Deux codecs qui n'existent pas, donc deux champs qui ne survivent pas

`Citizen::happiness` et `Citizen::needs.sick` sont dans le record et **ne sont pas
sérialisés** :

- les modificateurs de bonheur : `citizens::happiness::MANQUANT.md` §4 (rien
  n'écrit ni ne relit `AbstractHappinessModifier.serializeNBT`) ;
- la maladie : `CitizenDiseaseHandler` a son propre `write`/`read`
  (`:186-235`), et `citizens::happiness::disease::Sickness` n'a pas de codec.

Ce que je voulais écrire : `Happiness::to_compound` / `from_compound` (une liste
`{source, days, period}` par modificateur, `Source::from_name` existe déjà) et
`Sickness::to_compound` / `from_compound`. Les deux sont dans
`citizens/happiness/`, hors de mon périmètre d'écriture.

Conséquence mesurable : un citoyen rechargé repart avec les dix modificateurs
neufs et guéri. Les tests de `data` ne prétendent pas le contraire : ils ne
comparent que les champs qui font l'aller-retour.

## 3. La santé n'est pas dans le citoyen non plus

`CitizenData.MAX_HEALTH` existe (`:83`) mais `health` n'est pas un champ : c'est
l'attribut de l'entité (`AbstractEntityCitizen.java:159`), posé par
`initEntityValues` (`CitizenData.java:568`). `Citizen::needs.health` est donc
**runtime** : `to_compound` ne l'écrit pas, et un rechargement repart au maximum.
Ce que je voulais écrire : rien de plus dans ce module — la santé appartient à
l'entité, et la faire voyager dans le compound du citoyen créerait deux vérités.

## 4. Rien n'appelle ces règles sur une horloge

`entity_tick_pre`
(`crates/horizon-mod-vocab/src/hooks/entity_tick_pre.rs:19`) est un hook **global** :
il reçoit une entité, pas « le citoyen X à son tour ». Donc :

- `death::flee_is_over` et `death::pace` sont des règles pures que personne
  n'appelle par tick ;
- `data::Citizen::check` n'est appelé par aucun hook au chargement ;
- le passage de `Fate::Wait` à `Fate::Persist` pour une colonie en cours de
  chargement n'a pas de deuxième essai.

Ce que je voulais écrire : dans un tick par entité, `match death::on_join(...)`
puis `Fate::Discard(why) => host.remove_entity(...)`. La décision est testée ;
c'est la couture qui manque.

## 5. Ce que le métier écrit et qui n'est pas modélisé

- `AbstractJob.actionsDone` et ses jetons de requêtes asynchrones
  (`AbstractJob.java:188-206`) : l'état du métier, pas du citoyen. Le port ne
  l'écrit pas ; Java accepte son absence (`:219-222`) et repart à zéro — un
  transporteur rechargé perd ses livraisons en cours. `crate::jobs` ne modélise
  pas encore l'état d'un métier, seulement sa fiche.
- la famille (`parentA`, `parentB`, `siblings`, `children`, `partner`,
  `:1369-1385`) et les quatre listes de quêtes (`:1394-1415`) : douze clés, une
  tranche à elles seules. Le code voulu : `entity::family::Kin` et
  `entity::quests::Log`, avec `KEY_PARENT_A`/`B` (`NbtTagConstants.java:107-108`),
  `KEY_PARTNER` (`:111`), `KEY_CHILDREN` (`:110`), `KEY_SIBLINGS` (`:109`).
- les options de dialogue (`TAG_CHAT_OPTIONS`, `:1360-1366`) : état d'interface,
  à raccorder aux handlers d'interaction, pas à la donnée du citoyen.
- l'inventaire : `crate::inventory` le modélise déjà ; ici il n'y a que le
  **nombre** d'emplacements (`derived::carry_slots`).
- `raiderManager.onLostCitizen` (`EntityCitizen.java:1552`) : la comptabilité de
  raid n'existe pas encore dans le port.

## 6. Les deux refus que ce module ajoute à Java, assumés

Java perd des travailleurs en silence ; le port refuse et nomme :

- **résidents en trop** : `assignCitizen` répond `false` quand le module est plein
  (`WorkerBuildingModule.java:101`) et l'entrée est jetée. Ici
  `entity.refusal.overfull` avec `{capacity, found}` ;
- **id de résident ≤ 0** : `getCivilian(0)` répond `null` (`:129`) et l'entrée
  disparaît. Ici `entity.refusal.bad_citizen`.

Pareil pour `EntityCitizen.die` sans colonie : Java n'écrit **rien** (le bloc
entier est sous `if (colony != null && data != null)`, `:1550`) et laisse un
citoyen dans le registre pour une entité qui n'existe plus. Ici
`entity.refusal.no_colony` avec l'id.

## 7. Ce que le module ne fait pas du tout, par choix

Pas de rendu, pas d'IA, pas de pathfinding, pas de but : `src/citizen.rs` reste le
seul endroit qui touche une entité, et ce module n'appelle aucun hôte. C'est ce qui
permet de tester les 48 cas sans jeu, sans wasm et sans ressources préparées.
