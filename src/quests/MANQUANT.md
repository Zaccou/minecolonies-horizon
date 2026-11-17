# Ce qui manque au modele de quete, et ce que je voulais faire

Chaque entree dit : ce que le Java fait, ce que le port garde a la place, et ce qu'il faudrait
pour combler. Rien de tout cela n'est invente dans le code : ce qui n'est pas exprimable est ici.

## 1. Le SNBT : un `nbt` de fichier n'est jamais lu

`DeliveryObjectiveTemplateTemplate.java:105-114` et `ItemRewardTemplate.java:47-58` lisent une
**chaine** et la passent a `TagParser.parseTag(...)`, c'est-a-dire du SNBT (`{display:{Name:'"x"'}}`).
Le port n'a aucun lecteur SNBT, donc `Objective::Delivery.nbt` et `Reward::Item.nbt` gardent le
**texte verbatim** et le transmettent a l'appelant (`Hand::count/drain` recoit `Option<&str>`,
`Outcome::GiveItem` le rend tel quel).

Ce que je voulais faire : comparer le tag a celui de la pile tenue (mode `Exact`). Aujourd'hui
`Exact` ne compare que l'identifiant d'objet, et le tag traverse la frontiere sans etre interprete.
Il faudrait un lecteur SNBT (`horizon` en a un pour les commandes, pas pour les mods) ; ce n'est pas
une regle de quete, c'est un format.

## 2. L'ordre des octets de l'UUID du joueur

`QuestInstance.java:283` ecrit `putUUID(TAG_PLAYER, assignedPlayer)` et `:311` le relit. Vanilla
`CompoundTag.putUUID` ecrit un `IntArrayTag` de **quatre** entiers (les deux moities du `UUID`
coupees en deux). Le port garde `Option<[i32; 4]>` et relit exactement ce qu'il a ecrit, donc une
sauvegarde du port se relit, mais **l'ordre des quatre mots n'a pas ete verifie contre vanilla** :
le depot ne contient pas `CompoundTag.java`, et `mod_api` n'a pas de type `Uuid`.

Ce que je voulais faire : un type `Uuid` dans `mod_api` (ou une confirmation de l'ordre
`msb>>32, msb, lsb>>32, lsb`) pour qu'une sauvegarde ecrite par Java se relise ici et inversement.

## 3. Les effets de bord ne sont construits par personne

`IQuestSideEffect` (`:7-47`) et `WorkerIdleSideEffect` (`:11-70`) existent, et **aucun fichier ne
peut en declarer** :

- le parsing de `side-effects` est dans un bloc `/* ... */` (`QuestJsonListener.java:214-242`) ;
- la ligne qui remplirait la liste est commentee (`QuestInstance.java:122`) ;
- aucun fichier du pack n'a la cle `side-effects` (verifie sur `data/minecolonies/colony/quests/`).

Le port garde les quatre regles qui existent (`WorkerIdle::on_start` ne fait rien,
`apply_to` pose 1 jour, `on_finish`/`on_cancel` remettent a 0, `to_compound` est vide) et **ne les
branche nulle part** : `Template::from_json` ignore `side-effects` comme le Java le fait, en
silence. Ce que je voulais faire : brancher la cle, ce qui demande d'abord de decider qui garde la
liste des effets actifs (le champ n'existe meme pas dans `QuestInstance`).

## 4. `couple` est inatteignable (bug Java, non corrige volontairement)

`RelationshipRewardTemplate.createReward` lit le genre de relation avec
`jsonObject.get(TYPE_KEY)` (`:54`) — le `type` **de la recompense**, donc
`"minecolonies:relationship"` — alors que le fichier livre ecrit `"type": "couple"` dans
`details`. Consequence : le `if (type.equals("couple"))` (`:85`) est toujours faux et une recompense
de relation ne peut que **defaire** un couple existant.

Le port reproduit la lecture (`Reward::from_json`, branche `RELATIONSHIP`), et un test le prouve sur
le texte du fichier livre. Ce que je voulais faire : `details.get(TYPE_KEY)`, une ligne. C'est un
changement de comportement du mod, pas du portage : a decider par Simon, et a proposer en amont tant
que le fork est ouvert.

## 5. `!` dans un `triggerOrder` n'a jamais tourne

`QuestParsingConstants.NOT` (`:26`) existe, le constructeur d'arbre en fait un noeud
(`QuestJsonListener.java:335-338`), et `evaluate` n'a de cas que pour `||` et `&&`
(`:401-452`) : un noeud `!` tombe dans `default`, cherche `triggerMap.get("!")` (une map qui ne
contient que des numeros, `:302-306`) et dereference `null`. `Order::parse` refuse donc `!` par un
refus nomme (`Refusal::OrderNotImplemented`) au lieu de reproduire le crash.

Ce que je voulais faire : implementer `!` comme une negation booleenne. A ne pas faire sans changer
le comportement du mod.

## 6. L'ecran de dialogue et l'interaction de livraison

`DialogueObjectiveTemplateTemplate.startObjective` (`:108-120`) appelle
`colonyQuest.getQuestGiver().openDialogue(quest, index)`, et `CitizenData.openDialogue`
(`CitizenData.java:2032-2046`) choisit entre un `QuestDeliveryInteraction` et le dialogue ordinaire.
Le port s'arrete a la valeur : `Objective::Dialogue.tree`, `Answer::effect()`
(`Nothing`/`Advance`/`Cancel`) et `Objective::delivery_tree(hand)`. Le reste demande un ecran, un
citoyen et un paquet reseau — trois choses que `src/quests` n'a pas.

Ce que je voulais faire : rien de plus ici. La piece manquante est l'ecran (`ui_window_*`), pas le
modele.

## 7. Le gestionnaire d'evenements : cinq maps statiques

`QuestObjectiveEventHandler` (classe entiere) tient cinq `Map` indexees par bloc, type d'entite,
`BuildingEntry`, id de recherche et `UUID` du joueur, et les vide a chaque `onWorldLoad`. Le port
n'en garde que le contrat : `Objective::listener()` dit **quoi** ecouter (`Subject`), `observe` dit
ce qu'un evenement fait au compteur (`Seen`). L'enregistrement, la desinscription et le nettoyage a
la deconnexion sont a la charge de l'appelant, pour la meme raison qu'en 6.

## 8. Les textes de progression

`getProgressText(quest, style)` (`BreakBlockObjectiveTemplate.java:99-110`,
`BuildBuildingObjectiveTemplate.java:225-246`, ...) compose un `Component.translatable` avec le
compteur courant. Le port garde les **cles** (par exemple
`com.minecolonies.coremod.questobjectives.delivery.ready`) et le compteur, jamais la phrase
formatee : la mise en forme appartient au rendu. Deux cles prennent un argument (le nom de l'objet
livre, `DeliveryObjectiveTemplateTemplate.java:77` et `:84`) ; l'objet est dans l'objectif, donc
l'appelant peut le passer.

## 9. Deux registres que le Java consulte au parsing

`BuildBuildingObjectiveTemplate.java:127` resout le batiment via
`IMinecoloniesAPI.getBuildingRegistry().getValue(id)` et `ResearchObjectiveTemplate.java:57` la
recherche via `IGlobalResearchTree.getResearch(id)`. Un id inconnu donne `null` et un `NPE` plus
tard. Le port garde l'`Identifier` et ne resout rien : il n'a pas de registre de batiments ni
d'arbre de recherche sous la main (`src/building`, `src/research` en ont, mais un fichier de quete
se charge avant toute colonie). Ce que je voulais faire : un refus nomme a la resolution ; il faut
une table d'ids valides, qui vit dans le contenu (`data/`), pas ici.

## 10. Ce qui n'est pas un manque mais un choix

- **Le nom d'une quete est une cle de traduction**, pas une phrase : `Component.translatable(name)`
  (`QuestJsonListener.java:182`). `Template::name` garde la chaine.
- **`getQuesterTarget`-style replis** : `Instance::target` rend l'id du donneur la ou Java rend
  `null` ou leve. Le refus nomme existe pour l'index d'objectif (`Refusal::NoSuchObjective`).
- **Deux `Template` pour un fichier** : `crate::manager_09::quests::Template` (occurrences, parents)
  et `crate::quests::Template` (tout). Ils doivent etre remplis **du meme fichier** ; c'est une
  obligation d'assemblage, pas une duplication resolue ici — `manager_09` est hors de mon dossier et
  son type ne porte que ce que son tick lit.
