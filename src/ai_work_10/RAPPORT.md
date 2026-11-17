# Les trois éleveurs restants : lapinier, berger, palefrenier

Tranche `src/ai_work_10`. Sources portées, dans
`core/entity/ai/workers/production/herders/` :

| Métier | Classe | Lignes | Ce qu'elle ajoute |
| --- | --- | ---: | --- |
| lapinier | `EntityAIWorkRabbitHerder.java` | 69 | **rien** : une frappe au tirage et une icône |
| berger | `EntityAIWorkShepherd.java` | 188 | `SHEPHERD_SHEAR` (`:50`), la surcharge de `decideWhatToDo` (`:73-87`), la teinture (`:173-187`) |
| palefrenier | `EntityAIWorkStablemaster.java` | 757 | `HERDER_TRAIN` / `HERDER_READY_FOR_COMBAT` / `HERDER_GATHER_MOUNTS` (`:102-104`), la surcharge avec deux comptes à rebours (`:232-262`), le dressage (`:268-339`), la préparation au combat (`:346-422`, `:429-514`), la rentrée au bercail (`:563-643`) |

Le parent commun, `AbstractEntityAIHerder.java` (874 lignes), **n'est pas rejoué** : il est
déjà porté par `crate::ai_work_09::herder`, une seule fois.

## 1. Ce qui est écrit

| Fichier | Lignes | Contenu |
| --- | ---: | --- |
| `state.rs` | 488 | `HerdState` (9 états, 8 nommés + `Base(WorkerState)`), `HerdChoice` (les 5 réponses de la table de base), `Hand`, `TradeRefusal` (26 refus nommés) |
| `facts.rs` | 783 | `PenAnimal`, `PenFacts`, les trois `…Facts` de métier, `Draws`, `HerdAct`, `HerdMemory`, `HerdScene`, `arrived` |
| `mod.rs` | 550 | `HerdTrade`, `HerdAi`, `Decision`, `Outcome`, `register`, la couture avec `ai_work_09` |
| `rabbit.rs` | 163 | `butcherAnimal` (`:54-68`), la borne du tirage, l'icône |
| `shepherd.rs` | 201 | la surcharge, `shearSheep`, `dyeSheepChance`, Fortune, les dégâts |
| `stablemaster.rs` | 667 | les trois buts, `readyMount`, `feedHorse`, laisse, recherche à 30 blocs, durées |
| `tests.rs` | 1458 | 77 tests |

**Mesure totale : 4 312 lignes, dont 1 458 de tests.** C'est au-dessus des 900-1 400
demandés, et la raison est mesurable : les trois classes Java en font 1 014 à elles seules,
le palefrenier en porte 757, et la couche de faits (`facts.rs`, 783 lignes) n'a pas
d'équivalent Java — elle remplace les lectures du monde, une par `fichier.java:ligne`.
Réduire d'un tiers aurait voulu dire retirer des citations ou des tests ; je ne l'ai pas
fait. Si la cible est ferme, les blocs à couper sont les docs de `facts.rs` (le champ par
champ) et l'icône du lapinier.

## 2. La machine, et pourquoi elle ne recopie pas celle du parent

`HerdAi::new(HerdTrade::…)` construit une `Machine<HerdState, HerdScene>` sur
`crate::ai::state::Machine` (les trois passes, le compte à rebours, la règle de piège —
`BasicStateMachine.java:152-175`, `:199-236`, `TickRateStateMachine.java:73-105`). Elle
enregistre :

| État | Règle | Cadence | Java |
| --- | --- | ---: | --- |
| `Base(DECIDE)` | la surcharge | 80 | `AbstractEntityAIHerder.java:143` + `:73-87` / `:232-262` |
| `SHEPHERD_SHEAR` | `shearSheep` | 20 | `EntityAIWorkShepherd.java:50` |
| `HERDER_TRAIN` | `convertMount` | 10 | `EntityAIWorkStablemaster.java:102` |
| `HERDER_READY_FOR_COMBAT` | `readyMountForCombat` | 10 | `:103` |
| `HERDER_GATHER_MOUNTS` | `gatherMounts` | 10 | `:104` |
| `IDLE`, `START_WORKING`, `PREPARING`, `HERDER_BREED/BUTCHER/PICKUP/FEED` | reprise : la table propriétaire vient de décider → `DECIDE` | 1 | `AbstractEntityAIHerder.java:140-147` |

Aucun de ces états traversés n'a de règle ici : ce sont les états de `crate::ai`
(`worker::register_base`) et d'`ai_work_09::herder` (`herder::register`). La reprise est le
fil qui rend la main, pas une deuxième implémentation.

**La couture avec le parent.** `AbstractEntityAIHerder.decideWhatToDo` a exactement cinq
sorties (`:232`, `:236`, `:240`, `:244`, `:248`) ; `HerdChoice` est cet ensemble et rien
d'autre. L'appelant — qui fait tourner la table de base d'`ai_work_09` — dépose sa réponse
dans `PenFacts::base_answer`, exactement là où la sous-classe Java lit
`super.decideWhatToDo()`. Sans réponse déposée, la surcharge refuse
`BaseDecisionMissing` au lieu de deviner. Le détail et le coût : `MANQUANT.md` §6.

## 3. Une prémisse du brief qui ne tient pas

Le brief demande « les états (chercher, aller, agir, rendre) et leurs transitions ». Ce
schéma à quatre cases **n'est pas** la forme de ces trois buts, et l'aplatir changerait le
comportement observable :

- **Une action Java branche à l'intérieur d'un seul tick de cible.** `shearSheep`
  (`EntityAIWorkShepherd.java:110-166`) cherche, équipe, teste la marche et agit dans le
  même appel ; si la marche n'est pas finie elle rend `getState()` (`:129`), donc elle
  consomme la cadence entière de 20 ticks. Une machine « chercher → aller → agir → rendre »
  ferait payer une cadence par phase, soit jusqu'à 4 × 20 ticks là où Java en met 20.
- **Le palefrenier a trois buts qui se chevauchent, pas quatre phases.** `HERDER_TRAIN`,
  `HERDER_READY_FOR_COMBAT` et `HERDER_GATHER_MOUNTS` sont trois états **alternatifs**
  choisis par la décision (`:238`, `:244`, `:250`), pas trois étapes d'un même travail. Les
  états de ce module sont donc ceux que Java nomme, un par cible, et les branches internes
  sont des `TradeRefusal` nommés (`NotAtMount`, `MountRidden`, `NoStall`, …) — ce qui donne
  la lisibilité des quatre phases sans changer le nombre de ticks.

Le seul endroit où les quatre phases existent vraiment, c'est le palefrenier : `convertMount`
cherche (`:306-335`), marche (`:273-276`), agit (`:279-295`) puis rend `DECIDE` (`:294`), et
`gatherMounts` fait de même avec la stalle (`:589-618`). C'est ce que la suite d'états de
`tests.rs` montre, tick par tick.

## 4. Les décisions qui méritent d'être relues

1. **La main n'est vidée que dans le chemin qui rend la main.** `EntityAIWorkShepherd.java`
   `:81` est un `return`, `:84` ne l'est pas : un berger qui part tondre garde ses
   cisailles. Test : `a_shepherd_that_leaves_to_shear_keeps_its_shears_in_hand`, contre
   `shearing_off_hands_the_decision_back`.
2. **La tonte exige que la décision de base ait rendu `START_WORKING`** (`:79`) : un
   troupeau accouplable passe devant la tonte même avec `SHEARING` actif.
3. **`enchantmentLevel *= Math.max(1.0, primary / 5.0)` tronque** (`:132-133`) : Fortune III
   à 30 d'habileté donne une borne de 18, pas 18.0. Test : `the_fortune_multiplier_is_a_truncated_double_product`.
4. **La teinture passe pour `rand <= niveau`** (`:180`), pas `<`. Au niveau 1, deux tirages
   sur cent.
5. **Les cinq réponses de base sont traversées, pas décidées** : `HerdChoice` les nomme, la
   machine s'y trouve, `ai_work_09` les fait tourner.
6. **Le compte à rebours d'entraînement initial vaut 50** : le premier `DECIDE` le met à
   49, donc `<= 0` est faux et le palefrenier ne peut pas dresser avant cinquante
   décisions. Test : `the_first_decision_never_trains`.
7. **`&&` court-circuite, donc le nombre de tirages varie** : le tirage de `:238` n'est pas
   consommé quand le compte à rebours est positif, celui de `:244` l'est toujours. Les
   tirages sont nommés par usage (`Draws`) pour que la décision reste rejouable.
8. **Une écurie sans stalle étiquetée est refusée** au lieu de lever la
   `NullPointerException` de `:591` (`BuildingStable.java:128-131`). Refus nommé :
   `NoStall`.
9. **Le filtre de réservation de `:753` ne mord pas** (il teste `horseToRetrieve`) : porté
   tel quel, verrouillé par un test.
10. **`readyMount` prend l'objet dans la hutte sans vérifier l'arrivée** (`:442` non lu,
    `:444` lit l'inventaire de la hutte) : porté tel quel.

## 5. Les tests

77 tests dans `tests.rs`, en quatre familles :

- **les états et les refus** (7) : les 8 états et leur drapeau `isOkayToEat`
  (`AIWorkerState.java:357-414`), les 5 réponses de base, les 26 refus (clé unique, ligne
  citée), l'arrivée `<=` contre `is_living_at_site` `<`, l'enclos, le métier qui ne
  correspond pas ;
- **l'enregistrement** (5) : zéro cible pour le lapinier, une à 20 ticks pour le berger,
  trois à 10 pour le palefrenier, `DECIDE` à 80, aucun état en piège ;
- **les règles, une par branche** (49) : la frappe du lapinier (borne à 0/50/95/100/140,
  `<= 1`, main vide, hors portée, usure seulement si le coup passe), la tonte (les trois
  conditions de `:79`, la marche, la laine, teinture, les dégâts), l'écurie (les deux
  comptes à rebours, les trois tirages, le candidat, la limite `niveau × 2`, la conversion,
  la santé, la nourriture prise à la hutte, la requête unique, le cuir, l'armure, la laisse,
  la stalle, le rayon de 30) ;
- **les suites d'états, tick par tick** (4) : `DECIDE → SHEPHERD_SHEAR → DECIDE` ;
  `DECIDE → HERDER_TRAIN → (marche) → conversion` ; `DECIDE → HERDER_GATHER_MOUNTS →
  laisse → bercail` ; la reprise sur `DECIDE` et les 16 ticks d'IA entre deux décisions.

## 6. Ce qui n'est pas fait

- **Aucune compilation n'a été lancée** (consigne de l'orchestrateur : un build lancé par
  un agent met la machine en famine E/S). Le module n'a donc pas encore vu un compilateur :
  la première chose à faire est de le compiler, et les erreurs de type éventuelles seront
  de la mienne, pas du Java.
- **Ce module importe cinq constantes et `TICKS_SECOND` de `crate::ai_work_09`**
  (`DECIDING_DELAY`, `ONE_HUNDRED_PERCENT`, `BUTCHERING_ATTACK_DAMAGE`, `XP_PER_ACTION`,
  `ANIMAL_MULTIPLIER`). C'est le raccordement voulu par la règle « ne duplique pas », mais
  il a un coût : si `ai_work_09` est rouge, ce module ne compile pas non plus, pour une
  raison qui n'est pas dans mon dossier. Les noms à ne pas renommer sous mes pieds sont
  ceux-là.
- **Pas de harnais de tick de bout en bout** avec `ai_work_09` dans le même test : les deux
  scènes sont deux types différents (voir `MANQUANT.md` §6). La couture est testée de
  chaque côté, pas en un seul appel.
- **Pas d'intégration** dans `lib.rs`, `data/` ou `assets/` : hors périmètre de cette
  tranche, l'orchestrateur déclare les modules.
- **Les sons, particules, statistiques et l'expérience** ne sont pas portés (voir
  `MANQUANT.md` §1) : aucune décision n'en dépend.
