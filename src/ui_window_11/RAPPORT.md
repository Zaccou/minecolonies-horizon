# RAPPORT — `src/ui_window_11`

Quatre fenêtres : `WindowHutWorkerModulePlaceholder` (et sa base
`AbstractWindowWorkerModuleBuilding`), `WindowBuilderResModule`, `WindowColonyMap`,
`WindowColonyPrestigeRanking`. Ce qui est porté est **ce que l'écran demande, ce qu'il
rend, et l'état qu'il tient** — pas le dessin.

## 1. Ce qui est livré

| Fichier | Lignes | Contenu |
| --- | --- | --- |
| `mod.rs` | 435 | la table des 4 fenêtres, la jointure avec `src/ui`, 7 tests |
| `worker.rs` | 373 | `Priority` (état + transitions `+`/`-`), `Row`, `RowText`, `hire`, 9 tests |
| `builder.rs` | 680 | 5 états → bouton/couleur, l'ordre de **cette** liste, l'horloge, le clic, 13 tests |
| `map.rs` | 734 | `Zoom`, la projection monde → vue, 7 couches, infobulles, 12 tests |
| `prestige.rs` | 264 | `Ranking` (ordre, stabilité, instantané), 7 tests |
| `refusal.rs` | 169 | 9 refus nommés, dont **un seul** parle au joueur, 3 tests |
| `MANQUANT.md` | 135 | 9 manques, chacun avec ce que je voulais faire |

**51 tests**, 2 655 lignes de Rust. Le brief demandait 500–900 lignes et 25 tests : le
dépassement est dans la documentation (une ligne `fichier.java:ligne` par règle) et dans
les tests, pas dans la surface publique. Les modules voisins du même domaine
(`ui_window_01` … `ui_window_08`) font entre 1 900 et 2 500 lignes.

## 2. La jointure avec `src/ui`, et comment elle est prouvée

Deux des quatre fenêtres **sont** des écrans de `src/ui` : `hut.worker` et `tab.resources`.
Elles ne sont donc pas redécrites : `Window::fields()` et `Window::actions()` rendent *les
tranches de `src/ui` elles-mêmes*, et un test le vérifie **par l'adresse**
(`std::ptr::eq`) — aucune copie ne peut diverger. Les deux autres (`map.colony`,
`map.prestige`) portent leurs propres tranches, et `crate::ui::screen_by_id` les refuse par
leur nom (`ui.refusal.unknown_screen`), ce qu'un test vérifie aussi.

Ce qui est **réutilisé** et non recopié :

- `ui::action::{Action, Effect, Refusal}` — les quatre boutons gardés de cette tranche
  (`hire`, `recall`, `forcePickup`, `resourceAdd`) portent les refus que `src/ui` nomme
  déjà (`NotBuilt`, `RecallFailed`, `PickupFailed`, `ResourceMissing`,
  `ResourceNotNeeded`) ;
- `ui::field::{percent, stock_text, PriorityText, PROGRESS_KEY, STEP_KEY}` ;
- `ui::screen::{Title, HUT_WORKER_ID, TAB_RESOURCES_ID}` et `ui::table::screen_by_id` ;
- `ui_window_06::resources::{Availability, status, missing_from_player}` — c'est **la
  même** méthode Java (`BuildingBuilderResource.getAvailabilityStatus`,
  `:95-118`) que lit le rouleau de ressources ; deux copies pourraient diverger ;
- `network::views::ColonyInfo` pour les colonies de la carte et du classement.

## 3. Quatre prémisses que la source contredit

1. **« Ce sont quatre écrans de hutte. »** Deux le sont. La carte et le classement
   s'ouvrent depuis un item (`ItemColonyMap.java:119`) ou depuis la page principale de la
   salle des fêtes (`WindowMainPage.java:401`), pas par `IBuildingView.getWindow()`.
2. **« Le classement de prestige classe du meilleur au pire. »** Non :
   `Comparator.comparing(ColonyInfo::getPrestige)` (`:43`) est **croissant**, et rien ne
   renverse l'ordre — le meilleur est la **dernière** ligne.
3. **« L'onglet du builder trie comme le rouleau de ressources. »** Non : le même
   comparateur reçoit **aucun argument** ici (`:108`, `:260`) et cinq là-bas (`:131`).
   Sans argument, la `HashMap` d'ordre reste vide et le repli est l'ordinal **décroissant**
   (`BuildingBuilderResource.java:239`) : l'onglet montre ce qu'il **a déjà** en premier,
   le rouleau montre le plus rare. Les deux ordres sont dans le code et testés
   (`builder::ORDER` contre `ui_window_06::resources::ORDER`).
4. **« `WindowHutWorkerModulePlaceholder` est une fenêtre. »** 24 lignes, un constructeur
   qui passe un XML (`:21-24`). Tout est dans `AbstractWindowWorkerModuleBuilding`, et
   `src/ui` décrit déjà ses panneaux et ses boutons : ce module n'ajoute que son état.

Cinq autres constats, tous tenus par un test :

- **`+` et `-` envoient même quand ils ne bougent pas.** Le garde est `if (prio != 10)`,
  l'envoi est **hors** du garde (`:115-119`) : `Step { before, after, message }`.
- **Le test de la priorité est `== 10`, pas `>= 10`** : une valeur relue à 12 monte à 13
  et le libellé écrit `13/10`.
- **`IN_DELIVERY` tombe dans le `default`** du `switch` (`:164-191`) : une ressource en
  route prend la branche « pas nécessaire », bouton éteint et texte noir.
- **`hasMaps` n'est jamais remis à faux** (`:239-247`) : le `warning` peut rester caché
  alors qu'il n'y a plus de carte.
- **Le troisième canal de la boîte est du bleu** : `setColor(r, g, 33)` (`:543`) passe par
  `Box.setColor(int, int, int)` — 33 n'est pas un alpha.
- **L'horloge de l'onglet est de 21 images**, pas 20 (`tickToInventoryUpdate++ == 20`), et
  un clic la pose à `-60`, soit **81 images** avant le prochain rafraîchissement.

## 4. Écarts assumés, avec le Java en face

| Endroit | Java | Ici | Pourquoi |
| --- | --- | --- | --- |
| libellé de priorité | `prio == 0` → « never », sinon `prio/10` | `Priority::text` fait exactement ça ; `ui::field::priority_text` répond « never » pour **tout** négatif | deux règles différentes ; celle de la fenêtre garde `== 0`, et un test montre les deux |
| `pct_full` d'une maison de niveau 0 | `0/0.0` → `NaN` | `1.0` (pleine) | le seul appelant, `capacity_colour`, exclut déjà ce cas (`residents < level` est faux) |
| `scale_label` | `DecimalFormat("##")`, arrondi **au pair** | `round_ties_even` | même arrondi que `DecimalFormat`, donc `1/2 = 0,5` s'écrit `0x` |
| `effect` de la croix hors salle des fêtes | `close()` | `exit_effect(false) == None` | `Effect` n'a pas de variante « fermer » (`ui_window_06/MANQUANT.md` §2) |
| refus de `IN_DELIVERY` | aucun nom (branche `default`) | `ResourceNotNeeded` | Java ne dit rien ; c'est le nom de la branche qu'il prend |
| ids `map.colony` / `map.prestige` | n'existent pas | deux ids de cette table | `Effect::Opens` dit « un écran de `src/ui` » ; `crate::ui` les refuse par leur nom, et un test le prouve |

## 5. Vérification

**`port-check ui_window_11` est ROUGE sur le dépôt, et aucune erreur ne cite
`src/ui_window_11/`.** Les erreurs restantes sont dans trois dossiers d'autres agents :
`src/ai_work_10/{rabbit,shepherd}.rs` (`super::state::HerdAct` introuvable),
`src/ai_work_11/{mod,tests}.rs` (`tests` déclaré deux fois, `TradeState` introuvable),
`src/ui_window_13/{mod,quest_log}.rs` (`page` introuvable, format string).

Comme ces erreurs de résolution bloquent la vérification de **tout** le crate, j'ai monté
une copie de travail dans `/tmp/uic11` (hors du dépôt, donc hors des dossiers des autres),
y ai remplacé par un stub les seuls modules qui ne compilent pas et que ce module
n'utilise pas, puis compilé **sous le même `flock`** que `port-check` (jamais deux
compilations en parallèle). Résultat :

```
test result: ok. 51 passed; 0 failed; 0 ignored; 2754 filtered out
```

La copie a été supprimée (`rm -rf /tmp/uic11`).

### Un blocage qui n'est pas dans mon dossier

`src/ui_window_06/resources.rs:262` appelle `right.rank()` / `left.rank()` sur un `Row` :
`rank` est une méthode d'`Availability`, pas de `Row`. Le compilateur répond
`E0599: no method named 'rank' found for reference '&ui_window_06::resources::Row'`.
Ce module **dépend** de `ui_window_06::resources` (règle 6 : ne pas dupliquer
`Availability`), donc ce fichier doit compiler pour que le mien compile. Je ne l'ai pas
touché — c'est un dossier d'un autre agent. Le correctif tient en une ligne :
`right.status().rank().cmp(&left.status().rank())`.

## 6. Ce que je n'ai pas fait

Les neuf manques sont dans `MANQUANT.md` : vue de bâtiment cliente (donc pas de
`getMapDataList`, pas de `getRange`), `Effect` qui sache fermer, source pour l'état que la
fenêtre tient, widget de liste et de carte, gestionnaire de requêtes client, inventaire de
joueur et drapeau créatif, `getDisplayName()` d'un ordre de travail, id d'écran hors de
`src/ui`. Aucune API n'a été inventée : les six classes de message sont **nommées**, jamais
envoyées, et aucune fonction de ce module ne lit une donnée qui n'existe pas dans le port.
