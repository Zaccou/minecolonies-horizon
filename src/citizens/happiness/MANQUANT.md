# Ce qui manque au module `citizens/happiness`, et ce que je voulais faire

Chaque entrée dit **ce qui n'est pas possible**, **pourquoi**, et **quel code
aurait été écrit**. Rien de tout cela n'est du code fictif dans les `.rs` : ce
fichier est la liste des trous.

## 1. Aucun tick par citoyen, donc rien ne fait tourner ce module

`crates/horizon-mod-vocab/src/hooks/entity_tick_pre.rs:19` est un hook **global** :
il reçoit une entité, pas « le citoyen X à son tour ». Il n'existe donc aucun
appel périodique par citoyen, et sans lui :

- `Happiness::night_fall` (le `dayEnd` de `CitizenHappinessHandler.java:116-134`)
  n'est appelé par personne ;
- `Sickness::update` (`CitizenDiseaseHandler.java:100-120`, toutes les 60 ticks)
  n'est appelé par personne ;
- `Sickness::contact` (`:148-172`, sur collision) n'est appelé par personne.

Ce que je voulais écrire : dans le hook de tick, `if colony.is_dusk_edge() {
citizen.happiness.night_fall(&facts) }` et `if tick % 60 == 0 {
sickness.update(60, &gate, modifier, config, draw) }`. Les deux fonctions existent
et sont testées ; c'est la **couture** qui manque.

## 2. Pas d'horloge, donc pas de « tombée de la nuit »

`Happiness::night_fall` doit être appelée au **crépuscule**, pas à l'aube :
`Colony.checkDayTime` compare `isDay` à `WorldUtil.isDayTime(world)` et n'appelle
`checkCitizensForHappiness` qu'au passage jour → nuit
(`core/colony/Colony.java:643-651`). Le port a bien un `World::day_time` dans
`citizens::needs`, mais aucun état de colonie ne retient le jour précédent, donc
personne ne détecte le front.

Ce que je voulais écrire : un `last_day_time: i64` dans l'état de colonie, et
`fn dusk_edge(&self, world: &World) -> bool`. `World::is_past_time` existe déjà et
suffit au calcul ; le champ, lui, est dans `crate::colony`, hors de mon périmètre.

## 3. Aucun tirage aléatoire dans le module, par choix

`CitizenDiseaseHandler` tire deux fois par mise à jour (`nextInt(configModifier *
DISEASE_FACTOR)` puis `DiseasesListener.getRandomDisease`) et une fois par
collision (`nextInt(100)`, plus un `nextBoolean` pour les masques). Le port reçoit
ces réponses en valeurs (`disease::Draw`, `disease::Collision`), exactement comme
`needs::Appetite::cook_skips` reçoit le tirage du cuisinier.

Le SDK **a** une source aléatoire — `Host::level_random` →
`horizon-mod-vocab/src/host/levels.rs:606` — mais l'utiliser ici ferait entrer un
appel à l'hôte dans un module qui est aujourd'hui de la donnée pure : plus de test
sans wasm, plus de rejeu. C'est la couture (point 1) qui doit tirer, pas ce module.

Ce que je voulais écrire : rien de plus ici ; un `fn draw(&mut host) -> Draw` dans
le fichier de hook.

## 4. Aucun codec NBT : les modificateurs ne survivent pas à un rechargement

Java sérialise chaque modificateur (`read`/`write`,
`AbstractHappinessModifier.java:110-133`, `TimeBasedHappinessModifier.java:124-160`,
`ExpirationBasedHappinessModifier.java:240-255`) sous `happinessHandler`, et
`CitizenData.read` ignore purement et simplement un fichier sans ce tag (`:160-183`).
Le port n'a **rien** : un citoyen rechargé repart avec les dix modificateurs neufs
et perd `damage`/`death`/`quest`/`greatfood`/`raidwithoutdeath` en cours.

Ce que je voulais écrire : `Happiness::to_compound(&self) -> Compound` et
`Happiness::from_compound(&Compound) -> Result<Happiness, Refusal>`, en réutilisant
`crate::building::nbt` (`mod_api::{Compound, Tag}`), avec un refus nommé pour un id
inconnu — ce que Java fait silencieusement (`:174-181`).

Le champ `source()` et `Source::from_name` sont déjà là pour ça ; il ne manque que
les deux fonctions, et elles tombent dans un fichier que je n'ai pas ouvert pour ne
pas gonfler encore un module déjà long.

## 5. Pas de pile d'objets : une maladie se décrit, elle ne se soigne pas

`Disease::cures` porte `{"minecraft:carrot", 1}` — le nom de registre et le
nombre. Rien ne cherche ces objets dans un inventaire, rien ne les retire. Java le
fait en trois endroits : le médecin (`EntityAIWorkHealer.java:311-434`), le lit
d'hôpital (`EntityAISickTask.java:278-303`) et la pomme dorée
(`EntityCitizen.java:479-482`).

Ce que je voulais écrire : `fn can_cure(disease: &Disease, inventory: &Inventory)
-> bool` au-dessus de `crate::inventory`, et l'extraction dans le même geste.
`crate::inventory` existe et sait déjà chercher un objet ; c'est la jointure qui
manque, et elle appartient au système de requêtes.

## 6. Pas de traduction : les identifiants sortent, pas les phrases

`processDailyHappiness` déclenche des interactions `NO + id` et `DEMANDS + id`
(`CitizenHappinessHandler.java:124-131`), et la fenêtre du bonheur affiche le nom
de chaque facteur. Le port rend `Source::name()` (`"homelessness"`, `"greatfood"`…)
et `effects::label` (`"7/10"`), et s'arrête là : `assets/minecolonies/lang/` n'est
pas dans mon périmètre et les interactions sont une autre tranche.

Ce que je voulais écrire : `fn interaction(source: Source, days: i32) -> Option<&'static
str>` avec les clés `com.minecolonies.coremod.no.homelessness` / `.demands.…`, en
suivant `InteractionValidatorInitializer.java:264-285`.

## 7. Pas de lien avec les bâtiments : l'hôpital et le lit

`sleeps_at_hospital` est un booléen que le module lit ; Java l'écrit quand le
citoyen trouve un lit (`EntityAISickTask.java:165-210`) et le remet à zéro dans
`cure()` (`:226`). De même `getMysticalSiteMaxBuildingLevel()` est un niveau que
`ColonyFacts` reçoit tout fait : la boucle qui prend le maximum des sites
mystiques vit dans `RegisteredStructureManager.java:528-542`, hors périmètre.

Ce que je voulais écrire : `ColonyFacts::counted(buildings: &[Building], …)` en
prenant le max sur `Kind::MysticalSite`. `crate::building::Building::level()`
existe ; l'énumération d'une colonie, elle, est dans `crate::colony`.

## 8. `getBuildingLevelEquivalent` n'a qu'un override, et il ne concerne pas les maisons

Le facteur nourriture lit `getBuildingLevelEquivalent()`
(`CitizenHappinessHandler.java:285`) alors que le facteur logement lit
`getBuildingLevel()` (`ModHappinessFactorTypeInitializer.java:44`). La différence
existe bien : `ICommonBuilding.java:42-45` renvoie le niveau par défaut, et
**`BuildingGateHouse.java:98-101` est le seul override du dépôt** — une maison ne
l'override pas. Le port garde donc un seul `Housing::level` et cite les deux
lignes. Si un jour un bâtiment d'habitation override l'équivalent, il faudra deux
champs.

## 9. Les valeurs de configuration sont des nombres, pas des lectures

`diseaseModifier` (défaut 5, bornes 1..100, `ServerConfiguration.java:136`),
`initialcitizenamount` (défaut 4, `:125`) et `foodModifier` (défaut 1.0, `:135`)
arrivent en paramètres (`disease::Gate`, `disease::Sickness::update`,
`needs::Advance::food_modifier`). Le port n'a pas de `ServerConfiguration` ; quand
il en aura une, ce sont trois `copy` de plus, pas trois règles.

## 10. L'interface : la fenêtre du bonheur et les cœurs

`HappinessWindowCitizen.java` (la liste des facteurs avec leur valeur et leur
poids), `WindowCitizenPage.java:143` (`"7/10"`) et `CitizenWindowUtils.java:304-340`
(les cœurs par demi-cœur) sont du rendu. `Happiness::breakdown` rend exactement la
donnée que ces trois écrans dessinent — `(source, facteur, poids, compté)` — et
`effects::label` / `effects::hearts` la mettent en forme. Le dessin, lui, est côté
client.
