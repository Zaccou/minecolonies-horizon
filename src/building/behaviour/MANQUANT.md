# MANQUANT — ce que `src/building/behaviour` demande et que ce module ne peut pas fournir

Écrit par l'agent qui a porté la pose, l'amélioration, la casse et la table des modules.
Tout ce qui suit est un manque **mesuré**, pas une supposition : chaque entrée dit ce qui échoue
aujourd'hui, où, et le changement exact qui le débloque.

## 1. La déclaration du module — **bloquant**

`src/building/mod.rs` déclare `cost`, `instance`, `kind`, `nbt` et `requirements`, mais pas
`behaviour` :

```rust
mod cost;
mod instance;
mod kind;
mod nbt;
mod requirements;
// manque : pub mod behaviour;
```

Tant que cette ligne n'existe pas, `src/building/behaviour/` n'est **pas compilé** : les cinq
fichiers et leurs 52 tests sont invisibles pour `cargo test`, et une erreur dedans ne serait pas
signalée. L'agent de ce module n'a pas le droit d'écrire dans `src/building/mod.rs` (règle 1), donc
la ligne est à ajouter par l'orchestrateur.

Le module n'ajoute **aucune** dépendance et n'appelle **aucune** API du SDK : `mod_api` n'est pas
même importé. Il ne se sert que de `crate::building` (`Standing`, `Building`, `Footprint`, `Kind`,
`Site`, `Rules`, `Refusal`), de `crate::colony` (`Colony`, `WorkType`), de `crate::founding`
(`plan`, `Ground`, `Rules`, `Refusal`, `Placed`) et de `crate::jobs` (`Job`, `employ`, `EMPLOYMENTS`).
Aucun type n'est inventé, et les deux seules choses que `src/building` ne savait pas faire et qui
servent ici — `Standing::check_upgrade` et `Building::parent` — existaient déjà.

## 2. Ce que la casse ne peut pas rendre : l'inventaire plein

`Refusal::InventoryFull` existe et n'est jamais levée. Java refuse le ramassage quand le joueur n'a
pas de place (`AbstractBuilding.java:855-859`, `WARNING_BUILDING_PICKUP_PLAYER_INVENTORY_FULL`) ;
le port a un `crate::inventory::Inventory`, mais pas le lien entre une `ItemName` et une place
libre chez un joueur. `casse::pickup` refuse donc **seulement** la hutte imbriquée et laisse le
plein à l'appelant.

## 3. Le prestige n'est pas calculable

`Recalculated::Prestige` nomme l'étape, rien ne la fait : Java lit une note de coût sur le
blueprint (`SchemAnalyzerUtil.analyzeSchematic(blueprint).costScore`,
`AbstractBuilding.java:975-990`), c'est-à-dire une analyse de structure que `port-blueprint` sait
lire mais que `src/building` n'expose pas encore. Tant que ce n'est pas là, `Step` ne porte pas de
champ prestige.

## 4. Le recensement des citoyens compte une tête par bâtiment

Java somme `getAllAssignedCitizen().size()` module par module (`CitizenManager.java:133-140`) ;
`building::Building` ne tient **qu'un** travailleur pour tout le bâtiment (`instance.rs`, *one
worker per building*). Conséquence mesurée : une école de niveau 3 avec un professeur et six
élèves compte 1 tête ici et 7 là-bas, et une caserne de cinq modules de gardes compte 1 au lieu de
5 × niveau. Ce n'est pas un bug du recensement, c'est la limite du modèle en dessous.

## 5. Le mode d'embauche n'existe pas sur un bâtiment

Le recensement Java traite une résidence `HiringMode.LOCKED` différemment des autres
(`CitizenManager.java:145-148` : elle compte ses habitants *assignés*, pas sa capacité).
`network::buildings::HiringMode` existe, mais `building::Building` ne porte pas ce champ, donc
`amelioration::census` compte toujours la capacité d'un `living`.

## 6. Pas d'arbre de recherche

`amelioration::Research` est une valeur que l'appelant fournit : `has_effect` (l'arbre connaît-il
cette hutte) et `strength` (jusqu'à quel niveau). Java les lit dans la colonie
(`AbstractBuilding.java:786-798`). Le port n'a pas de `ResearchManager`, donc
`Research::none()` est le seul défaut sûr — et c'est exactement le comportement de Java pour une
hutte que l'arbre ne cite pas.

## 7. Pas de retrait sur `Standing`

`Standing` sait `insert` et `at`, pas `remove` (`src/building/requirements.rs`). La casse planifie
donc sur un `&Standing` et applique sur un `&mut Colony` : `casse::demolish` retire le record et
les enfants, `Standing` ne bouge pas. Un `Standing::remove(at)` rendrait `demolish` inutile et
permettrait de recalculer le recensement après coup.

## 8. La boîte d'un bâtiment n'est pas sur le bâtiment

`casse::plan` prend `plot: Footprint` en argument : `building::Building` ne garde pas les coins de
son schéma (Java les recalcule depuis le block entity,
`AbstractSchematicProvider.java:266-286`). L'appelant les tient de `founding::Placed::footprint`
ou du blueprint.

## 9. Le chantier (construction tape) est du contenu absent

`casse::Tape` calcule l'anneau à balayer (`ConstructionTapeHelper.java:66-110`) mais
`minecolonies:blockconstructiontape` fait partie des 143 items que `MANQUANTS.md` (racine) liste
comme absents du port. Poser et retirer la bande est donc un appel au monde, pas une écriture de ce
module.

## 10. Une incohérence de nom relevée en passant

`colony::Building` stocke le type dans un `String` et `src/colony/colonie.rs` le remplit avec un
nom de **bloc** (`"minecolonies:blockhutbuilder"`) dans ses propres tests, alors que Java y met le
nom d'**entrée** (`ModBuildings.BUILDER_ID`, donc `minecolonies:builder`) et que
`building::Building::to_compound` écrit bien le chemin d'entrée. `founding::hut_of` et
`behaviour::pose` attendent le chemin d'entrée : un enregistrement au nom de bloc est refusé en
`UnknownKind`. Ce n'est pas corrigé ici (le fichier n'est pas à moi) ; c'est signalé parce qu'un
test de colonie qui passerait par `hut_of` échouerait pour cette raison.
