# `manager_03` — raid, animaux, jobs, interactions

Quatre gestionnaires, 1 842 lignes de Java, 3 572 lignes de Rust, **42 tests**.

| Fichier | Ce qu'il répond | Java |
| --- | --- | --- |
| `raid.rs` | raid autorisé, nombre de raiders, difficulté, nuits, coût d'une mort, mémoire de la colonie | `RaidManager.java:74-460`, `:700-1103` |
| `animal.rs` | à quel id un animal s'accroche, quel id un nouveau prend, ce qu'une ré-inscription jette, ce que la sauvegarde contient | `AnimalManager.java:74-446` |
| `jobdata.rs` | quel métier un nom sauvegardé désigne, et ce qu'un nom qui ne résout rien coûte | `JobDataManager.java:27-93` |
| `interaction.rs` | les sept types de dialogue, la classe que chacun reconstruit, ce qu'un `handlertype` invalide coûte | `InteractionResponseHandlerManager.java:22-49` |

## Ce qui était déjà porté, et qui n'est donc pas dupliqué ici

- **La machine à événements du raid est `crate::events::raid`** (`Lifecycle`, `Raid`,
  `Status`) : compteur d'id, `addEvent`, `onColonyTick` (start/update/finish/drop), les
  statuts « actifs ». `raid.rs` ne la réécrit pas : `Manager::is_raided` marche sur
  `Lifecycle::ids()` et interroge `Lifecycle::is_active`, qui **est**
  `IColonyRaidEvent.isRaidActive()`.
- **Le registre des métiers est `crate::jobs`** (`Job`, 50 classes, chemins, tables des
  huttes). `jobdata.rs` ne porte que la résolution du nom et ses échecs, avec
  `Job::from_path` comme registre.
- **La sauvegarde de colonie est `crate::persistence`**, qui dit lui-même
  (`src/persistence/MANQUANT.md:165`) que `animalManager` (`Colony.java:942`) n'a pas de
  module à qui confier un compound. `animal.rs` **est** ce module.
- Le `ColonyManager` (`crate::manager`) n'a rien de ces quatre gestionnaires : il n'y a
  pas de doublon.

## Prémisses fausses, mesurées

1. **« La sauvegarde des animaux fonctionne » — faux.** `Colony.write:940-942` donne un
   compound vide à `AnimalManager.write`, qui y met `animalManager`
   (`:347-357`), puis l'enveloppe **une seconde fois** sous la même clé. `Colony.read:772`
   passe à `read` le compound **intérieur**, dont `:280` cherche `animalManager` à
   l'intérieur de lui-même : rien n'y est, il sort. **Aucun animal ne survit à un
   redémarrage en Java**, en silence. `Registry::write`/`read` parlent la forme plate que
   `AnimalManager.read` attendait, et un test la fait l'aller-retour.
2. **Le javadoc de `registerAnimal` contredit son code.** `:70` dit que l'animal
   **existant** est retiré « if it is alive » ; `:116` teste le **nouvel arrivant**. Un
   arrivant mort est jeté et l'animal déjà lié reste. Porté tel quel (`Discard::NotAlive`).
3. **La boucle de réutilisation d'id ne peut pas échouer.** `:238-246` a `count + 1` places
   pour `count` entrées : une est libre. Le repli sur un `nextAnimalID` périmé est du code
   mort ; le port dit la preuve au lieu de garder le repli.
4. **`AnimalData.serializeNBT:155` invente un UUID à chaque sauvegarde** quand il n'en a
   pas (`uuid != null ? uuid : UUID.randomUUID()`) : l'identité de l'animal change d'une
   sauvegarde à l'autre. `Entry::uuid` est un `Option` ; une lecture signale
   `Repair::NoUuid` au lieu d'en fabriquer un.
5. **`onRaidEventFinished` lit `raidHistories.get(0)`** (`:1058`), la **plus ancienne**
   entrée, alors que tout le reste de la classe lit la dernière ; sur une liste vide, il
   lève une `IndexOutOfBounds`. Le port garde la sémantique (c'est la fin « miséricorde »
   qui en dépend) et refuse proprement (`Refusal::NoHistory`). Le contrôle « un raid tourne
   encore » vient **avant** (`:1050-1056`), ce qu'un test fixe.
6. **Un refus du raid n'est pas atomique.** `:442-445` rend `NO_SPAWN_POINT` **après**
   l'écriture de l'historique et la remise à zéro des nuits, et **avant**
   `colony.markDirty()` (`:458`) : l'entrée reste en mémoire sans être marquée à sauver.
   `raider_event` reproduit les trois effets, test à l'appui (`pirate_ground_raid` et
   `norsemen_ship_raid` sont produits par l'échelle et jamais acceptés en entrée, ce qui
   rend le chemin atteignable).
7. **La difficulté enregistrée n'est pas 0,9 mais 0,89.** `7 / 10.0 + 0.2` vaut
   0,8999999999999999 en binaire, et `((int) (x * 100)) / 100.0` (`:456`) tronque : 0,89.
   Le port calcule pareil, et le test le dit.
8. **`setPassThroughRaid` (`:1100-1103`) n'a aucun appelant** dans cette version, et
   `passingThroughRaidTime` n'est jamais remis à zéro : `passingThroughRaidTime > 0`
   (`:740`) reste vrai à vie. Corollaire mesuré : `isRaided(0)` est **vrai** au tick 0
   (`:723`), donc la première nuit d'une colonie ne compte pas.
9. **`JobDataManager` : trois contrôles morts et un piège.** `jobType == null` (`:34-37`)
   ne peut pas arriver ; `Optional.ofNullable(jobEntry)` (`:44`) re-teste ce que `:38` a
   déjà écarté ; le `else` « Unknown Job type » (`:64-67`) n'est atteignable que par un
   producteur qui rend `null`. Le piège : `new ResourceLocation("builder")` donne
   `minecraft:builder`, le registre ne connaît pas, **le citoyen perd son métier pour un
   deux-points manquant** (`NameRefusal::Foreign`). Et un `type` qui n'est pas une chaîne
   donne `""`, `new ResourceLocation("")` lève, et rien n'attrape avant `Colony.read` :
   c'est le seul refus de ce fichier qui coûte la colonie.
10. **`InteractionResponseHandlerManager` : l'avertissement « type inconnu » est
    inatteignable.** `:28` fait `getValue(handlerType).getProducer()` : quand le registre
    n'a pas l'entrée, `getValue` rend `null` et le `.getProducer()` lève un NPE **avant**
    le `if (handler != null)` de `:29`. Le `else` `:45-48` n'est atteignable que par un
    producteur qui rend `null` — le port sépare les deux (`KindRefusal::Unknown` /
    `Refusal::NoProducer`). Deux autres faits : `simplenotification` est enregistré avec
    `StandardInteraction` (`ModInteractionsInitializer.java:26-30`), donc il revient en
    `StandardInteraction` et se re-sauve `standard` ; et l'appelant
    (`CitizenData.java:1502`) déréférence le retour `@Nullable` (`handler.getId()`), le NPE
    qui en sort étant ce que `:1504` journalise en « Did the quest vanish? ».

## Écart avec la cible annoncée

La cible « 600 à 1 000 lignes, 30 tests » correspond à la moitié d'**un** de ces quatre
fichiers (`RaidManager.java` fait 1 253 lignes à lui seul). Le port livre 3 572 lignes et
42 tests, au ratio du dépôt (1,9 ligne de Rust par ligne de Java ici, 4,6 pour
`manager_01`) : une doc de module qui cite chaque seuil, un refus nommé par sortie, et un
test par règle. Rien n'a été coupé côté sources plutôt que côté substance ; ce qui n'est
**pas** porté est listé dans `MANQUANT.md`, avec ce que l'appelant doit fournir.

## Vérification

`port-check manager_03` reste **ROUGE**, mais uniquement sur les dossiers d'autres agents
en cours d'écriture, et la liste bouge d'un run à l'autre : `ai_work_10/rabbit.rs`,
`ai_work_10/shepherd.rs`, `ai_work_11/mod.rs`, `ai_work_11/tests.rs`,
`ui_window_13/quest_log.rs`, `manager_04/mod.rs`. **Aucune erreur ne cite
`manager_03`**, sur six passages.

Le module a donc été vérifié isolément, par une crate sonde sur l'hôte
(`/tmp/m03probe`) qui inclut `src/manager_03/mod.rs` par `#[path]` avec les vraies
dépendances (`mod_api`) et le vrai `crate::jobs::Job` :

```text
test result: ok. 52 passed; 0 failed        (42 de manager_03 + 10 de jobs)
cargo clippy --all-targets : aucun avertissement dans src/manager_03
cargo doc avec RUSTDOCFLAGS=-D warnings : propre
```
