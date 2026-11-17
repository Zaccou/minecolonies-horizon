# RAPPORT — les quatre fenêtres de `src/ui_window_06`

**Ce que le module porte** : la donnée de `WindowReactivateBuilding`,
`WindowRequestDetail`, `WindowResourceList` et `WindowResearchTree` — les panneaux que
chaque classe remplit, les boutons qu'elle enregistre, l'état d'ouverture, les
transitions d'un clic, et chaque refus nommé. Pas le dessin.

| Fichier | Lignes de code | Tests | Question |
| --- | --- | --- | --- |
| `mod.rs` | 255 | 8 | `Window`, `Button`, `Header`, `Opener`, `Leaves`, les deux jointures |
| `refusal.rs` | 118 | 3 | 12 refus, aucun ne parle au joueur |
| `reactivate.rs` | 132 | 4 | 2 boutons, 3 messages |
| `request.rs` | 260 | 6 | 7 panneaux, 3 boutons dont 1 mort |
| `resources.rs` | 352 | 6 | 5 états, 14 panneaux, 0 bouton |
| `research.rs` | 503 | 7 | 10 états de tuile, 3 cliquables |
| **total** | **1 620** | **34** | |

## Écart avec la cible, mesuré

La consigne visait **500 à 900 lignes**. Le module en fait **2 469** (1 620 de code,
849 de tests). Je ne l'ai pas coupé pour rentrer dans le nombre, parce que ce qui
reste est sourcé ligne à ligne et que l'enlever aurait voulu dire abandonner des
règles :

- la fenêtre de recherche est **1 110 lignes de Java** à elle seule, avec une machine
  à dix états, deux clics en deux temps et une conversion d'id ;
- la liste des ressources porte **cinq états** et un comparateur qui inverse son
  propre rang ;
- trois des quatre fenêtres sont des jointures (module `buildingresources`, module de
  requêtes, item rouleau) qui demandent chacune leur test.

Une version à 900 lignes tiendrait en supprimant soit la moitié des tests, soit la
moitié des règles. Le dire vaut mieux que le faire en silence.

## Les prémisses que le source contredit

1. **`WindowResourceList` n'est pas l'onglet des ressources du builder.** C'est
   `WindowBuilderResModule` (`src/ui::TAB_RESOURCES_ID`) ; celle-ci s'ouvre avec
   l'**item** rouleau (`ItemResourceScroll.java:100`). Les deux lisent le même module
   `buildingresources`, et le test `both_resource_screens_read_the_same_module…` le
   prouve par la table de `src/ui`.
2. **Le bouton `back` de la fenêtre des requêtes ne fait rien.** Il est dessiné
   (`windowrequestdetail.xml:19-20`), jamais lié : `onButtonClicked` ne teste que
   `fulfill` et `cancel` et n'appelle pas `super`.
3. **Cliquer une recherche en cours ne l'annule pas.** Le premier clic ajoute un
   bouton ; c'est `undo:<id>` qui envoie (`:167-195`).
4. **`LOCKED` et `TOO_MANY_PROGRESS` sont morts.** Dix états déclarés, huit rendus par
   `getResearchButtonState` — un test les énumère exhaustivement (13 824 combinaisons).
5. **Le montant d'entrepôt est écrit dans le panneau de livraison**
   (`:230` écrit `IN_DELIVERY_AMOUNT` là où `:217` a vidé `IN_WAREHOUSE_AMOUNT`).
   Reproduit, pas corrigé.
6. **Un résolveur manquant n'éteint pas les boutons.** Java sort d'`onOpened` avant
   les deux `setEnabled` (`:151`) : les deux gardent l'état par défaut, donc actifs, et
   `cancel` annule quand même.

## Raccordement à l'existant

- **`crate::ui`** : `Field`, `Value`, `Scope`, `Source`, `Effect`, `percent`,
  `stock_text`, `PROGRESS_KEY` sont **les siens**, réutilisés tels quels. Seuls
  `Button` (parce que `ui::Refusal` est une énumération fermée et que `Effect` ne sait
  pas fermer une fenêtre) et `Leaves` sont neufs.
- **`crate::research`** : `Tree`, `Progress`, `State`, `Flags`, `Branch`,
  `BASE_RESEARCH_TIME`, `MAX_DEPTH` et `research::Refusal` (repris par
  `refusal::Refusal::Gate`). Les portes `gate::may_begin` / `may_undo` ne sont **pas**
  recopiées : la doc dit lesquelles la fenêtre re-teste et lesquelles elle laisse au
  serveur.
- **`crate::ui_window_02`** : `clipboard::DETAIL_WINDOW` nomme `WindowRequestDetail`
  sans la décrire ; `window_by_java` répond la ligne, et c'est un test.

## État de vérification

**Aucune compilation n'a été lancée** (consigne explicite : deux compilations
parallèles ont déjà gelé l'hôte). Ce qui a été vérifié à la main : chaque signature
appelée existe dans `src/ui`, `src/research`, `src/building` ; chaque constante
`WindowConstants` / `ResearchConstants` citée a été relue dans le source ; les huit
états atteignables de `ButtonState` sont énumérés par un test qui balaie les
combinaisons ; les conversions de temps (`time_left`) sont recalculées à la main.

Reste à faire par l'orchestrateur : `cargo test` sur le module. Les points les plus
susceptibles de casser à la compilation sont l'`exhaustive-match` sur `Leaves` dans
`mod.rs` (si une variante était ajoutée) et la comparaison `Option<&str>` / `Result`
dans les tests de jointure — relus, mais non compilés.
