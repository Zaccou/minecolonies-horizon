# Ce que `src/research/` ne peut pas faire, et ce qu'il faudrait

L'arbre de recherche de l'université est porté : **327 fichiers JSON** lus, **4**
branches, **106** catégories d'effet et **217** recherches dans le tableau, les
coûts, les exigences, les refus nommés et la persistance des enregistrements
(`src/research/`, 8 fichiers). Ce fichier dit **ce qui manque autour**, avec la
ligne de Java ou du SDK qui le dit. Chaque point est un choix, pas un oubli.

---

## 1. Aucun point d'entrée : `src/lib.rs` n'est pas à cet agent

Les règles sont pures et rendent un `Result<_, Refusal>`. Personne ne les appelle
encore : l'université n'a ni bloc, ni menu, ni `[[native.mixin]]` dans cette
tranche, et `src/lib.rs`, `src/menu.rs`, `src/hut.rs` appartiennent à d'autres
agents. Ce qui est prêt à être branché :

| Règle | Ce qu'il faudrait | Contrat |
| --- | --- | --- |
| ouvrir l'arbre | un menu d'université | `Registrar::menu` (`src/menu.rs`) |
| lister les recherches affichables | l'écran | `gate::visible`, `gate::can_display` |
| cliquer « commencer » | une commande ou un bouton | `Progress::begin` |
| le temps qui passe | un tick d'entité | `Progress::advance`, appelé par le métier de chercheur |

Le dernier point est le seul qui demande une décision : **quel** chercheur
avance **quelle** recherche. Java tire au hasard parmi celles en cours
(`core/entity/ai/workers/education/EntityAIWorkResearcher.java:86`) ; un mod wasm
n'a pas de RNG à bord (`mod_api` n'en expose pas), donc `advance` prend l'id et
le choix reste à l'appelant. C'est une différence assumée, pas un trou : une
recherche qui avance parce qu'un agent a décidé est testable, une recherche qui
avance au hasard ne l'est pas.

## 2. La liste des bâtiments d'une colonie n'est pas lisible

`Requirement::met` a besoin des bâtiments qui existent : la lecture `building`
**somme** les niveaux de tous ceux du même type
(`api/colony/managers/interfaces/ICommonRegisteredStructureManager.java:207-231`),
la lecture `single-building` prend le maximum. `crate::colony::Colony` garde ses
bâtiments dans une table indexée par position et n'expose que
`building_at(at)` (`src/colony/colonie.rs:216-219`) : il n'y a pas d'itérateur,
donc pas moyen de calculer une somme.

D'où `cost::Buildings`, la vue que l'appelant remplit. La bonne correction est
une ligne dans `colony` (`pub fn buildings(&self) -> impl Iterator<Item = &Building>`),
dans un module que cet agent n'écrit pas.

## 3. `nbt` est la troisième copie des mêmes quatre fonctions

`src/colony/nbt.rs` et `src/building/nbt.rs` en portent déjà une, chacune
`mod nbt;` donc invisible d'ici (`src/building/nbt.rs:9-13` le dit pour la
sienne). `src/research/nbt.rs` est la troisième : `int`, `text`, `compounds`,
`compound_list`. Rien de neuf, rien de faux — mais trois copies d'un lecteur NBT,
c'est deux de trop. Il suffit de `pub(crate) mod nbt;` dans `lib.rs`, ou d'un
`src/nbt.rs` commun, pour en supprimer deux.

## 4. Le filtre d'inventaire côté joueur n'est pas reproductible

`IGlobalResearch.isPlayerResearchMatch` refuse une pile **enchantée ou
renommée** (`api/research/IGlobalResearch.java:205-217`) : un livre
`Sharpness I` ne paie pas une recherche, un livre normal si. Côté bâtiment, le
même test ne filtre rien (`:225-233`).

`inventory::Stack` porte un nom et un compte, **pas de composants**
(`src/inventory/stack.rs:70-78`) : le filtre du bâtiment est exact, celui du
joueur ne peut pas l'être. Le port compte donc les deux de la même façon, et un
livre enchanté dans l'inventaire du joueur paie — c'est plus permissif que Java,
dans un sens qui ne dupe personne.

## 5. Annuler une recherche ne coûte rien

`attemptResetResearch` fait payer `server.researchResetCost`
(`core/colony/managers/ResearchManager.java` → `GlobalResearchTree.getResearchResetCosts`,
`:302-344`) : une liste d'ids d'items avec NBT, lue dans la configuration
serveur. Un mod wasm n'a pas de configuration serveur (`config.rs` de l'API
expose les options du mod, pas celles d'un autre), donc `Progress::undo` ne
facture pas. Le refus nommé existe (`Immutable`, `AutoStarts`, `ChildInTheWay`) ;
seul le prix manque, et il manque **en moins**, ce qui est le bon sens de
l'erreur : personne ne perd d'items à cause d'un port.

## 6. Cinq tags seulement, et c'est dit

Sept coûts de la donnée sont des coûts de **tag** (`#minecraft:iron_ores`,
`:gold_ores`, `:diamond_ores`, `:planks`, `minecolonies:concrete`) et les cinq
sont portés avec leurs membres (`src/research/cost.rs`, `TAGS`). Un data pack
qui en ajoute un reçoit `Refusal::TagUnknown`, pas un compte de zéro : zéro
laisserait croire que la recherche est payable et l'item absent.

La résolution passe par les fichiers de tags du jeu
(`data/minecraft/tags/item/*.json`, extraits du jar client) et par
`data/minecolonies/tags/items/concrete.json` du datagen. Un vrai lecteur de tags
demanderait l'API des tags de données (`horizon-mod-vocab/src/host/tags.rs`), qui
parle des tags **du monde chargé** : elle n'existe qu'à l'exécution, pas dans un
test d'hôte.

## 7. Pas d'écran

`WindowResearchTree` fait 1 100 lignes : vue à glisser-zoomer, en-têtes de
niveau, flèches entre parent et enfant, position des icônes
(`core/client/gui/WindowResearchTree.java`). Le port garde les **décisions** de
cet écran — ce qui est visible (`:285`), ce qui ne peut pas être annulé
(`:228-243`) — et rien du dessin : `icon`, `subtitle`, `sortOrder` ne servent
qu'à l'affichage, et seuls les deux derniers sont conservés parce que le tri est
une règle de lecture et le sous-titre une donnée du fichier.

## 8. Pas de paquet, parce qu'il n'y a qu'un côté

`GlobalResearchTreeMessage` (72 lignes) envoie l'arbre entier au client à la
connexion (`core/research/GlobalResearchTreeMessage.java`). Un mod wasm tourne
d'un côté, et l'arbre est une constante de compilation : il n'y a rien à
envoyer, et le désérialiseur de `GlobalResearchFactory` (`:216-254`) n'a pas de
correspondant.

## 9. Deux chemins de code que la donnée n'emprunte pas

- `Cost::Any` (`minecolonies:item_list`, `ListItemCost` + `getAsJsonArray(json, "items")`) :
  **0** des 247 entrées de coût du jeu de données. Il est porté, lu et testé,
  parce que le type existe (`ModResearchCosts.java:18`) et qu'un pack peut s'en
  servir.
- `Requirement::Research` (`minecolonies:research`, `ResearchResearchRequirement`) :
  **0** des 186 exigences. Même raison. Son test dit explicitement qu'aucune
  ligne livrée ne le déclare (`tree::tests`), pour que le jour où une ligne en
  ajoute une, ce soit le test qui casse et pas le joueur.
