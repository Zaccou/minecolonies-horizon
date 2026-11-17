# MANQUANT — ce que ces deux écrans demandent et que le port n'a pas

Ce module n'appelle **aucune** API de l'hôte. Il n'utilise que `std`, `crate::ui`
(`Field`, `Value`, `Scope`, `Source`, `Action`, `Effect`), `crate::base::Dimension`,
`crate::network::colony_settings::TriggerConnectionEvent`, et — pour ne pas le
dupliquer — `crate::ui_window_13::quest_log` / `::refusal`. Rien n'a été ajouté à
`crates/` : chaque manque est nommé ci-dessous, avec le contrat que le port prend à sa
place.

## 1. Un widget de liste, et un widget de liste à deux niveaux

**Ce que je voulais faire** : décrire les **six** listes de ces deux écrans — les trois
`quests` du journal (`windowquestlog.xml:21`, `:34`, `:46`) et les trois de la page
d'alliance (`layoutalliance.xml:11`, `:23`, `:39`) — dont deux paires partagent le même
id de bouton dans deux listes.

**Ce qui existe** : `mod_api::client::widgets::Widgets`
(`crates/horizon-mod-vocab/src/client/widgets.rs:300-322`) n'a que `add_button` et
`add_text_field`, plus un `draw` (`:394`). Ni ligne, ni défilement, ni liste. C'est le
même manque que `src/ui/MANQUANT.md` §1 et `ui_window_13/MANQUANT.md` §1 ; il coûte six
listes de plus ici.

**Ce que j'ai fait** : une liste est un `Field` de `Value::Row` plus les champs de sa
ligne, chacun dans `Scope::Row(<id de la liste>)` — donc `name` existe trois fois dans
la page d'alliance sans se confondre.

## 2. Le gestionnaire de quêtes, et le registre des modèles

**Ce que je voulais faire** : lire les trois listes comme Java les lit —
`getAvailableQuests()`, `getInProgressQuests()`, `getFinishedQuests()`
(`IQuestManager.java:94`, `:101`, `:108`) — et le nom de chaque quête dans
`IQuestManager.GLOBAL_SERVER_QUESTS` (`:22`).

**Ce qui existe** : rien. Le port n'a ni `IQuestInstance`, ni `IQuestTemplate`, ni
`FinishedQuest`, ni registre statique. `quest::Instance` et `quest::Finished` sont donc
des **structures d'entrée** remplies par l'appelant, pas des lectures du modèle. Le jour
où le port aura un gestionnaire de quêtes, elles deviennent les accesseurs de la vue et
rien d'autre ne bouge.

**Et un refus que Java n'a pas** : un index d'objectif hors du modèle
(`Refusal::ObjectiveOutOfRange`) et un modèle absent
(`ui_window_13::refusal::Refusal::UnknownTemplate`). Java, lui, indexe et lit un `null`.

## 3. Le moteur d'objectifs d'une quête

**Ce que je voulais faire** : porter le texte d'objectif tel quel,
`objectiveTemplate.getProgressText(quest, Style.EMPTY.withColor(...))`
(`WindowQuestLogInProgressQuestQuestModule.java:43`, `:51`).

**Ce qui existe** : rien. `IQuestObjectiveTemplate.getProgressText` est un moteur de
formatage (progression courante, cible, verbe) que le port n'a pas, et qui n'est pas
dans le paquet `questlog/` : il vit du côté `api/quests/`.

**Ce que j'ai fait** : `quest::Objective::text: Option<&str>` — l'appelant fournit la
chaîne déjà composée, et son absence est un refus nommé
(`Refusal::ObjectiveTextMissing`) plutôt qu'une ligne vide qui aurait l'air d'une quête
sans objectif.

## 4. Le gestionnaire de connexions, côté client

**Ce que je voulais faire** : lire `getDirectlyConnectedColonies()`,
`getIndirectlyConnectedColonies()`, `getConnectionEvents()` et
`getColonyDiplomacyStatus(id)` (`WindowAlliancePage.java:80-81`, `:204`, `:219`).

**Ce qui existe** : `colony::Colony` porte un id, un nom, une dimension, un centre, des
citoyens, des bâtiments, des ordres et des membres (`src/colony/colonie.rs:144-161`) —
**pas** de connexions ni de diplomatie. `IColonyConnectionManager` n'a pas été porté :
c'est pourquoi `TriggerConnectionEvent.event` voyage en `Vec<u8>`
(`src/network/MANQUANT.md` §1) au lieu d'un `ConnectionEvent` typé.

**Ce que j'ai fait** : `alliance::Page` est l'état de l'écran, rempli par l'appelant
depuis ses propres listes. `alliance::receive` porte la **règle** du gestionnaire serveur
(`ColonyConnectionManager.java:722-784`) sans le gestionnaire, ce qui permet de tester la
transition — et `alliance::event_bytes` fabrique maintenant les octets que
`TriggerConnectionEvent` attendait sans producteur, selon
`ConnectionEvent.serializeByteBuf` (`:26-31`). Le message du port a donc un émetteur : ce
qui manque encore est son **gestionnaire**, pas son format.

## 5. `BlockPosUtil.dist` en `int`

**Ce que je voulais faire** : la distance affichée, à l'identique —
`(int) BlockPosUtil.dist(colonyData.pos, colony.getCenter())`
(`WindowAlliancePage.java:179`), `Math.sqrt` d'une somme de carrés calculée en `int`
(`BlockPosUtil.java:563-569`).

**Ce qui existe** : aucun `BlockPosUtil` dans le port ; les positions sont des
`[i32; 3]`.

**Ce que j'ai fait** : `alliance::distance` calcule en `i64`. **C'est une divergence,
assumée et documentée** : au-delà de 46 340 blocs d'écart sur un axe, Java déborde et
affiche une distance fausse ; le port ne déborde pas et affiche la vraie. Reproduire un
débordement ne servirait aucun joueur.
