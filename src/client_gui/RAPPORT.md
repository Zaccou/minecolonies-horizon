# RAPPORT — les écrans client : données et état (pas le dessin)

Quatre fichiers, **89 tests**, aucun appel à l'hôte. `src/client_gui` ne touche ni `lib.rs`,
ni `Cargo.toml`, ni `data/`, ni `assets/`, ni un autre module.

| Fichier | Lignes | Tests | Ce qu'il porte |
| --- | --- | --- | --- |
| `containers.rs` | ~660 | 19 | les 7 menus lisibles : cases, lignes, `quickMoveStack` |
| `map_data.rs` | ~420 | 14 | l'image de carte (128×128) et l'arithmétique de zoom/glisser |
| `modules.rs` | ~1160 | 29 | 15 fenêtres de module : niveau, progression, refus, filtres, ordres |
| `citizen.rs` | ~640 | 19 | les 7 fenêtres d'un citoyen : onglets, 3 barres, compétences, famille, requêtes |
| `mod.rs` | ~220 | 8 | la porte du module, les comptes, et 4 tests de couture |

## Les prémisses du brief qui ne tiennent pas

1. **« `containers/` : 8 fichiers »** — vrai de nom. `api/inventory/container/ContainerField.java`
   fait **0 octet** dans ce checkout et **rien ne le référence** (`grep -rn ContainerField`
   sur `src/main/java` : zéro occurrence ; `WindowField` n'importe aucun conteneur). Sept
   menus sont décrits, pas huit. `containers::EMPTY_CONTAINER_FILE` porte le trou.
2. **« les listes de townhall/citizen/questlog »** — **déjà écrites** par d'autres tranches.
   La page citoyens est `ui_window_15::citizen` (filtre, tri, sélection, 4 visages, `/10` et
   `/20` en dur, contradiction de l'icône d'entité) ; le journal de quêtes est
   `ui_window_13::quest_log` (3 pages, 5 panneaux, infobulle mesurée, suivi 30 s). J'ai
   écrit un `lists.rs` complet avant de m'en apercevoir, puis je l'ai **supprimé** : ses
   336 lignes ne portaient que du déjà-écrit. Ce qui reste est un test de couture
   (`mod.rs`) qui vérifie les deux modules et l'incohérence `/20 ≠ 60 ≠ 10 icônes`.
3. **« les modules : l'état d'un module »** — Java n'a pas ce type. Le niveau est celui du
   module ou du bâtiment selon la ligne lue, la progression est écrite à la main, et le
   refus est un `disable()` ou une couleur. `modules::State` est donc **trois champs
   séparés**, et `modules::Refusal` nomme des refus que Java ne nomme pas.
4. **« un module = une fenêtre »** — 24 fenêtres pour **157 modules** (`BuildingModules`).
   Les fenêtres sont génériques ; `modules::WINDOWS` joint sur la **classe de vue**, et
   deux fenêtres ne l'ont même pas en dur : `SpecialAssignmentModuleWindow` monte
   `layoutcourierassignment.xml` ou `layoutminerassignment.xml` selon la vue
   (`CourierAssignmentModuleView.java:125`, `MinerAssignmentModuleView.java:124`).

## Ce que la source dit et qui surprend (gardé, testé, jamais « corrigé »)

- **`ContainerCrafting.quickMoveStack` sort avant d'agir** : `index <= total_crafting_slots`
  (`:311-315`) couvre le résultat **et** la grille, donc la branche `if (index == 0)` de
  `:325-332` est **inatteignable**. Idem au four : `index <= FURNACE_SLOTS` (`:300-303`)
  rend le maj-clic sur l'entrée impossible et `:311-318` mort.
- **Le maj-clic de l'alambic est destructeur et inversé** : `index < 3` vide les **trois
  potions** (`:330-334`, `:286-291`), et la case 0 de l'écran n'est pas une potion mais
  **l'ingrédient** (`:176` contre `:178-180`). `containers::brewing_shift` nomme la branche.
- **La colonne d'un inventaire de bloc divise par 8 et non par 9** (`ContainerRack.java:101`,
  recopié dans les trois autres) : au-delà de huit lignes, `slots / 8 + 1` colonnes pour
  huit lignes dessinées.
- **La barre de vie change de couleur avec la vie** : le rouge remplit jusqu'à 20, l'or mord
  à 21 (par sa moitié) puis remplit de 22 à 41, le vert de 42 à 61, le bleu dès 62. Une
  barre **pleine** à 40 points est donc dorée, et à 19 points le libellé écrit `9` pour
  **dix** icônes (neuf pleines + une moitié).
- **Trois échelles de saturation** : `/20` en dur dans l'hôtel de ville, `60` dans le
  modèle, `10` icônes de 6 points dans la barre du citoyen.
- **L'entrepôt n'affiche une ressource manquante que si le compte est négatif**
  (`:164-172`), et sa ligne parle de **blocs d'émeraude** en dur (`:91`). Son bouton de tri
  s'arrête au niveau 3 (`:42`).
- **Le restaurant rallume le bouton que la limite vient d'éteindre** : `disable()` à
  `:557-560`, puis `if (isInMenu) … else enable()` à `:564-571` qui **écrase**. Et sa
  consommation divise par `saturationSum` sans garde (`:319`).
- **Un ordre de travail n'affiche que la dernière raison** : trois `if` successifs écrasent
  `disabledMessage` (`:200-216`), donc « ne peut pas construire » gagne sur « déjà pris »,
  qui gagne sur « pas d'ouvrier ».
- **La porte de colonie est muette pour un joueur solvable non allié** : le bouton s'éteint
  sans infobulle, et l'icône du prix s'affiche quand même (`:157-176`).
- **Deux planchers de zoom** : `0.02` pour le zoom, `0.26219988382999904` — un littéral sans
  nom — pour le dessin, donc entre les deux le dessin et la conversion des clics ne
  s'accordent plus.

## Ce que je n'ai pas dupliqué (règle 6)

`ui_window_15::citizen` (page citoyens), `ui_window_13::quest_log` (journal),
`ui_window_10::rack` (`RACK_SLOTS`, `MAX_STORAGE_UPGRADE` — **ré-exportés**, pas redéfinis),
`ui_window_08::brewing` (les quatre cases de l'alambic), `ui_window_11::map`
(`MAP_TILE`, `scale_label`), `ui::field::percent`, `ui::table::module_screen`,
`citizens::MAX_SATURATION`. Les tests de `mod.rs` sont les coutures.

## Ce qui n'est pas fait (voir `MANQUANT.md`)

Le huitième conteneur (fichier vide), un widget de liste, une mesure de largeur de texte,
une barre latérale d'onglets, un type d'état de module côté port, le registre de brassage,
la couleur d'un pixel de carte (vanilla), et l'envoi d'un message client → serveur.

## Les cinq comptes que les tests avaient faux (corrigés contre le Java)

Aucun n'était un défaut du modèle : dans les cinq cas, c'est l'attente du test qui
comptait mal.

| Test | Ce qui était faux | Preuve |
| --- | --- | --- |
| `the_always_tabs_are_five_…` | `tabs(true, false, false)` comprend **déjà** le métier : la liste nue est celle d'un citoyen **sans** bâtiment de travail | `AbstractWindowCitizen.java:70-87` |
| `the_inventory_tab_sends_…` | six onglets ouvrent une fenêtre, pas sept : l'inventaire **envoie** `OpenInventoryMessage` et n'en ouvre aucune | `AbstractWindowCitizen.java:49-51` |
| `a_container_without_cases_of_its_own_…` | `slots = 0` ne veut pas dire « rack sans extension » mais « aucune case possédée » : la cible est la plage vide `moveItemStackTo(…, 0, 0)`, donc aucun transfert — le modèle refuse maintenant par `Refusal::NoTarget` | `ContainerRack.java:179`, `:188-191` |
| `a_relative_conversion_…` | `(100 − 20 + 40) / 1 + 20 = 140`, pas 120 : le défilement s'ajoute **avant** la division | `ZoomDragMap.java:82-84` |
| `the_two_windows_that_take_their_xml_…` | quatorze fenêtres sur quinze ont un seul XML, pas treize | `ItemListModuleView.java:144`, `CourierAssignmentModuleView.java:125`, `MinerAssignmentModuleView.java:124` |

## Vérification

**Aucune compilation lancée**, comme demandé : l'orchestrateur compile et renvoie les
erreurs. Les 88 tests sont écrits pour ne dépendre que de `std` et de constantes du port.
Le volume dépasse la cible de 1 400–1 800 lignes (≈3 200 avec les tests) parce que la
tranche couvre quatre domaines Java et 44 fichiers source ; l'alternative aurait été de
retirer des sources `fichier.java:ligne`, ce que la règle 3 interdit.
