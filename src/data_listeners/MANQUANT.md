# MANQUANT — ce que `src/data_listeners` ne peut pas faire, et ce qu'il faudrait

Écrit pour les neuf écouteurs de données. Chaque entrée dit **ce que je voulais faire**, **ce que
l'SDK ou le dépôt offre réellement**, **ce que j'ai fait à la place**, et **ce que ça coûte**.
Aucun contournement silencieux : le module n'importe rien (`use std::collections::BTreeSet` et c'est
tout), et les huit choses qu'il ne peut pas exprimer sont nommées ci-dessous.

Déclaration citée : `Cargo.toml` (« no registry dependency, no serde, no game internal »).

---

## 1. Pas de lecteur JSON : la ligne arrive déjà lue

`SimpleJsonResourceReloadListener.apply` reçoit une `Map<ResourceLocation, JsonElement>` : le moteur du
jeu a déjà lu les fichiers. Les neuf corps Java ne font ensuite que **décider quoi faire d'une ligne**.

Le dépôt n'a pas de `serde` et n'a pas le droit d'en ajouter (`Cargo.toml`), et le parseur NBT écrit à
la main de `port-blueprint` vit dans un autre dépôt. Écrire un troisième lecteur JSON pour 40 clés
aurait dupliqué `port-blueprint` sans porter une seule règle de plus.

**Fait à la place** : chaque fichier du module déclare sa propre `Row` typée (`citizen_names::Row`,
`visitors::Row`, `nbt_keys::Row`…) sur le modèle de `citizens::happiness::disease::Row`, et n'expose que
les refus que la ligne *lue* permet de nommer.

**Coût** : une erreur de **forme** n'est pas refusée ici. `ItemNbtListener.java:60` attend un tableau à
la racine du fichier ; un objet y donne une `ClassCastException` que le `catch` de `:74-77` transforme en
file perdue. Cette règle est documentée dans `nbt_keys` mais pas exécutable. De même `get("parts")` sur
une clé absente est un NPE en Java : `Row::parts` est un `Option` et [`citizen_names::Refusal::MissingField`]
le refuse, mais c'est l'appelant qui doit remplir le `Option`.

**Ce qu'il faudrait** : un lecteur `&str -> Value` partagé par le dépôt (300 lignes, aucune dépendance),
et les `Row` deviendraient des `fn parse(Value) -> Result<Row, Refusal>`.

---

## 2. Aucun registre : « cet item existe » est une question posée au dehors

Trois écouteurs refusent une ligne parce que son item n'existe pas (recrutement `:146-150`, étude
`:120`, compatibilité `:70-73`). La réponse vit dans `ForgeRegistries.ITEMS`, que ce crate n'a pas.

**Fait à la place** : `known_item: &dyn Fn(&str) -> bool`, fourni par l'appelant.

**Coût** : un appelant paresseux peut répondre `true` partout et retrouver le comportement Java. La
règle est alors décorative. C'est le même compromis que `research::gate` (les conditions de bâtiment
arrivent en `bool`) et que `citizens::happiness::disease` (le tirage arrive en `i32`).

---

## 3. Aucune source aléatoire : les tirages arrivent faits

`chanceCustomVisitors` tire un `nextDouble` par visiteur (`:256`), `getRandomRecruitCost` un gaussien
(`:97`), `generateName` un index et une lettre (`CitizenData.java:458-472`). Un mod wasm qui tire ses
propres nombres ne peut pas être rejoué — la décision est déjà prise ailleurs dans le dépôt
(`citizens::needs::Appetite::cook_skips`).

**Fait à la place** : `visitors::first_chance(pack, rolls)`, `NameFile` + `compose(file, first, initial,
last)` ; la lettre du prénom composé est un paramètre.

**Coût** : rien sur les règles, tout sur l'ergonomie de l'appelant. Un test peut fixer exactement la
suite de tirages, ce qui est justement l'intérêt.

---

## 4. Aucun accès à la colonie : le niveau du bâtiment est un paramètre

`RecruitmentItemsListener.getRandomRecruitCost(building.getBuildingLevel())`
(`TavernBuildingModule.java:196`) lit un champ d'une colonie chargée. Le module n'a ni colonie ni
bâtiment.

**Fait à la place** : `recruitment::band(building_level)` et `recruitment::pick(&table, rarity)`. La
règle « le niveau 4 demande les paliers 10 à 12 » est donc testable sans serveur, ce qui est la seule
façon de la prouver : en jeu il faut une taverne niveau 4 et un tirage à 6,7 %.

---

## 5. Aucun journal : un refus est une valeur, pas une ligne

Les neuf fichiers Java écrivent dans `Log.getLogger()` (`CitizenNameListener.java:86`,
`ResearchListener.java:261`, `:298`…). Le module n'a pas d'accès au journal et n'en veut pas : une
règle qui écrit dans un journal n'est pas testable.

**Fait à la place** : `Refused::name()` rend une chaîne stable et namespacée
(`data_listeners.refusal.recruit_rarity_out_of_range(tavern/bread.12)`), et `Report::names()` les
rassemble. L'appelant décide du niveau (`warn`, `error`, silence).

**Coût** : les niveaux de gravité de Java (`warn` pour un effet inconnu, `error` pour un parent
manquant, `:583-587`) ne sont pas dans le type. Un appelant qui veut les respecter doit les relire dans
le Java ; le rapport les liste.

---

## 6. Aucun événement de rechargement propre : l'ordre est une valeur

`timeline::STEPS` décrit ce qu'un `/reload` exécute, d'après le commentaire de
`DataPackSyncEventHandler.java:24-40`. Rien ne l'exécute : ce module n'a ni serveur, ni datapack, ni
système de ressources.

Le bus déjà porté (`src/events/`) connaît `Topic::CustomRecipesReloaded`, posté par
`CustomRecipeManager.java:438` — mais depuis le **gestionnaire de paquet client** (`:412`), pas depuis un
rechargement. Ce module **ne poste rien** : `timeline::POSTED_ON_RELOAD` est vide, et
`timeline::LOOKS_LIKE_RELOAD` nomme l'événement voisin avec sa ligne pour qu'on ne le câble pas au
mauvais moment. `src/events` reste la seule implémentation de bus : aucun second n'a été créé, et
`EventBus` n'est pas instancié ici.

**Coût** : `timeline` est une table vérifiable, pas un ordonnanceur. Le jour où un hôte appellera ces
listeners, c'est lui qui devra respecter [`timeline::STEPS`] et `timeline::complete_at`.

---

## 7. L'évaluation de `triggerOrder` est ailleurs

`QuestJsonListener.parseTriggerOrder` construit un arbre (`:314-390`) et `evaluate` le parcourt contre
une `IColony` (`:399-453`). Ce module porte la **validation** (jetons, indices, arité) et pas
l'évaluation : `src/quests/` appartient à un autre agent, et `ITriggerReturnData` demande une colonie.

**Coût** : un ordre validé n'est pas encore un ordre évalué. La coupure est volontaire : tout ce que
Java fait mal se voit à la validation (voir `trigger`), et tout ce que Java fait bien demande une
colonie.

---

## 8. `citizensuffix` et la voix : lus et jamais appliqués

`CustomVisitorListener.applyToVisitor` lit `voiceProfile` puis appelle `getVoiceProfile()` sans setter
(`:207-210`) : la valeur ne va nulle part. Elle est conservée dans `Visitor::voice_profile` telle
quelle, avec la ligne, plutôt que supprimée — un port qui l'oublie ne peut plus dire ce que le Java
fait.

**Coût** : un champ qui ne sert à rien, et c'est la vérité sur la source.
