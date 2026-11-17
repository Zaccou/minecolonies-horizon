# AUDIT DE FIDÉLITÉ — `src/client_gui` contre `core/client/gui`

Échantillon de **53 méthodes** réparties sur **23 fichiers Java** (dont les sept de
`api/inventory/container/`), choisies pour porter une règle : borne, ordre, refus, durée,
formule. Le tableau ci-dessous a 56 lignes : deux sont des constats d'absence assumée et une
ne compare qu'une constante. Chaque ligne a été relue dans le Java à la ligne citée, puis
comparée au Rust.

**7 divergences réelles, toutes corrigées dans ce dossier** (aucune ailleurs, aucune
compilation, aucun `git`).

## Le tableau

`fidèle` = même règle, même borne, même ordre. `partiel` = la règle est là mais un morceau
manque ou est faux. `absent` = rien dans le Rust.

### `api/inventory/container/` → `containers.rs`

| # | Java | Rust | Verdict |
| --- | --- | --- | --- |
| 1 | `ContainerRack.java:98` `inventorySize = slots / 9` | `owned_rows` `containers.rs:288` | fidèle |
| 2 | `ContainerRack.java:101` `columns` (divise par `INVENTORY_BAR_SIZE`) | `display_columns` `:300` | fidèle |
| 3 | `ContainerRack.java:102` `extraOffset` (0 ou 2) | `extra_offset` `:310` | fidèle |
| 4 | `ContainerCitizenInventory.java:149` `extraOffset + 1` | `citizen_extra_offset` `:321` | fidèle |
| 5 | `ContainerRack.java:105-118` boucle `min(rows,8) × columns`, bornée par `index < size` | `owned_slots_added` `:335` | **divergent → corrigé (D1)** |
| 6 | `ContainerRack.java:123-144` 27 + 9 cases de joueur | `total_slots` `:343` | fidèle |
| 7 | `ContainerCitizenInventory.java:193-196` puis `:201-219` (armure **avant** le joueur) | `ARMOUR_SLOTS` `:356`, `tail_slots` `:364` | **partiel → corrigé (D2)** |
| 8 | `ContainerRack.java:168-190` `quickMoveStack` (`maxIndex` `:179`, plages `:183`/`:188`) | `quick_move` `:389` | fidèle (après D1) |
| 9 | `ContainerCrafting.java:309-315` sortie `index <= total_crafting_slots` | `crafted_quick_move` `:425` | fidèle |
| 10 | `ContainerCraftingFurnace.java:298-303` idem, `index <= FURNACE_SLOTS` | `quick_move` + `TransferRule::FurnaceLocked` `:389` | fidèle |
| 11 | `ContainerCraftingBrewingstand.java:324-353` + `setContainer` `:286-291` | `brewing_shift` `:456` | fidèle |
| 12 | `ContainerCrafting.java:238-241` `canSwitchRecipes` | — | absent, **délégué** : `ui_window_09::crafting::State::switchable` |
| 13 | `ContainerRack.java:149-166` `clicked` (mise à jour du stockage) | — | absent : pas d'inventaire dans le port (`MANQUANT.md` §6) |

### `core/client/gui/map/` → `map_data.rs`

| # | Java | Rust | Verdict |
| --- | --- | --- | --- |
| 14 | `ZoomDragMap.java:82-84` `calcRelativeX` | `relative` `map_data.rs:227` | fidèle |
| 15 | `ZoomDragMap.java:57-61` `childIsVisible` (4 bornes) | `visible` `:237` | fidèle |
| 16 | `ZoomDragMap.java:200-210` `scrollInput` (ancre, `/` ou `×`, clamp) | `zoom_anchor` `:185`, `zoom_scale` `:176`, `zoom` `:198` | **partiel → documenté (D6)** |
| 17 | `ZoomDragMap.java:191-193` `scrollX - x * dragFactor` | `drag` `:217` | fidèle |
| 18 | `ZoomDragMap.java:149`, `:166` clamp du **dessin** à `0.26219988382999904` | `render_scale` `:168` | fidèle |
| 19 | `MinecraftMap.java:51-56` 128×128, `x + y * 128` | `Map::index` `:138`, `Map::pixel` `:147` | fidèle |
| 20 | `MinecraftMap.java:59` `register("minecolonies_map/" + id)` | `Map::texture_name` `:128` | fidèle (le refus d'un id vide est un ajout assumé, voir plus bas) |
| 21 | `WindowColonyMap.java:262` `512 * currentScale` | `Map::tile_size` `:157` | fidèle |

### `core/client/gui/modules/building/` → `modules.rs`

| # | Java | Rust | Verdict |
| --- | --- | --- | --- |
| 22 | `ItemListModuleWindow.java:138-141` filtre `descriptionId` **ou** `hoverName` | `item_list_matches` `modules.rs:597` | fidèle |
| 23 | `ItemListModuleWindow.java:157-176` `allowed first`, inversé | `allowed_first` `:624` | fidèle |
| 24 | `ItemListModuleWindow.java:215-222` libellé ON/OFF | `switch_label` `:637` | fidèle |
| 25 | `ItemListModuleWindow.java:107-119` `add = (on && inv) \|\| (!on && !inv)` | `switch_action` `:662` | fidèle |
| 26 | `ItemListModuleWindow.java:93-99` + `:80` relance 10 ticks après la frappe | `FILTER_DEBOUNCE` `:140` | **partiel** : constante exacte, compte à rebours non modélisé (3 lignes de boucle UI, aucune autre règle) |
| 27 | `EntityListModuleWindow.java:141` filtre nom d'affichage **ou** `res.toString()` | `entity_list_matches` `:610` | fidèle |
| 28 | `SettingsModuleWindow.java:81-109` recyclage par panneau `id` | `setting_row` `:450` | **divergent → corrigé (D3)** |
| 29 | `ToolModuleWindow.java:30-31` `tooldesc.` + chemin de l'item | `tool_desc_key` `:421` | fidèle |
| 30 | `UniversityModuleWindow.java:54-62` branche visible si exigences vides ou non cachée | `university_branch_visible` `:502` | fidèle |
| 31 | `UniversityModuleWindow.java:74-109` `locked` / `Or` / prérequis rouge-cyan | `university_lines` `:511` | fidèle |
| 32 | `WarehouseOptionsModuleWindow.java:111-120` statut forcé + « x of z » | `warehouse_availability` `:547`, `Progress::x_of_z` | fidèle |
| 33 | `WarehouseOptionsModuleWindow.java:124-175` bouton, manquant seulement négatif, `amount - available` | `warehouse_button` `:562`, `missing_from_player_text` `:578`, `quantity_missing` `:568` | fidèle |
| 34 | `WarehouseOptionsModuleWindow.java:68`, `:201` tri au niveau 3 | `sorting_available` `:588` | fidèle |
| 35 | `RestaurantMenuModuleWindow.java:319` `(moyenne × 10 × clients) / Σ saturation` | `restaurant_consumption` `:893` | fidèle |
| 36 | `RestaurantMenuModuleWindow.java:555-571` limite **puis** réactivation | `restaurant_switch` `:915` | fidèle |
| 37 | `FarmFieldsModuleWindow.java:184-191` distance au carré + direction verticale sans distance | `field_distance_argument` `:722`, `field_line` `:684` | fidèle |
| 38 | `FarmFieldsModuleWindow.java:127-135` clic = inverse de l'état | `field_click` `:708` | **absent → ajouté (D7)** |
| 39 | `WorkOrderModuleWindow.java:142-146` groupe **puis** `WORK_ORDER_COMPARATOR` | `work_order_compare` `:775` | **partiel → corrigé (D4)** |
| 40 | `WorkOrderModuleWindow.java:197-213` la dernière raison gagne | `work_order_refusal` `:817` | fidèle |
| 41 | `WorkOrderModuleWindow.java:217-228` « cancel » dès que la hutte le tient | `work_order_button` `:837` | fidèle |
| 42 | `WorkOrderModuleWindow.java:120` `isInRange` = `canBuildIgnoringDistance` | `work_order_visible` `:794` | **partiel → documenté (D5)** |
| 43 | `ConnectionModuleWindow.java:110-114` coût `dist / 125` | `gate_cost` `:847` | fidèle |
| 44 | `ConnectionModuleWindow.java:163-178` bouton porte (alliance + origine) | `gate_button` `:878` | fidèle |
| 45 | `SpecialAssignmentModuleWindow.java:88-117` une ligne par citoyen de chaque module | `assignment_rows` `:934`, `assignment_text` `:946` | fidèle |

### `core/client/gui/citizen/` → `citizen.rs`

| # | Java | Rust | Verdict |
| --- | --- | --- | --- |
| 46 | `AbstractWindowCitizen.java:41-87` cinq onglets + débogage + métier conditionnel | `tabs` `citizen.rs:202` | fidèle (après la correction de test du tour précédent) |
| 47 | `CitizenWindowUtils.java:147-206` barre de vie, 4 paliers, 10 icônes | `hearts` `:245` | fidèle |
| 48 | `CitizenWindowUtils.java:257-299` saturation, 10 icônes de 6 points, moitié | `saturation_icons` `:282` | fidèle |
| 49 | `CitizenWindowUtils.java:302-355` bonheur `(int) × 2`, palier rouge | `happiness_icons` `:294`, `happiness_bar_value` `:306` | fidèle |
| 50 | `CitizenWindowUtils.java:385-405` panneau = métier en minuscules, boutons en créatif | `skill_pane_id` `:312`, `skill_buttons_pane_id` `:318`, `skills_editable` `:324` | fidèle |
| 51 | `FamilyWindowCitizen.java:40-44` parent vide → clé « unknown » | `parent` `:343` | fidèle |
| 52 | `FamilyWindowCitizen.java:46-57` conjoint absent → `-` | `partner_line` `:355` | fidèle |
| 53 | `RequestWindowCitizen.java:121-146` jetons du citoyen puis ceux de `-1` | `request_tokens` `:373` | fidèle |
| 54 | `RequestWindowCitizen.java:204-215` jamais les 5 cases équipées | `fulfil_slot` `:390` | fidèle |
| 55 | `RequestWindowCitizen.java:234` `créatif ? amount : min(amount, count)` | `fulfil_quantity` `:397` | fidèle |
| 56 | `RequestWindowCitizen.java:156-176` quatre conditions de « fulfillable » | `fulfillable` `:410` | fidèle |

Comptage : 56 lignes de tableau. Deux sont des constats d'absence assumée (12, 13) et une
ne compare qu'une constante (26) ; les **53 autres** comparent une méthode Java à une
fonction du port, et **7 d'entre elles** ont demandé une correction (5, 7, 28, 38, 39, 42,
16).

## Divergences trouvées et corrigées

### D1 — la borne `slots.size()` comptait des cases non posées (réelle)

`ContainerRack.java:105-118` n'ajoute que `min(rows, 8) × columns` cases, et s'arrête à
`index < size`. La frontière de `quickMoveStack` est `inventorySize * 9` (`:179`), mais la
borne haute des destinations est `this.slots.size()` (`:183`) : **les deux ne coïncident
que si l'inventaire du bloc est un multiple de neuf**. Mon `tail_slots` prenait `slots`
brut, donc pour 10 cases il rendait 46 au lieu de 45, et `quick_move(…, 45)` était accepté
là où Java lève.

Corrigé : `owned_slots_added` (`containers.rs:335`) + `tail_slots` (`:364`) + test
`the_slots_actually_added_stop_at_the_last_full_row`.

### D2 — l'armure d'un citoyen est **avant** l'inventaire du joueur (réelle, doc)

`ContainerCitizenInventory.java:193-196` pose les quatre cases d'armure, puis
`:201-219` les deux boucles du joueur. Ma doc disait « après l'inventaire du joueur
(`:231` ») : faux, et l'ordre compte, car la recherche d'une destination part de la fin
(`ContainerRack.java:183`) — barre rapide, joueur, **puis** armure. La plage
`[maxIndex, slots.size())` était juste, le commentaire et le test ne l'étaient pas.

Corrigé : doc de `ARMOUR_SLOTS` / `tail_slots`, ligne du tableau d'en-tête, et test renommé
`…_puts_its_armour_between_itself_and_the_player`.

### D3 — sans panneau `id`, Java ne recycle **jamais** (réelle, sémantique)

`SettingsModuleWindow.java:83-86` ne vide la ligne que si le panneau `id` existe **et**
porte un autre identifiant. Sans `id`, la ligne n'est jamais vidée : vide elle est posée une
fois (`:88-90`), pleine elle garde les widgets du **premier** réglage et `render` écrit
dedans (`:109`). Mon `Row::NoIdentityPane` disait « reconstruit à chaque rafraîchissement »,
ce qui est l'inverse.

Corrigé : variante `Row::Stale`, `setting_row` (`modules.rs:450`) distingue la première
pose (`Relayout`) de la réutilisation périmée (`Stale`), et le test couvre les deux.

### D4 — le tri des ordres a **deux** clés, pas une (réelle, manquante)

`WorkOrderModuleWindow.java:142-146` :
`comparingInt(getWorkOrderGroup).thenComparing(IWorkOrder.WORK_ORDER_COMPARATOR)`. Ma doc
affirmait que l'ordre de l'énumération « est l'ordre de tri », et le second critère
manquait — un ordre courant de priorité 1 serait passé après un ordre libre de priorité 5
si un appelant triait par groupe seul.

Corrigé : `work_order_compare` (`modules.rs:775`) compose le groupe avec
`colony::ordre::compare` (déjà porté, `src/colony/ordre.rs:257`), + test
`the_work_order_list_sorts_by_group_then_by_the_orders_own_comparator`.

### D5 — `isInRange` ne parle pas de distance (doc)

`WorkOrderModuleWindow.java:120` : la variable s'appelle `isInRange` mais vaut
`wo.canBuildIgnoringDistance(position, level)`. Corrigé dans la doc de
`work_order_visible` (`modules.rs:794`) : le paramètre garde le nom Java, et la doc dit ce
qu'il fait.

### D6 — le zoom traite un axe à la fois (doc)

`ZoomDragMap.java:203-210` fait la même formule pour X et pour Y (`childY` `:204`,
`oldY` `:206`, `setScrollY` `:210`). `zoom` (`map_data.rs:198`) est axe-agnostique : la doc
le dit maintenant, avec les trois lignes.

### D7 — le clic sur un champ manquait (absente)

`FarmFieldsModuleWindow.java:127-135` : champ pris → `freeField`, libre → `assignField`,
sans autre condition. Ajouté : `FieldClick` + `field_click` (`modules.rs:708`) + test.

## Ce que je n'ai pas pu vérifier, ou que j'ai volontairement écarté

1. **`ContainerRack.clicked`** (`:149-166`) : il resynchronise le stockage et l'entrepôt
   quand une pile change. Il n'y a pas d'inventaire dans le port — c'est la même raison que
   `MANQUANT.md` §6 (aucune route client → serveur, aucun inventaire côté port).
2. **`ContainerCrafting`** : `canSwitchRecipes` (`:238-241`) est déjà porté par
   `ui_window_09::crafting::State::switchable`, vérifié ; `clicked` / `canTakeItemForPickAll`
   (`:363-366`) et `slotsChanged` (`:193-232`) sont du serveur (choix de recette), hors de
   cette tranche.
3. **`MinecraftMap.freeTexture` / `close`** : cycle de vie d'une texture `DynamicTexture`.
   Le SDK n'expose pas de handle de texture (`MANQUANT.md` §7).
4. **`ZoomDragMap.getScale`** (`:226`) : simple getter, lu par `WindowColonyMap:337`.
   Rien à porter — l'échelle est une entrée de mes fonctions.
5. **`Map::new` refuse un id de panneau vide** (`map_data.rs:117`) : c'est une **divergence
   assumée** — Java enregistrerait la texture `"minecolonies_map/"`, ce qui n'est pas un
   nom. Le refus est nommé (`map.refusal.nameless_pane`) plutôt que silencieux.
6. **La composition visuelle des barres** (quelle texture de cœur à quelle position,
   `CitizenWindowUtils.java:208-244`, `:358-372`) : c'est du dessin, hors périmètre.
7. **`avgCustomerConsumption`** (`RestaurantMenuModuleWindow.java:131-141`) est calculé
   dans le constructeur (`sum / customers.size()`, `NaN` si aucun client) : mon
   `restaurant_consumption` le reçoit en argument et refuse l'infini — le cas « zéro
   client » reste donc à la charge de l'appelant, ce qui est dit dans la doc.
8. **Le texte exact des libellés** : les clés sont citées, pas les traductions (le port n'a
   pas de moteur de langue) ; les deux exceptions rendues en clair (`"x of z"`,
   `"N m"`) sont tirées de `lang/manual_en_us.json`, cité en commentaire.

## État

- 92 tests dans `src/client_gui` (3 ajoutés par cet audit), fichiers équilibrés, aucun
  `unwrap()`, aucun appel à l'hôte.
- Aucune compilation lancée, aucun `git`, aucune écriture hors de
  `src/client_gui/`.
