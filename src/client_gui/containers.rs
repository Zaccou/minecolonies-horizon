//! Les huit conteneurs de MineColonies : **ou sont les cases, et ou va une pile quand on
//! maj-clique dessus**.
//!
//! Java reference: `api/inventory/container/` — `ContainerRack`, `ContainerBuildingInventory`,
//! `ContainerCitizenInventory`, `ContainerGrave`, `ContainerCrafting`,
//! `ContainerCraftingFurnace`, `ContainerCraftingBrewingstand`, `ContainerField`, et les
//! fenetres qui les montent (`core/client/gui/containers/Window*.java`).
//!
//! **Ce module ne decrit pas les fenetres.** Ce qu'elles demandent, leurs boutons et le
//! message que chaque clic envoie sont deja dans [`crate::ui_window_08`],
//! [`crate::ui_window_09`] et [`crate::ui_window_10`] ; les cases de l'alambic sont meme
//! deja dans `ui_window_08::brewing::SLOTS` ([`crate::ui_window_08::brewing`]), et ce
//! module les **cite** au lieu de les reecrire. Ce qui manquait est la couche que ces
//! trois modules laissent de cote parce qu'elle n'est pas dans la fenetre : les
//! **menus**, c'est-a-dire le nombre de cases, leur decoupage, et la regle de transfert
//! `quickMoveStack`.
//!
//! # Ce que le decoupage dit, et qui n'est pas uniforme
//!
//! Sept des huit menus suivent **la meme forme** : N lignes possedees (l'inventaire du
//! bloc, du citoyen, de la tombe), puis les 27 cases du joueur, puis sa barre rapide.
//! `ContainerRack.java:98-144` en est le modele, et `ContainerBuildingInventory.java:70-112`,
//! `ContainerGrave.java:68-110`, `ContainerCitizenInventory.java:145-218` le recopient.
//! Les trois autres sont des grilles d'artisanat et n'ont pas de lignes du tout.
//!
//! | Menu | Cases possedees | Regle de transfert |
//! | --- | --- | --- |
//! | `ContainerRack` | `slots / 9` lignes, deux blocs empiles possibles | generique |
//! | `ContainerBuildingInventory` | `slots / 9` lignes | generique |
//! | `ContainerCitizenInventory` | `slots / 9` lignes **+ 4 cases d'armure** apres | generique |
//! | `ContainerGrave` | `slots / 9` lignes | generique |
//! | `ContainerCrafting` | resultat + grille 2x2 ou 3x3 (`5` ou `10`) | grille verrouillee |
//! | `ContainerCraftingFurnace` | entree + sortie (`2`) | four verrouille |
//! | `ContainerCraftingBrewingstand` | ingredient + 3 potions (`4`) | alambic, sur mesure |
//! | `ContainerField` | **illisible** : le fichier fait 0 octet dans ce checkout | inconnu |
//!
//! # Trois choses que la source dit et que le bon sens refuse
//!
//! 1. **Le maj-clic sur le resultat d'artisanat ne fait rien, et Java ne s'en apercoit
//!    pas.** `ContainerCrafting.quickMoveStack` sort immediatement pour
//!    `index <= total_crafting_slots` (`ContainerCrafting.java:311-315`), soit `0..=5`
//!    ou `0..=10` : la branche `if (index == 0)` de `:325-332`, celle qui est censee
//!    sortir le resultat vers l'inventaire, est **inatteignable**. [`quick_move`] rend
//!    donc [`Refusal::GridLocked`] pour la case 0 aussi.
//! 2. **Le four est dans le meme cas, en plus court** : `index <= FURNACE_SLOTS`
//!    (`ContainerCraftingFurnace.java:300-303`) couvre les deux cases du four, donc le
//!    maj-clic ne peut jamais remplir l'entree. `:311-318` est du code mort.
//! 3. **Le maj-clic de l'alambic est destructeur et inverse.** `index < 3` appelle
//!    `setContainer(EMPTY)` (`ContainerCraftingBrewingstand.java:330-334`), qui **vide
//!    les trois cases de potion** (`:286-291`) ; or la case 0 de l'ecran n'est pas une
//!    potion mais **l'ingredient** (`ContainerCraftingBrewingstand.java:176` contre
//!    `:178-180`). Un maj-clic sur l'ingredient efface donc les potions en cours, sans
//!    rien transferer. [`brewing_shift`] nomme ce cas plutot que de le lisser.
//!
//! # Ce qui n'est pas ici
//!
//! Pas une seule coordonnee n'est portee : `PLAYER_INVENTORY_INITIAL_X_OFFSET` et ses
//! voisines existent pour que le jeu dessine, et `TRANCHE.md` §3 M2 dit deja que cette
//! tranche n'a pas de disposition de cases. Les quatre constantes de pixels sont
//! nommees quand meme, parce qu'elles disent **pourquoi** `INVENTORY_BAR_SIZE` vaut huit
//! et non neuf : la colonne de cases du bloc demarre a `BAR_SIZE` et non a l'origine, ce
//! qui est un decentrement, pas une taille. La difference est expliquee sur
//! [`display_columns`].

/// Le nombre de colonnes d'un inventaire de joueur (`InventoryConstants.java:11`).
pub const COLUMNS: usize = 9;

/// Le nombre de lignes de l'inventaire de joueur (`InventoryConstants.java:36`).
pub const ROWS: usize = 3;

/// `INVENTORY_BAR_SIZE` (`InventoryConstants.java:41`) : **huit**, et c'est le piege du
/// fichier. Le nom dit "taille de la barre rapide", la barre rapide a neuf cases, et la
/// constante sert de **decalage en pixels** pour la colonne de gauche (`:119`,
/// `PLAYER_INVENTORY_INITIAL_X_OFFSET` vaut huit lui aussi) **et** de diviseur dans
/// [`display_columns`]. Deux usages, deux sens.
pub const BAR_SIZE: usize = 8;

/// L'ecart horizontal entre deux cases (`InventoryConstants.java:26`).
pub const SLOT_PX: i32 = 18;

/// `PLAYER_INVENTORY_INITIAL_X_OFFSET` (`InventoryConstants.java:16`).
pub const PLAYER_X: i32 = 8;

/// `PLAYER_INVENTORY_INITIAL_Y_OFFSET` (`InventoryConstants.java:21`).
pub const PLAYER_Y: i32 = 30;

/// `PLAYER_INVENTORY_HOTBAR_OFFSET` (`InventoryConstants.java:31`).
pub const HOTBAR_Y: i32 = 88;

/// Cases de l'atelier sans la grille 3x3 : resultat + grille 2x2
/// (`InventoryConstants.java:91`).
pub const CRAFTING_GRID: usize = 5;

/// Cases du four (`InventoryConstants.java:96`).
pub const FURNACE_SLOTS: usize = 2;

/// Les cinq cases que la grille 3x3 ajoute (`InventoryConstants.java:101`).
pub const ADDITIONAL_GRID: usize = 5;

/// Premiere case de la barre rapide dans un atelier (`InventoryConstants.java:106`).
pub const HOTBAR_START: usize = 32;

/// Total d'un atelier 2x2 (`InventoryConstants.java:111`).
pub const TOTAL_SLOTS: usize = 41;

/// Total du four (`InventoryConstants.java:116`).
pub const TOTAL_SLOTS_FURNACE: usize = 38;

// La taille d'un rack (`DEFAULT_SIZE`, `Constants.java:143`) et ce qu'une extension
// ajoute (`SLOT_PER_LINE`, `:163`) **ne sont pas redefinies ici** :
// `ui_window_10::rack` porte les deux (`RACK_SLOTS`, `RACK_SLOTS_BY_UPGRADE`), et ce
// module s'y raccorde. Un rack n'est qu'un inventaire de bloc parmi les quatre que
// `quick_move` traite, et sa taille ne change pas sa regle de transfert.

/// La ligne que le coffre de champ aurait du fournir, et qui est vide.
///
/// `api/inventory/container/ContainerField.java` fait **0 octet** dans ce checkout : le
/// fichier existe, il n'a pas de contenu. Rien ne le reference non plus —
/// `core/client/gui/containers/WindowField.java` n'importe aucun conteneur et n'ouvre
/// qu'un ecran de champ. Le port garde le nom du fichier pour que le trou soit visible
/// au lieu de disparaitre.
pub const EMPTY_CONTAINER_FILE: &str = "api/inventory/container/ContainerField.java";

/// Ce qu'un menu possede avant l'inventaire du joueur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : trois formes de possession, et une quatrieme serait un nouveau menu
pub enum Inventory {
    /// `n` lignes de neuf, ou `n = slots / 9` (`ContainerRack.java:98`).
    Storage,
    /// `n` lignes, **plus** des cases ajoutees apres l'inventaire du joueur
    /// (`ContainerCitizenInventory.java:231` : quatre cases d'armure).
    StorageAndArmour,
    /// Une grille d'artisanat, qui ne se compte pas en lignes
    /// (`ContainerCrafting.java:126-155`, `ContainerCraftingFurnace.java:178-191`).
    Grid(usize),
}

/// La regle de transfert d'un menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les quatre regles que Java ecrit, et pas une de plus
pub enum TransferRule {
    /// `owned -> [owned, total)` en partant de la fin, et l'inverse en partant du debut
    /// (`ContainerRack.java:179-190`).
    Generic,
    /// Rien ne bouge depuis la grille : Java sort avant de regarder la case
    /// (`ContainerCrafting.java:311-315`).
    GridLocked,
    /// Idem, pour les deux cases du four (`ContainerCraftingFurnace.java:300-303`).
    FurnaceLocked,
    /// Regle sur mesure de l'alambic ([`brewing_shift`]).
    Brewing,
}

/// Un menu : sa fenetre, son conteneur, ce qu'il possede et comment il transfere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Menu {
    /// La fenetre cliente qui le monte.
    pub window: &'static str,
    /// La classe de menu, sous `api/inventory/container/`.
    pub container: &'static str,
    /// Ce que le menu possede en propre.
    pub inventory: Inventory,
    /// Sa regle de transfert.
    pub rule: TransferRule,
}

/// Les **sept** menus lisibles, avec la fenetre qui les monte.
///
/// `ContainerField` manque, et c'est le fichier vide (`EMPTY_CONTAINER_FILE`). Les paires
/// fenetre/conteneur sont celles des constructeurs : `WindowRack.java:68`,
/// `WindowBuildingInventory.java:44`, `WindowCitizenInventory.java:79`,
/// `WindowGrave.java:88`, `WindowCrafting.java:136`, `WindowFurnaceCrafting.java:97`,
/// `WindowBrewingstandCrafting.java:78`.
pub const MENUS: &[Menu] = &[
    Menu {
        window: "WindowRack",
        container: "ContainerRack",
        inventory: Inventory::Storage,
        rule: TransferRule::Generic,
    },
    Menu {
        window: "WindowBuildingInventory",
        container: "ContainerBuildingInventory",
        inventory: Inventory::Storage,
        rule: TransferRule::Generic,
    },
    Menu {
        window: "WindowCitizenInventory",
        container: "ContainerCitizenInventory",
        inventory: Inventory::StorageAndArmour,
        rule: TransferRule::Generic,
    },
    Menu {
        window: "WindowGrave",
        container: "ContainerGrave",
        inventory: Inventory::Storage,
        rule: TransferRule::Generic,
    },
    Menu {
        window: "WindowCrafting",
        container: "ContainerCrafting",
        inventory: Inventory::Grid(CRAFTING_GRID),
        rule: TransferRule::GridLocked,
    },
    Menu {
        window: "WindowFurnaceCrafting",
        container: "ContainerCraftingFurnace",
        inventory: Inventory::Grid(FURNACE_SLOTS),
        rule: TransferRule::FurnaceLocked,
    },
    Menu {
        window: "WindowBrewingstandCrafting",
        container: "ContainerCraftingBrewingstand",
        inventory: Inventory::Grid(4),
        rule: TransferRule::Brewing,
    },
];

/// Le menu de ce conteneur, par nom de classe.
pub fn menu(container: &str) -> Option<&'static Menu> {
    MENUS.iter().find(|menu| menu.container == container)
}

/// Pourquoi un transfert n'a rien fait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : ferme sur les cinq facons dont Java refuse un maj-clic
pub enum Refusal {
    /// La case demandee n'existe pas dans ce menu : `slots.get(index)` leve
    /// (`ContainerRack.java:170-172`).
    NoSuchSlot { index: usize, slots: usize },
    /// La case est vide : `!slot.hasItem()` (`ContainerRack.java:172-176`).
    NothingToMove,
    /// La case est dans une grille que Java n'a jamais l'intention de vider
    /// (`ContainerCrafting.java:311-315`, `ContainerCraftingFurnace.java:300-303`).
    GridLocked,
    /// L'alambic n'a pas de transfert par plages : sa regle est [`brewing_shift`], et un
    /// appelant qui demande une plage s'est trompe de fonction.
    BrewingRule,
    /// La plage de destination est vide : `moveItemStackTo(stack, 0, maxIndex, false)`
    /// avec `maxIndex == 0` (un conteneur sans case a lui, `ContainerRack.java:188`) ne
    /// fait aucune iteration et rend `false`, donc Java rend `EMPTY` (`:189-191`).
    NoTarget { from: usize, to: usize },
}

impl Refusal {
    /// Un nom stable, prefixes, qui ne peut pas etre confondu avec une phrase.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::NoSuchSlot { .. } => "container.refusal.no_such_slot",
            Refusal::NothingToMove => "container.refusal.empty_slot",
            Refusal::GridLocked => "container.refusal.grid_locked",
            Refusal::BrewingRule => "container.refusal.brewing_rule",
            Refusal::NoTarget { .. } => "container.refusal.no_target",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// Ou une pile s'en va.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : deux destinations, et une troisieme serait un nouveau menu
pub enum Transfer {
    /// Vers l'inventaire du joueur, en partant de la fin : `moveItemStackTo(stackCopy,
    /// maxIndex, slots.size(), true)` (`ContainerRack.java:183`).
    ToPlayer { from: usize, to: usize },
    /// Vers les cases possedees, en partant du debut : `moveItemStackTo(stackCopy, 0,
    /// maxIndex, false)` (`ContainerRack.java:188`).
    ToOwned { from: usize, to: usize },
}

impl Transfer {
    /// La plage visitee, dans l'ordre de recherche.
    pub fn range(self) -> (usize, usize) {
        match self {
            Transfer::ToPlayer { from, to } | Transfer::ToOwned { from, to } => (from, to),
        }
    }
}

/// Les lignes d'inventaire possedees, `slots / 9` (`ContainerRack.java:98`).
pub fn owned_rows(slots: usize) -> usize {
    slots / COLUMNS
}

/// La colonne de cases du bloc, telle que Java la calcule
/// (`ContainerRack.java:101`, recopie a l'identique dans les trois autres menus).
///
/// **Deux pieges, et ils sont dans la meme ligne.** Le diviseur est `INVENTORY_BAR_SIZE`
/// (huit) et non `INVENTORY_COLUMNS` (neuf), et la comparaison porte sur `<=` : jusqu'a
/// huit lignes la colonne a neuf cases, au-dela elle prend `slots / 8 + 1` cases — soit,
/// pour 90 cases, douze colonnes pour huit lignes affichees, donc une derniere ligne
/// partielle (la boucle de `:105-118` s'arrete a `index < size`).
pub fn display_columns(rows: usize, slots: usize) -> usize {
    if rows <= BAR_SIZE {
        COLUMNS
    } else {
        (slots / BAR_SIZE) + 1
    }
}

/// La marge verticale que Java ajoute quand l'inventaire du bloc depasse huit lignes
/// (`ContainerRack.java:102` : zero ou deux).
pub fn extra_offset(rows: usize) -> i32 {
    if rows <= BAR_SIZE {
        0
    } else {
        2
    }
}

/// La marge supplementaire de l'inventaire du citoyen, qui vaut un de plus que les autres
/// (`ContainerCitizenInventory.java:149-150`) : `+ 1` pour sa propre ligne, et un
/// `newOffset` de cinq.
pub fn citizen_extra_offset(rows: usize) -> i32 {
    extra_offset(rows) + 1
}

/// Le nombre total de cases d'un menu a inventaire de stockage : les cases possedees, les
/// 27 du joueur, les 9 de sa barre (`ContainerRack.java:123-144`).
pub fn total_slots(slots: usize) -> usize {
    slots + ROWS * COLUMNS + COLUMNS
}

/// Les quatre cases d'armure que `ContainerCitizenInventory` ajoute **apres** l'inventaire
/// du joueur (`ContainerCitizenInventory.java:231`, une par `EquipmentSlot`).
pub const ARMOUR_SLOTS: usize = 4;

/// La derniere case du menu, celle que `slots.size()` rend et que `moveItemStackTo`
/// utilise comme borne (`ContainerRack.java:183`).
///
/// **C'est la borne de Java, pas celle du joueur** : pour l'inventaire d'un citoyen elle
/// inclut les quatre cases d'armure, qui sont donc une destination de maj-clic. Pour une
/// grille d'artisanat, les cases possedees sont celles de la grille (`owned`) et non
/// `slots`, qui ne veut rien dire pour un menu sans inventaire de bloc.
pub fn tail_slots(inventory: Inventory, slots: usize) -> usize {
    let player = ROWS * COLUMNS + COLUMNS;
    match inventory {
        Inventory::Storage => total_slots(slots),
        Inventory::StorageAndArmour => total_slots(slots) + ARMOUR_SLOTS,
        Inventory::Grid(owned) => owned + player,
    }
}

/// La case du resultat d'artisanat est la premiere, et la grille suit
/// (`ContainerCrafting.java:126-155` : `craftResultSlot` d'abord, puis la matrice).
pub fn crafting_owned(complete: bool) -> usize {
    if complete {
        CRAFTING_GRID + ADDITIONAL_GRID
    } else {
        CRAFTING_GRID
    }
}

/// La regle de transfert, en une reponse : ou va la pile, ou pourquoi elle reste.
///
/// C'est `quickMoveStack` sans les piles : Java decide **d'abord** si un transfert est
/// possible (`ContainerRack.java:168-190`), et c'est cette decision que le port peut
/// tenir sans inventaire.
pub fn quick_move(menu: &Menu, index: usize, has_item: bool, slots: usize) -> Result<Transfer, Refusal> {
    let tail = tail_slots(menu.inventory, slots);
    if index >= tail {
        return Err(Refusal::NoSuchSlot { index, slots: tail });
    }
    match menu.rule {
        TransferRule::GridLocked => Err(Refusal::GridLocked),
        TransferRule::FurnaceLocked => Err(Refusal::GridLocked),
        TransferRule::Brewing => Err(Refusal::BrewingRule),
        TransferRule::Generic => {
            if !has_item {
                return Err(Refusal::NothingToMove);
            }
            let owned = owned_rows(slots) * COLUMNS;
            let (from, to) = if index < owned { (owned, tail) } else { (0, owned) };
            // Une plage vide, c'est `moveItemStackTo` qui ne boucle pas et rend `false`
            // (`ContainerRack.java:188-191`) : Java ne transfere rien.
            if from == to {
                return Err(Refusal::NoTarget { from, to });
            }
            if index < owned {
                Ok(Transfer::ToPlayer { from, to })
            } else {
                Ok(Transfer::ToOwned { from, to })
            }
        }
    }
}

/// La meme decision, mais **avec le compte de cases que Java utilise vraiment** : la
/// grille d'artisanat n'a pas d'inventaire du joueur dans sa partie possedee, elle a
/// - `owned` cases puis les 36 du joueur (`ContainerCrafting.java:167-186`).
///
/// Java sort avant de transferer, quelle que soit la case : cette fonction rend donc
/// toujours [`Refusal::GridLocked`], et c'est son interet — un appelant qui attendrait un
/// transfert doit lire ce refus, pas une plage vide.
pub fn crafted_quick_move(_index: usize) -> Result<Transfer, Refusal> {
    Err(Refusal::GridLocked)
}

/// Ce qu'un maj-clic fait dans l'alambic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les cinq branches de ContainerCraftingBrewingstand.java:330-351
pub enum BrewAction {
    /// `index < 3` : `setContainer(EMPTY)`, donc **les trois cases de potion sont
    /// videes** (`:330-334`). La case 0 de l'ecran etant l'ingredient (`:176`), cette
    /// branche se declenche sur l'ingredient et sur les deux premieres potions.
    ClearsPotions,
    /// `index == 3` : `setInput(EMPTY)`, donc **l'ingredient est vide** (`:335-339`).
    /// C'est la troisieme potion de l'ecran (`:180`) qui declenche ceci.
    ClearsIngredient,
    /// Pile valide comme ingredient : `setInput(stack)` (`:341-345`).
    FillsIngredient,
    /// Pile valide comme potion **et de taille un** : `setContainer(stack)`
    /// (`:346-350`).
    FillsPotions,
    /// Rien (`:353`).
    Nothing,
}

/// La branche de `ContainerCraftingBrewingstand.quickMoveStack` pour cette case et cette
/// pile.
///
/// Les deux predicats de pile sont ceux du registre de brassage vanilla
/// (`BrewingRecipeRegistry.isValidIngredient` / `isValidInput`,
/// `ContainerCraftingBrewingstand.java:341`, `:346`), passes en arguments : ce port n'a
/// pas de registre de recettes de brassage, donc il ne peut pas les calculer lui-meme.
pub fn brewing_shift(index: usize, valid_ingredient: bool, valid_input: bool, count: usize) -> BrewAction {
    if index < 3 {
        return BrewAction::ClearsPotions;
    }
    if index == 3 {
        return BrewAction::ClearsIngredient;
    }
    if valid_ingredient {
        return BrewAction::FillsIngredient;
    }
    if valid_input && count == 1 {
        return BrewAction::FillsPotions;
    }
    BrewAction::Nothing
}

/// L'ingredient est la case zero de l'ecran et la case trois du handler.
///
/// Ce fait est deja porte par `ui_window_08::brewing::SLOTS` ([`crate::ui_window_08::brewing`]),
/// et il est repete ici **une seule fois**, comme une assertion de la meme table : c'est
/// lui qui rend [`BrewAction::ClearsPotions`] destructeur.
pub const INGREDIENT_SCREEN_SLOT: usize = 0;

/// La case du handler qui porte l'ingredient (`ContainerCraftingBrewingstand.java:176`).
pub const INGREDIENT_HANDLER_SLOT: usize = 3;

#[cfg(test)]
mod tests {
    use super::*;

    /// La taille d'un rack neuf, **celle de `ui_window_10::rack`**.
    const RACK: usize = crate::ui_window_10::rack::RACK_SLOTS as usize;

    fn rack() -> &'static Menu {
        menu("ContainerRack").unwrap_or_else(|| panic!("no rack menu"))
    }

    #[test]
    fn seven_menus_are_readable_and_the_eighth_file_is_empty() {
        assert_eq!(MENUS.len(), 7, "huit fichiers dans api/inventory/container, un vide");
        assert!(EMPTY_CONTAINER_FILE.ends_with("ContainerField.java"));
        assert!(menu("ContainerField").is_none(), "le fichier vide ne donne pas de menu");
        for (index, menu) in MENUS.iter().enumerate() {
            assert!(!menu.window.is_empty());
            assert!(menu.container.starts_with("Container"), "{}", menu.container);
            for other in &MENUS[index + 1..] {
                assert_ne!(menu.window, other.window, "deux menus pour une fenetre");
                assert_ne!(menu.container, other.container);
            }
        }
        // Les quatre fenetres de stockage ont la meme regle, les trois grilles non.
        let generic = MENUS.iter().filter(|menu| menu.rule == TransferRule::Generic).count();
        assert_eq!(generic, 4);
        assert_eq!(MENUS.iter().filter(|m| m.rule == TransferRule::GridLocked).count(), 1);
        assert_eq!(MENUS.iter().filter(|m| m.rule == TransferRule::FurnaceLocked).count(), 1);
        assert_eq!(MENUS.iter().filter(|m| m.rule == TransferRule::Brewing).count(), 1);
    }

    #[test]
    fn a_single_rack_is_three_rows_of_nine_and_the_player_brings_three_more() {
        assert_eq!(owned_rows(RACK), 3);
        assert_eq!(display_columns(3, RACK), COLUMNS);
        assert_eq!(extra_offset(3), 0);
        assert_eq!(citizen_extra_offset(3), 1, "le citoyen decale d'un de plus");
        assert_eq!(total_slots(RACK), 27 + 27 + 9);
    }

    #[test]
    fn two_racks_stacked_make_six_rows_and_still_nine_columns() {
        // ContainerRack.java:84-89 empile deux inventaires : 27 + 27.
        let double = RACK * 2;
        assert_eq!(owned_rows(double), 6);
        assert_eq!(display_columns(6, double), COLUMNS, "six lignes tiennent sous la barre");
        assert_eq!(extra_offset(6), 0, "et ne prennent pas la marge de deux");
        assert_eq!(total_slots(double), 54 + 36);
    }

    #[test]
    fn past_eight_rows_java_divides_by_eight_and_not_by_nine() {
        // 9 lignes = 81 cases : display_columns bascule sur `slots / 8 + 1`.
        assert_eq!(display_columns(9, 81), 11, "81 / 8 + 1 = 11, pas 9");
        assert_eq!(display_columns(9, 90), 12);
        assert_eq!(extra_offset(9), 2, "ContainerRack.java:102");
        assert_eq!(display_columns(8, 72), COLUMNS, "huit lignes restent en neuf colonnes");
        assert_eq!(extra_offset(8), 0);
    }

    #[test]
    fn a_full_block_puts_its_last_row_below_the_eight_drawn_ones() {
        // La boucle de ContainerRack.java:105-118 dessine au plus BAR_SIZE lignes.
        let rows = owned_rows(90);
        let drawn = rows.min(BAR_SIZE);
        assert_eq!(drawn, 8);
        assert!(display_columns(rows, 90) * drawn >= 90, "la derniere ligne est partielle");
    }

    #[test]
    fn an_empty_slot_has_nothing_to_move_and_the_rule_says_so() {
        let result = quick_move(rack(), 0, false, RACK);
        assert_eq!(result, Err(Refusal::NothingToMove));
        assert_eq!(result.map_err(|refusal| refusal.name()), Err("container.refusal.empty_slot"));
    }

    #[test]
    fn a_full_slot_goes_to_the_player_or_comes_back_to_the_rack() {
        // ContainerRack.java:181-190, deux branches et deux seulement.
        let from_rack = quick_move(rack(), 0, true, RACK).unwrap_or_else(|_| panic!("refused"));
        assert_eq!(from_rack.range(), (27, 63));
        let from_player = quick_move(rack(), 27, true, RACK).unwrap_or_else(|_| panic!("refused"));
        assert_eq!(from_player.range(), (0, 27));
        // La derniere case du joueur (la barre rapide) est aussi "pas au rack".
        let hotbar = quick_move(rack(), 62, true, RACK).unwrap_or_else(|_| panic!("refused"));
        assert_eq!(hotbar.range(), (0, 27));
    }

    #[test]
    fn a_container_without_cases_of_its_own_can_receive_nothing() {
        // `slots = 0` : `maxIndex = inventorySize * 9` vaut 0 (`ContainerRack.java:179`),
        // donc un clic depuis l'inventaire du joueur vise la plage vide
        // `moveItemStackTo(stackCopy, 0, 0, false)` (`:188`) : aucune iteration, aucun
        // transfert, et Java rend `EMPTY` (`:189-191`). Le port refuse par un nom.
        assert_eq!(quick_move(rack(), 3, true, 0), Err(Refusal::NoTarget { from: 0, to: 0 }));
        assert_eq!(
            quick_move(rack(), 3, true, 0).map_err(|refusal| refusal.name()),
            Err("container.refusal.no_target")
        );
        // Un rack reel (27 cases, `ui_window_10::rack::RACK_SLOTS`) a une cible, lui.
        let transfer = quick_move(rack(), 30, true, RACK).unwrap_or_else(|_| panic!("refused"));
        assert_eq!(transfer.range(), (0, 27));
    }

    #[test]
    fn a_slot_past_the_last_is_not_a_slot() {
        let result = quick_move(rack(), 63, true, RACK);
        assert_eq!(result, Err(Refusal::NoSuchSlot { index: 63, slots: 63 }));
        // La meme borne que le tableau de cases : l'indice 62 existe, le 63 non.
        assert!(quick_move(rack(), 62, true, RACK).is_ok());
    }

    #[test]
    fn the_crafting_result_slot_never_moves_because_java_returns_first() {
        // ContainerCrafting.java:311-315 : `index <= total_crafting_slots`.
        for complete in [false, true] {
            let owned = crafting_owned(complete);
            for index in 0..=owned {
                assert_eq!(crafted_quick_move(index), Err(Refusal::GridLocked), "index {index}");
            }
        }
        let crafting = menu("ContainerCrafting").unwrap_or_else(|| panic!("no crafting"));
        // Et la meme chose par la porte generale, qui ne regarde meme pas la case.
        assert_eq!(quick_move(crafting, 0, true, 0), Err(Refusal::GridLocked));
        assert_eq!(quick_move(crafting, 40, true, 0), Err(Refusal::GridLocked));
    }

    #[test]
    fn the_grid_has_five_slots_or_ten_and_the_five_extra_are_the_3x3() {
        assert_eq!(crafting_owned(false), 5, "resultat + 2x2");
        assert_eq!(crafting_owned(true), 10, "resultat + 3x3");
        assert_eq!(crafting_owned(true) - crafting_owned(false), ADDITIONAL_GRID);
        // Les totaux de Java restent coherents avec les cases ajoutees.
        assert_eq!(TOTAL_SLOTS, CRAFTING_GRID + 27 + 9);
        assert_eq!(TOTAL_SLOTS + ADDITIONAL_GRID, crafting_owned(true) + 27 + 9);
    }

    #[test]
    fn the_furnace_refuses_both_of_its_own_slots() {
        // ContainerCraftingFurnace.java:300-303 : `index <= FURNACE_SLOTS`.
        let furnace = menu("ContainerCraftingFurnace").unwrap_or_else(|| panic!("no furnace"));
        for index in 0..=FURNACE_SLOTS {
            assert_eq!(quick_move(furnace, index, true, 0), Err(Refusal::GridLocked));
        }
        // Le four n'a pas de case possedee "de stockage" : tout le reste est au joueur.
        assert_eq!(total_slots(0), 36);
        assert_eq!(TOTAL_SLOTS_FURNACE, FURNACE_SLOTS + 36);
    }

    #[test]
    fn the_hotbar_constant_disagrees_with_where_the_furnace_layout_puts_it() {
        // Le four ajoute 2 + 27 + 9 cases : sa barre rapide occupe donc 29..38.
        assert_eq!(FURNACE_SLOTS + ROWS * COLUMNS, 29);
        assert_eq!(TOTAL_SLOTS_FURNACE - (FURNACE_SLOTS + ROWS * COLUMNS), 9);
        // Mais `HOTBAR_START` vaut 32, une constante ecrite pour l'atelier : les trois
        // premieres cases de la barre rapide du four tombent donc du cote "inventaire
        // principal" des plages de `ContainerCraftingFurnace.java:319-328`.
        assert!(HOTBAR_START > FURNACE_SLOTS + ROWS * COLUMNS);
        assert_eq!(TOTAL_SLOTS_FURNACE - HOTBAR_START, 6);
        assert_eq!(HOTBAR_START, CRAFTING_GRID + ROWS * COLUMNS);
    }

    #[test]
    fn the_brewing_stand_clears_the_potions_when_you_shift_click_the_ingredient() {
        // Case 0 = l'ingredient (ContainerCraftingBrewingstand.java:176) et `index < 3`
        // vide les trois potions (:330-334).
        assert_eq!(brewing_shift(INGREDIENT_SCREEN_SLOT, true, false, 1), BrewAction::ClearsPotions);
        assert_eq!(brewing_shift(1, false, true, 1), BrewAction::ClearsPotions);
        assert_eq!(brewing_shift(2, false, true, 1), BrewAction::ClearsPotions);
        // Case 3 = la troisieme potion, et c'est l'ingredient qui est efface.
        assert_eq!(brewing_shift(3, false, true, 1), BrewAction::ClearsIngredient);
        assert_ne!(INGREDIENT_SCREEN_SLOT, INGREDIENT_HANDLER_SLOT);
    }

    #[test]
    fn a_stack_from_the_player_fills_the_stand_and_nothing_leaves() {
        // :341-350, et les deux branches rendent EMPTY (rien n'est rendu au joueur).
        assert_eq!(brewing_shift(10, true, false, 3), BrewAction::FillsIngredient);
        assert_eq!(brewing_shift(10, false, true, 1), BrewAction::FillsPotions);
        // Une pile de potions de plus d'une unite ne remplit rien.
        assert_eq!(brewing_shift(10, false, true, 2), BrewAction::Nothing);
        assert_eq!(brewing_shift(10, false, false, 1), BrewAction::Nothing);
        // L'ingredient a priorite sur la potion quand les deux predicats sont vrais.
        assert_eq!(brewing_shift(10, true, true, 1), BrewAction::FillsIngredient);
    }

    #[test]
    fn every_refusal_has_its_own_prefixed_name() {
        let all = [
            Refusal::NoSuchSlot { index: 3, slots: 2 },
            Refusal::NothingToMove,
            Refusal::GridLocked,
            Refusal::BrewingRule,
            Refusal::NoTarget { from: 0, to: 0 },
        ];
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().starts_with("container.refusal."), "{refusal}");
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name());
            }
        }
        assert_eq!(Refusal::GridLocked.to_string(), "container.refusal.grid_locked");
        // L'alambic ne se tranche pas en plages : sa refus a son propre nom.
        let stand = menu("ContainerCraftingBrewingstand").unwrap_or_else(|| panic!("no stand"));
        assert_eq!(quick_move(stand, 10, true, 0), Err(Refusal::BrewingRule));
    }

    #[test]
    fn the_player_half_is_always_the_same_thirty_six_slots() {
        // 27 + 9, quelle que soit la taille de l'inventaire du bloc.
        for slots in [0, 27, 54, 81, 90] {
            let owned = owned_rows(slots) * COLUMNS;
            assert_eq!(total_slots(slots) - owned, 36, "slots {slots}");
        }
    }

    #[test]
    fn the_citizen_inventory_puts_its_armour_after_the_player_slots() {
        // ContainerCitizenInventory.java:231 : les quatre cases d'armure sont ajoutees
        // apres les cases du joueur, donc dans la borne `slots.size()` du maj-clic.
        let slots = RACK;
        let owned = owned_rows(slots) * COLUMNS;
        let citizen = menu("ContainerCitizenInventory").unwrap_or_else(|| panic!("no citizen menu"));
        assert_eq!(tail_slots(Inventory::StorageAndArmour, slots), owned + 36 + ARMOUR_SLOTS);
        // Un rack n'a pas d'armure : sa borne est celle du joueur.
        assert_eq!(tail_slots(Inventory::Storage, slots), owned + 36);
        // Depuis l'inventaire du citoyen, l'armure est une destination de maj-clic...
        let to_player = quick_move(citizen, 0, true, slots).unwrap_or_else(|_| panic!("refused"));
        assert_eq!(to_player.range(), (owned, owned + 36 + ARMOUR_SLOTS));
        // ...et une case d'armure renvoie vers l'inventaire du citoyen.
        let from_armour = quick_move(citizen, owned + 36 + 1, true, slots)
            .unwrap_or_else(|_| panic!("refused"));
        assert_eq!(from_armour.range(), (0, owned));
        // La case d'apres l'armure n'existe pas.
        assert_eq!(
            quick_move(citizen, owned + 36 + ARMOUR_SLOTS, true, slots),
            Err(Refusal::NoSuchSlot { index: owned + 40, slots: owned + 40 })
        );
    }

    #[test]
    fn the_four_pixel_constants_say_why_eight_is_not_nine() {
        // `BAR_SIZE` sert deux fois : decalage de colonne (huit pixels) et diviseur.
        assert_eq!(BAR_SIZE, 8);
        assert_ne!(BAR_SIZE, COLUMNS);
        assert_eq!(PLAYER_X as usize, BAR_SIZE, "la colonne du bloc demarre a huit");
        assert_eq!(PLAYER_Y, 30);
        assert_eq!(HOTBAR_Y - PLAYER_Y, 58, "trois lignes de 18 plus quatre");
        assert_eq!(SLOT_PX, 18);
    }
}
