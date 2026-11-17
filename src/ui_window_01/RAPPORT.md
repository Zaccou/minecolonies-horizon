# RAPPORT — la donnée de `WindowAssignCitizen`, `WindowBannerPicker`, `WindowBannerRallyGuards`

Écrit uniquement dans `src/ui_window_01/`. Aucune compilation lancée (règle de la mission).
`lib.rs` déclare déjà `pub mod ui_window_01;` — je n'y ai pas touché.

## Ce qui a été écrit

| Fichier | Lignes | Rôle |
| --- | --- | --- |
| `mod.rs` | 369 | l'en-tête, `Button` / `Panel` / `Refusal` / `Emphasis` / `Afterwards`, la table des trois fenêtres, 3 tests |
| `constants.rs` | 248 | `WindowConstants` : les 24 constantes que ce lot lit, avec leur ligne ; les 6 ids privés de la fenêtre de ralliement ; 4 tests |
| `assign.rs` | 665 | `WindowAssignCitizen` : les deux lignes d'une rangée, le filtre, le tri, les quatre clics ; 11 tests |
| `banner.rs` | 594 | `WindowBannerPicker` : les couches, la table d'exclusion, le défilement, le verrou patreon ; 9 tests |
| `rally.rs` | 332 | `WindowBannerRallyGuards` : la liste des tours, les trois états d'une rangée, les deux boutons ; 6 tests |
| `MANQUANT.md` | 138 | les 10 manques, chacun nommé |

**2 208 lignes de Rust, 743 de commentaires portant les sources, 1 465 de code, 33 tests.** C'est
au-dessus des 500–900 visées : je n'ai pas trouvé quoi couper sans perdre soit une règle, soit sa
ligne Java, soit un test. Les en-têtes de module sont la partie la plus compressible si tu préfères.

## Raccordement à `src/ui` — ce que je n'ai pas décrit deux fois

`WindowAssignCitizen` **est déjà** un écran de `src/ui` (`SCREENS`, id `assign`, classe
`WindowAssignCitizen`). Je ne l'ai donc pas redécrit : `assign::screen()` rend le `Screen` de
`src/ui`, et `assign::ROW_PANES` ne fait que nommer les deux panneaux de rangée que `src/ui` déclare
déjà — un test vérifie que ces quatre panneaux existent bien dans `src/ui`, avec la bonne portée,
la bonne `Source` et le bon `Value` (`citizen` = `Name`, `job` = `Text`). Ce que j'ajoute et que
`src/ui` ne peut pas porter : **le contenu composé** du panneau `job`, le filtre et le tri de la
liste de gauche, et les transitions des clics.

Les deux fenêtres de bannière ne sont dans aucune table : `rally::PANEL` et `banner::Picker` sont
neufs, et un test prouve que `src/ui::screen_by_id` les refuse (`UnknownScreen`) — c'est la bonne
réponse, ce ne sont pas des écrans de hutte.

J'ai réutilisé `Refusal`, `may_assign`, `may_unassign`, `next_hiring_mode`, `HiringMode::slug`,
`Field`, `Value`, `Source`, `Title`, `Effect` et `Screen`. Aucune API de l'hôte, aucun `crates/`,
aucune dépendance.

## Prémisses fausses, dites plutôt que contournées

1. **« les champs affichés (nom, niveau, progression, stock) » ne décrit aucun de ces trois écrans.**
   - **niveau** : lu **une fois** pour filtrer la grille de motifs (`WindowBannerPicker.java:144-150`),
     jamais affiché ;
   - **progression** : il n'y en a pas. Ce qui y ressemble est une **distance** euclidienne
     (`BlockPosUtil.java:453-462`), comparée à un seuil de **300** blocs
     (`WindowAssignCitizen.java:39`) ;
   - **stock** : il n'y en a pas. La fenêtre de ralliement affiche un **nombre de gardes**
     (`WindowBannerRallyGuards.java:119-123`).
   Le quatrième écran de ce lot, `WindowBannerPicker`, **n'a ni panneau nommé ni XML** : il étend
   `net.minecraft.client.gui.screens.Screen` (`:49`) et construit ses boutons en objets anonymes.
   `Field`/`Screen` ne s'y appliquent pas — c'est pourquoi il est décrit par `Picker`, pas par un
   `Panel`.

2. **Les trois fenêtres nomment leurs panneaux de trois façons différentes.** `WindowAssignCitizen`
   importe `WindowConstants.*` (`:29`) ; `WindowBannerRallyGuards` n'importe **rien** de
   `WindowConstants` et déclare ses 6 ids en privé (`:36-61`), dont un `remove` qui a la même valeur
   que `WindowConstants.BUTTON_REMOVE` (`:584`) sans jamais le nommer ; `WindowBannerPicker` n'a
   aucun id.

3. **`Screen.gate` ne peut pas garder la fenêtre de ralliement.** `Gate` n'a qu'une variante, un
   avancement ; la précondition du ralliement est une **donnée** (liste non vide,
   `ItemBannerRallyGuards.java:220-226`), donc `rally::may_open` et un refus nommé.

4. **`Action` ne suffit pas à ces fenêtres.** `Action.effect` est un seul `Effect`, et quatre
   handlers en font deux (bannière : écrire le drapeau **et** rouvrir l'hôtel de ville, `:234-241` ;
   ralliement : envoyer **et** se fermer, `:164-168` ; mode : poser **et** faire envoyer par la vue,
   `LivingBuildingView.java:87-91`). Surtout, `Action.refusals` est
   `&'static [crate::ui::action::Refusal]`, une enum **fermée** où `NoSuchTower` n'existe pas et où
   je ne peux pas l'ajouter (dossier d'un autre agent). D'où `Button` = un `Effect` + un
   `Afterwards` + les refus de ce module, et `Panel` = la forme de `Screen` avec des `Button`.
   `Button::action()` rend l'`Action` équivalente quand le bouton n'a **aucun** refus propre, et un
   test le vérifie : les deux descriptions s'accordent partout où elles peuvent.

5. **`WindowAssignCitizen` n'a pas de refus à elle : elle emprunte ceux de `src/ui`.** `NoRoom`,
   `Travelling` et `HousingHandledElsewhere` viennent de `crate::ui::action`, pas d'une copie.

## Ce que Java fait et qu'un porteur pourrait ne pas voir

- **Un tooltip est posé puis effacé.** Ligne de la liste *assignée* :
  `setText(travelling)` à `WindowAssignCitizen.java:384`, puis `setText(Component.empty())`
  **inconditionnel** à `:391`. Le joueur ne voit jamais le tooltip « travelling » ; c'est le
  `disable()` qui refuse. Côté *non assigné*, le tooltip vide est posé **dans la branche permise**
  (`:311`), donc le `hire.warning` de `:316` survit. `hire_tooltip` et `fire_tooltip` gardent
  l'asymétrie ; un test l'exige.
- **Le seuil de 300 se mesure sur deux paires différentes.** Liste non assignée :
  `work ↔ home actuel` (`:270`) ; liste assignée : `work ↔ cette maison` (`:366`). Le **même**
  citoyen peut être rouge dans une liste et normal dans l'autre. Test dédié.
- **`hireClicked` revérifie le compte, `fireClicked` ne revérifie rien** (`:105-108` contre
  `:123-136`). Le port garde l'asymétrie.
- **Le compte de gardes n'est pas un mélange.** L'étiquette dit `knight|ranger: N` où `N` est
  **tous** les gardes de la tour (`:119-123`) — Java le sait et l'a écrit :
  `//todo we probably want to display the exact mix.` (`:112`).
- **Un `TODO` de Java est périmé.** `WindowBannerPicker.java:197` porte
  `// TODO: port this last vital condition` à côté de `activeLayer < layers.size() && activeLayer != 0` :
  la condition **est** écrite. Je suis le code, pas le commentaire.
- **La molette est avalée sur la couche de base.** `mouseScrolled` renvoie `true` même quand
  `activeLayer == 0` n'a rien déplacé (`:392-403`) ; `mouseClicked` fait pareil (`:416`).
- **La borne de défilement peut être négative.** `ceil(p/8) - 4 + 1` vaut `-2` pour 3 motifs ;
  ce que `Mth.clamp` fait d'une borne négative n'est pas dans ce dépôt. `scroll_bound` plancher à 0
  et **le dit** au lieu de prétendre connaître la réponse de Java.
- **Le drapeau par défaut contient déjà une couche.** `ColonyView.java:110-112` :
  `new BannerPattern.Builder().addPattern(BASE, WHITE).toListTag()`. Donc
  `createPatterns(WHITE, flag)` rend 2 couches, `layers.remove(0)` en laisse 1 (`:157-160`) : le
  sélecteur s'ouvre sur **une** couche BASE/BLANC, comme pour un drapeau vide mais par un autre
  chemin. Test dédié.
- **Le verrou patreon ne peut pas se déclencher aujourd'hui.** Il ne frappe que les motifs du
  namespace `minecolonies` (`:216`, `:348`) ; aucun `BannerPattern` n'est enregistré sous ce
  namespace dans ce dépôt (pas de `data/minecolonies/banner_pattern/`, aucune classe
  `BannerPattern` hors de `WindowBannerPicker`). La règle est portée, l'instance manque.
- **La couleur rouge de la tour disparue est `rgbaToInt(255, 0, 0, 1)`** (`:132`) : alpha **1**,
  pas 255. Le port garde l'intention (`Emphasis::Red`) et cite le littéral au lieu de l'emballer.
- **`WindowConstants` a des valeurs en double, et ça compte** : quatre champs valent `name`, trois
  valent `job`, deux `done`, deux `cancel`. Deux listes à un mot près : `assigned`/`unassigned`
  (fenêtre d'assignation, `:954`/`:949`) contre `assignedCitizen`/`unemployed`
  (maison et embauche, `:1100`/`:994`). Un test les compte.

## Une correction que je n'ai pas faite, parce que ce n'est pas mon dossier

`src/ui/screen.rs` déclare, pour l'écran `assign` :
`Field::new("buildingassignmentmode", Value::Text, Source::Module("living"))`.
Ce panneau a un id mais **aucune classe ne l'écrit** : c'est le libellé statique du bouton `mode`
(`windowassigncitizen.xml:32`, `label="$(com.minecolonies.coremod.gui.buildingassignmentmode)"`), et
le littéral n'apparaît nulle part dans l'arbre Java. C'est une **étiquette**, pas une valeur : la
bonne déclaration serait un `label` sur l'action `mode`, ou rien. Je l'ai rangé dans
`constants::ASSIGN_STATIC_LABELS` et un test vérifie que les huit **autres** panneaux de cet écran,
eux, sont bien des constantes de `WindowConstants`. À trancher côté `src/ui`.

## Un test corrigé après le premier `cargo test`, tranché contre le Java

`assign::tests::a_citizen_who_lives_at_work_or_already_here_is_not_listed` affirmait
`listable(Some(at(5)), Some(at(5)), at(6)) == true` — un citoyen dont la maison est **égale** à son
lieu de travail, avec `here` ailleurs. **C'était l'attente qui était fausse, pas `listable`.** Le
filtre de `WindowAssignCitizen.java:196-197` est
`(!Objects.equals(home, work) || home == null) && !here.equals(home)` : quand `home == work` et que
`home` n'est pas nul, le premier conjoint vaut `false`, et `here` n'entre pas dans cette moitié du
test. Le citoyen est donc masqué **où que soit sa maison**. L'assertion est devenue
`!listable(…, at(6))` (avec la ligne Java en commentaire), et le cas « maison ailleurs, travail
ailleurs, ni l'une ni l'autre ici » est couvert par `listable(Some(at(5)), Some(at(6)), at(7))`.
L'implémentation n'a pas bougé : elle était déjà celle du Java.

## Ce qu'il reste à faire côté orchestrateur

- Compiler et exécuter : `ui_window_01` n'appelle aucune API de l'hôte, les tests seuls suffisent.
- Si `clippy` passe : `needless_return` possible dans `banner::next_selected_colour`
  (le `return match` est volontaire, il sort du cas « au-delà de la dernière couche »).
- Décider du sort de `buildingassignmentmode` dans `src/ui`.
