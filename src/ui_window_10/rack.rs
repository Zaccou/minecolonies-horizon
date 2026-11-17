//! L'écran du rack : la grille de cases qu'il dessine et les deux textes qu'il écrit.
//!
//! Java reference : `core/client/gui/containers/WindowRack.java` (155 lignes),
//! `api/inventory/container/ContainerRack.java:72-145` (les slots et leurs coordonnées),
//! `api/util/constant/InventoryConstants.java:11-41`, `api/util/constant/Constants.java:143`
//! (`DEFAULT_SIZE`), `:163` (`SLOT_PER_LINE`), et
//! `core/colony/buildings/workerbuildings/BuildingWareHouse.java:51` (`MAX_STORAGE_UPGRADE`).
//!
//! **Ce n'est pas un écran de hutte.** `WindowRack` étend `AbstractContainerScreen`
//! (`:21`), pas `AbstractBuildingWindow` : il n'a **pas de XML**, pas de `findPaneOfTypeByID`,
//! pas de `registerButton`, et `src/ui` ne le porte pas. C'est le seul écran de cette
//! tranche dans ce cas, et [`crate::ui_window_10::Entry::Rack`] existe pour lui.
//!
//! Ce que l'écran **demande** est donc une seule chose : le nombre de slots de
//! l'inventaire joint, et l'ordre dans lequel les deux racks y sont mis. Ce qu'il
//! **rend** est une grille de cases et deux traits de texte — dont aucun n'a d'id.
//!
//! # Une première fausse piste, corrigée par la mesure
//!
//! Le nom de la texture est calculé (`:136-146`) : `gui%s.png` avec
//! `inventoryRows * 9` pour argument. Les fichiers présents sous
//! `assets/minecolonies/textures/gui/` sont exactement `gui81`, `gui90`, `gui99` et
//! `gui108`, plus `generic_108` pour les huit lignes ou moins. On pouvait en conclure que
//! la formule nomme des fichiers absents — c'est faux, et le calcul le montre :
//!
//! | taille d'un rack | où | |
//! | --- | --- | --- |
//! | 27 | `Constants.DEFAULT_SIZE` | un rack simple |
//! | 27 + `size` × 9, `size` ≤ 3 | `TileEntityRack.upgradeRackSize` (`:270-273`) + `MAX_STORAGE_UPGRADE` | 36, 45, 54 |
//! | deux racks joints | `CombinedInvWrapper` (`ContainerRack.java:80-94`) | 54 → 108, pas de 9 |
//!
//! Un rack plafonne donc à **54** slots, deux à **108**, et les quatre textures
//! nommées couvrent exactement les quatre tailles qui dépassent huit lignes : 81, 90,
//! 99, 108. [`REACHABLE`] énumère l'espace réellement atteignable et un test vérifie
//! qu'aucune de ses cases ne tombe sur [`Refusal::MissingRowTexture`]. Le refus reste,
//! parce qu'un NBT modifié à la main peut sortir de cet espace.
//!
//! # Deux nombres de lignes, et le dessin n'utilise pas le même que la texture
//!
//! `inventoryRows` (`:88`) n'est pas plafonné ; `rows` (`:89`) l'est à
//! `INVENTORY_BAR_SIZE` = 8. La texture est choisie **sur le premier** (`:115`) et le
//! blit sur le second (`:117-127`). Un rack de 108 slots a donc 12 lignes de contenu,
//! une texture de 12 lignes (`gui108`) et **8 lignes dessinées** — 112 cases pour 108
//! objets. [`Grid`] garde les deux nombres séparément : les confondre ferait dessiner une
//! texture de 12 lignes sur une hauteur de 8.

use super::refusal::Refusal;
use crate::ui::{Action, Field};

/// L'id de cet écran dans la table de ce module. **Java n'en a pas** : un rack s'ouvre
/// par son bloc ([`crate::blocks::Window::Rack`], `BlockMinecoloniesRack.java:234-241`).
pub const ID: &str = "rack";

/// La classe Java.
pub const JAVA: &str = "WindowRack";

/// La texture du coffre générique, `CHEST_GUI_TEXTURE` (`WindowRack.java:26`).
pub const CHEST_TEXTURE: &str = "minecolonies:textures/gui/generic_108.png";

/// Le gabarit des textures de plus de huit lignes (`:31`, appliqué à `:144`).
pub const LOCATION: &str = "textures/gui/gui%s.png";

/// Le bloc du rack, dont ce module lit les variantes au lieu de les redéclarer.
pub const RACK_BLOCK: &str = "minecolonies:blockminecoloniesrack";

/// La propriété de bloc qui décide de l'ordre des deux inventaires
/// (`AbstractBlockMinecoloniesRack.java:13`, lue à `WindowRack.java:73`).
pub const VARIANT_PROPERTY: &str = "variant";

/// La variante d'un rack qui **ne se dessine pas** (`RackType.java:16`) : elle passe
/// derrière son voisin.
pub const NO_RENDER: &str = "dontrender";

/// `INVENTORY_COLUMNS` (`InventoryConstants.java:11`).
pub const COLUMNS: i32 = 9;
/// `INVENTORY_BAR_SIZE` (`:41`) — le plafond de lignes **dessinées**.
pub const BAR_SIZE: i32 = 8;
/// `PLAYER_INVENTORY_OFFSET_EACH` (`:26`), l'écart entre deux cases.
pub const SLOT: i32 = 18;
/// `GOOD_SIZE` (`WindowRack.java:41`) : au-delà, la texture dépend du nombre de lignes.
pub const GOOD_SIZE: i32 = 8;
/// `Y_OFFSET` (`:46`).
pub const Y_OFFSET: i32 = 114;
/// `TEXTURE_SIZE` (`:36`), la taille de la texture source d'un blit.
pub const TEXTURE_SIZE: i32 = 350;
/// `TEXTURE_HEIGHT` (`:51`), la hauteur du second blit du découpage.
pub const TEXTURE_HEIGHT: i32 = 96;
/// `TEXTURE_OFFSET` (`:56`, écrit `126 * 2 - 17`).
pub const TEXTURE_OFFSET: i32 = 238;
/// La largeur d'un `AbstractContainerScreen` avant l'élargissement de `:93-96` :
/// `imageWidth` de la classe vanilla, que `WindowRack` **ajoute** au lieu de la poser.
pub const BASE_WIDTH: i32 = 176;
/// `INVENTORY_ROWS` (`InventoryConstants.java:36`), les trois rangées du joueur.
pub const PLAYER_ROWS: i32 = 3;
/// Les cases du joueur : trois rangées de neuf et la barre rapide
/// (`ContainerRack.java:120-145`).
pub const PLAYER_SLOTS: i32 = 36;
/// `DEFAULT_SIZE` (`Constants.java:143`) : les slots d'un rack jamais amélioré.
pub const RACK_SLOTS: i32 = 27;
/// `MAX_STORAGE_UPGRADE` (`BuildingWareHouse.java:51`) : trois améliorations, pas plus,
/// donc `27 + 3 × 9 = 54` slots au maximum par rack.
pub const MAX_STORAGE_UPGRADE: i32 = 3;
/// La position du titre, `drawString(title, 8, 6)` (`:105`).
pub const TITLE_POS: [i32; 2] = [8, 6];
/// L'abscisse du titre de l'inventaire du joueur (`:106`).
pub const INVENTORY_TITLE_X: i32 = 8;
/// Le retrait vertical du titre de l'inventaire du joueur, `imageHeight - 94` (`:106`).
/// **Un littéral dans Java**, pas une constante nommée.
pub const INVENTORY_TITLE_INSET: i32 = 94;

/// Les quatre textures qui portent un nombre de lignes dans leur nom, mesurées sous
/// `assets/minecolonies/textures/gui/` : `gui81`, `gui90`, `gui99`, `gui108`.
pub const ROW_TEXTURES: &[(i32, &'static str)] =
    &[(9, "gui81"), (10, "gui90"), (11, "gui99"), (12, "gui108")];

/// Les tailles de slot qu'un rack peut réellement avoir : `27 + 9k`, `k` de 0 à
/// `MAX_STORAGE_UPGRADE`.
pub const RACK_SLOTS_BY_UPGRADE: [i32; 4] = [27, 36, 45, 54];

/// L'espace atteignable : un rack seul, puis deux racks joints, chacun dans
/// [`RACK_SLOTS_BY_UPGRADE`] — 27 à 108 slots, de 9 en 9.
///
/// `None` est l'absence de voisin (`WindowRack.java:82-85`).
pub const REACHABLE: &[(i32, Option<i32>)] = &[
    (27, None), (36, None), (45, None), (54, None),
    (27, Some(27)), (27, Some(36)), (27, Some(45)), (27, Some(54)),
    (36, Some(27)), (36, Some(36)), (36, Some(45)), (36, Some(54)),
    (45, Some(27)), (45, Some(36)), (45, Some(45)), (45, Some(54)),
    (54, Some(27)), (54, Some(36)), (54, Some(45)), (54, Some(54)),
];

/// De quel côté les deux inventaires sont joints, `WindowRack.java:71-85`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two branches of Java's `if (container.neighborRack != null)`
pub enum Joint {
    /// Pas de voisin : l'inventaire du rack seul (`:84`).
    Single,
    /// Un voisin qui se dessine : **ce** rack d'abord (`:75`).
    RackFirst,
    /// Un voisin `NO_RENDER` : le voisin d'abord, parce que c'est lui qu'on voit
    /// (`:79`).
    NeighbourFirst,
}

/// L'ordre des deux inventaires, exactement comme Java le décide.
pub fn joint(neighbour: bool, variant: &str) -> Joint {
    if !neighbour {
        Joint::Single
    } else if variant == NO_RENDER {
        Joint::NeighbourFirst
    } else {
        Joint::RackFirst
    }
}

/// Le nombre de slots de l'inventaire joint — ce que `getSlots()` répond sur le
/// `CombinedInvWrapper` (`:75-84`, lu à `:87`).
pub fn slots(order: Joint, rack: i32, neighbour: i32) -> i32 {
    match order {
        Joint::Single => rack,
        Joint::RackFirst | Joint::NeighbourFirst => rack + neighbour,
    }
}

/// La géométrie que `WindowRack` calcule dans son constructeur (`:87-96`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    /// Le nombre de slots joints.
    pub slots: i32,
    /// `inventoryRows` (`:88`) : **non plafonné**, et c'est lui qui choisit la texture.
    pub inventory_rows: i32,
    /// `rows` (`:89`) : plafonné à [`BAR_SIZE`], et c'est lui qui décide de la hauteur.
    pub rows: i32,
    /// `columns` (`:90`). Java divise par `INVENTORY_BAR_SIZE` — huit, pas neuf — donc
    /// la grille est plus large que le strict nécessaire dès qu'elle dépasse huit
    /// lignes : 81 slots tiennent en 9 colonnes et la fenêtre en réserve 11.
    pub columns: i32,
    /// `imageHeight` (`:92`).
    pub image_height: i32,
    /// `imageWidth` (`:93-96`).
    pub image_width: i32,
}

impl Grid {
    /// La grille de `slots` slots, ou le refus nommé quand la boucle de
    /// `ContainerRack` ne peut pas toutes les placer.
    pub fn of(slots: i32) -> Result<Grid, Refusal> {
        let inventory_rows = slots / COLUMNS;
        let rows = inventory_rows.min(BAR_SIZE);
        let columns = if inventory_rows <= BAR_SIZE {
            COLUMNS
        } else {
            slots / BAR_SIZE + 1
        };
        let image_height = Y_OFFSET + rows * SLOT;
        let image_width = if columns > COLUMNS {
            BASE_WIDTH + (columns - COLUMNS) * SLOT
        } else {
            BASE_WIDTH
        };
        let grid = Grid { slots, inventory_rows, rows, columns, image_height, image_width };
        if slots < 0 || grid.capacity() < slots {
            // `ContainerRack.java:109-116` double la boucle de deux plafonds et garde
            // `index < size` : au-delà de `capacity()`, les objets restants n'ont pas de
            // case. Java les perd en silence.
            return Err(Refusal::UnreachableSlots { slots });
        }
        Ok(grid)
    }

    /// Les cases que la grille offre : `rows × columns`, pas `slots`.
    pub fn capacity(&self) -> i32 {
        self.rows * self.columns
    }

    /// Les slots que la dernière rangée dessinée laisse vides.
    pub fn spare_in_last_row(&self) -> i32 {
        let used = self.slots - (self.rows - 1) * self.columns;
        self.columns - used
    }

    /// Le nom complet de la texture de fond, `getCorrectTextureForSlots`
    /// (`:136-146`) — qui reçoit **`inventoryRows`**, pas `rows`.
    pub fn texture(&self) -> Result<String, Refusal> {
        texture_for_rows(self.inventory_rows)
    }

    /// La hauteur du premier blit quand la texture est découpée (`:119`). Java calcule
    /// cette valeur avec `inventoryRows` (`:119`), ce qui revient à `rows × 18 + 17`
    /// dans la branche où elle sert, les deux nombres y étant égaux.
    pub fn split_height(&self) -> i32 {
        self.rows * SLOT + SLOT - 1
    }
}

/// La texture d'un nombre de lignes, ou [`Refusal::MissingRowTexture`].
///
/// Jusqu'à [`GOOD_SIZE`] lignes, une seule texture sert et son nom ne change pas
/// (`:138-141`) ; au-delà, le nom porte `rows × 9` (`:144`) et seuls quatre fichiers
/// existent.
pub fn texture_for_rows(rows: i32) -> Result<String, Refusal> {
    if rows <= GOOD_SIZE {
        return Ok(CHEST_TEXTURE.to_owned());
    }
    ROW_TEXTURES
        .iter()
        .find(|(candidate, _)| *candidate == rows)
        .map(|(_, name)| format!("minecolonies:textures/gui/{name}.png"))
        .ok_or(Refusal::MissingRowTexture { rows })
}

/// L'abscisse et l'ordonnée du titre de l'inventaire du joueur (`:106`).
pub fn inventory_title_pos(image_height: i32) -> [i32; 2] {
    [INVENTORY_TITLE_X, image_height - INVENTORY_TITLE_INSET]
}

/// Les variantes du bloc du rack, **lues dans** [`crate::blocks`] plutôt que recopiées :
/// les cinq valeurs de `RackType` (`RackType.java:13-17`) sont déjà une ligne de la
/// table des blocs (`decoration.rs`, `RACK_VARIANT`).
pub fn variants() -> Result<&'static [&'static str], Refusal> {
    let block =
        crate::blocks::block(RACK_BLOCK).ok_or(Refusal::RackVariantMissing)?;
    let property = block
        .states
        .iter()
        .find(|state| state.name == VARIANT_PROPERTY)
        .ok_or(Refusal::RackVariantMissing)?;
    match property.values {
        crate::blocks::Values::Enum(values) => Ok(values),
        // `RACK_VARIANT` est un `Values::Enum` : le reste serait une table cassée, pas
        // une entrée utilisateur.
        _ => Err(Refusal::RackVariantMissing),
    }
}

/// L'écran du rack, dans la forme que [`crate::ui_window_10::Entry`] attend d'un écran.
///
/// Ses deux listes sont **vides**, et c'est le fait : pas un panneau du rack n'a d'id
/// (les deux textes sont des `drawString` sans `findPaneOfTypeByID`, `:105-106`) et pas
/// un bouton n'est enregistré (`:68-97`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// L'id de ce module.
    pub id: &'static str,
    /// La classe Java.
    pub java: &'static str,
    /// La texture — le rack est le seul écran de la tranche sans XML.
    pub resource: &'static str,
    /// Les panneaux à id : aucun.
    pub fields: &'static [Field],
    /// Les boutons : aucun.
    pub actions: &'static [Action],
}

/// Le rack.
pub static WINDOW: Window = Window {
    id: ID,
    java: JAVA,
    resource: CHEST_TEXTURE,
    fields: &[],
    actions: &[],
};

/// Une action du rack par id — toujours [`Refusal::RackTakesNoButton`], et c'est la
/// réponse : Java n'enregistre rien, donc un clic dans cette fenêtre est une case, pas un
/// bouton (`ContainerRack.clicked`, `:149-164`).
pub fn try_action(id: &'static str) -> Result<Action, Refusal> {
    WINDOW
        .actions
        .iter()
        .copied()
        .find(|action| action.id == id)
        .ok_or(Refusal::RackTakesNoButton { id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_rack_draws_three_rows_in_the_generic_chest_texture() {
        // WindowRack.java:88-96 pour 27 slots.
        let grid = Grid::of(RACK_SLOTS).unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(grid.inventory_rows, 3, "27 / 9");
        assert_eq!(grid.rows, 3);
        assert_eq!(grid.columns, 9);
        assert_eq!(grid.image_height, 168, "114 + 3 * 18");
        assert_eq!(grid.image_width, BASE_WIDTH, ":93 ne s'applique pas sous 9 colonnes");
        assert_eq!(grid.capacity(), 27);
        assert_eq!(grid.spare_in_last_row(), 0);
        assert_eq!(grid.texture().as_deref(), Ok(CHEST_TEXTURE));
        assert_eq!(grid.split_height(), 71, ":119 — 3 * 18 + 18 - 1");
    }

    #[test]
    fn the_drawn_rows_stop_at_eight_and_the_texture_keeps_counting() {
        // 108 slots : douze lignes de contenu, huit dessinées.
        let grid = Grid::of(108).unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(grid.inventory_rows, 12);
        assert_eq!(grid.rows, BAR_SIZE, ":89 plafonne le dessin, pas :88");
        assert_eq!(grid.columns, 108 / 8 + 1, "Java divise par INVENTORY_BAR_SIZE");
        assert_eq!(grid.columns, 14);
        assert_eq!(grid.image_height, 258, "114 + 8 * 18");
        assert_eq!(grid.image_width, 266, "176 + (14 - 9) * 18");
        assert_eq!(grid.capacity(), 112, "112 cases pour 108 objets");
        assert_eq!(grid.spare_in_last_row(), 4);
        assert_eq!(
            grid.texture().as_deref(),
            Ok("minecolonies:textures/gui/gui108.png"),
            ":115 passe inventoryRows à getCorrectTextureForSlots"
        );
        // Les deux nombres diffèrent : c'est exactement le piège.
        assert_ne!(grid.rows, grid.inventory_rows);
    }

    #[test]
    fn every_reachable_rack_size_has_a_texture_file() {
        // L'espace atteignable est exactement le produit des tailles possibles.
        assert_eq!(REACHABLE.len(), RACK_SLOTS_BY_UPGRADE.len() * (RACK_SLOTS_BY_UPGRADE.len() + 1));
        for rack in RACK_SLOTS_BY_UPGRADE {
            assert_eq!(rack % COLUMNS, 0, "{rack} n'est pas un multiple de neuf");
            assert!(rack >= RACK_SLOTS, "{rack} descend sous DEFAULT_SIZE");
            assert!(
                rack <= RACK_SLOTS + MAX_STORAGE_UPGRADE * COLUMNS,
                "{rack} dépasse trois améliorations"
            );
        }
        for (rack, neighbour) in REACHABLE {
            let order = joint(neighbour.is_some(), NO_RENDER);
            let slots = slots(order, *rack, neighbour.unwrap_or(0));
            let grid = Grid::of(slots).unwrap_or_else(|refusal| {
                panic!("{rack}+{neighbour:?} = {slots} slots : {refusal}")
            });
            assert!(
                grid.texture().is_ok(),
                "{slots} slots demandent {} lignes, texture absente",
                grid.inventory_rows
            );
            assert!(grid.capacity() >= slots, "{slots} slots ne tiennent pas");
            assert!(grid.rows <= BAR_SIZE);
        }
        // Deux racks plafonnent à 2 × 54 et le plus gros tient dans gui108.
        let biggest = Grid::of(108).unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(
            biggest.texture().as_deref(),
            Ok("minecolonies:textures/gui/gui108.png")
        );
    }

    #[test]
    fn the_first_eight_row_counts_share_one_texture_and_the_next_four_have_their_own() {
        for rows in 0..=GOOD_SIZE {
            assert_eq!(texture_for_rows(rows).as_deref(), Ok(CHEST_TEXTURE), "{rows}");
        }
        for (rows, name) in ROW_TEXTURES {
            let expected = format!("minecolonies:textures/gui/{name}.png");
            assert_eq!(texture_for_rows(*rows), Ok(expected));
            assert!(*rows > GOOD_SIZE, "sous huit lignes, la texture ne change pas");
        }
        // Treize lignes : `gui117.png` est calculé et n'existe pas.
        assert_eq!(
            texture_for_rows(13),
            Err(Refusal::MissingRowTexture { rows: 13 })
        );
        assert_eq!(
            texture_for_rows(13).map_err(Refusal::name),
            Err("ui10.refusal.missing_row_texture")
        );
    }

    #[test]
    fn the_neighbour_decides_the_order_and_a_no_render_rack_moves_last() {
        // :82-85.
        assert_eq!(joint(false, "fullsingle"), Joint::Single);
        assert_eq!(joint(true, "fullsingle"), Joint::RackFirst);
        assert_eq!(joint(true, "emptysingle"), Joint::RackFirst);
        // `NO_RENDER` : le rack invisible passe derrière (`:77-80`).
        assert_eq!(joint(true, NO_RENDER), Joint::NeighbourFirst);
        assert_ne!(joint(true, NO_RENDER), joint(true, "full"));
        // Les deux jointures additionnent ; seule la solitude ne le fait pas.
        assert_eq!(slots(Joint::Single, 27, 54), 27);
        assert_eq!(slots(Joint::RackFirst, 27, 54), 81);
        assert_eq!(slots(Joint::NeighbourFirst, 27, 54), 81);
    }

    #[test]
    fn the_rack_has_no_pane_with_an_id_and_no_button_at_all() {
        assert_eq!(WINDOW.id, ID);
        assert_eq!(WINDOW.java, JAVA);
        assert!(WINDOW.fields.is_empty(), "aucun findPaneOfTypeByID dans WindowRack");
        assert!(WINDOW.actions.is_empty(), "aucun registerButton dans WindowRack");
        // Demander une action n'est pas une absence muette : c'est un refus nommé.
        assert_eq!(
            try_action("hire"),
            Err(Refusal::RackTakesNoButton { id: "hire" })
        );
        let refusal = try_action("hire").err();
        assert_eq!(refusal.map(Refusal::key), Some(None));
        // La ressource est une texture, pas un XML — le seul écran de la tranche ainsi.
        assert!(WINDOW.resource.starts_with("minecolonies:textures/gui/"));
        assert!(WINDOW.resource.ends_with(".png"));
    }

    #[test]
    fn the_two_text_runs_have_positions_and_no_pane_id() {
        // :105-106.
        assert_eq!(TITLE_POS, [8, 6]);
        let grid = Grid::of(54).unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(grid.image_height, 222, "114 + 6 * 18");
        assert_eq!(inventory_title_pos(grid.image_height), [8, 128], "222 - 94");
        // Le second texte descend avec la fenêtre, le premier non.
        let tall = Grid::of(108).unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(inventory_title_pos(tall.image_height), [8, 164]);
        assert_eq!(TITLE_POS[1], 6);
    }

    #[test]
    fn a_slot_count_the_container_loop_cannot_place_is_refused() {
        // 70 slots : 7 lignes de 9 cases = 63 places, 7 objets sans case.
        assert_eq!(Grid::of(70), Err(Refusal::UnreachableSlots { slots: 70 }));
        // 73 à 80 tombent dans le même trou.
        for slots in 73..=80 {
            assert!(Grid::of(slots).is_err(), "{slots}");
        }
        // Les tailles réelles, elles, tombent juste.
        for (rack, neighbour) in REACHABLE {
            let count = slots(joint(neighbour.is_some(), "full"), *rack, neighbour.unwrap_or(0));
            assert!(Grid::of(count).is_ok(), "{count}");
        }
        // Et un négatif est refusé plutôt que dessiné à l'envers.
        assert!(Grid::of(-9).is_err());
    }

    #[test]
    fn the_rack_variants_are_the_block_table_s_own() {
        let values = variants().unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(values.len(), 5, "RackType.java:13-17");
        assert_eq!(values[0], "emptysingle");
        assert_eq!(values[4], NO_RENDER);
        assert!(values.contains(&NO_RENDER));
        // Ce module ne redéclare pas la liste : il lit celle du bloc.
        let block = crate::blocks::block(RACK_BLOCK)
            .unwrap_or_else(|| panic!("le rack n'est pas dans la table des blocs"));
        assert_eq!(block.click, crate::blocks::Click::Rack);
        assert_eq!(block.states.len(), 2, "facing et variant seulement");
    }
}
