//! `WindowGrave` : une tombe, deux titres, aucune action — et trois branches que ce mod ne
//! peut pas atteindre.
//!
//! Java reference : `core/client/gui/containers/WindowGrave.java` (169 lignes),
//! `api/inventory/container/ContainerGrave.java:59-116`, `core/tileentities/
//! TileEntityGrave.java:204-219` et `api/tileentities/AbstractTileEntityGrave.java:42`.
//!
//! # Une tombe a toujours six rangées, donc deux des trois branches sont mortes
//!
//! Le seul inventaire de tombe du mod est celui d'`AbstractTileEntityGrave` :
//! `super(tileEntityTypeIn, pos, state, DEFAULT_SIZE * 2)` (`:42`), et `DEFAULT_SIZE` vaut
//! **27** (`Constants.java:143`) — soit **54 emplacements**, donc
//! `54 / SLOTS_EACH_ROW = 6` rangées (`WindowGrave.java:93`), toujours. Or la classe a trois
//! branches qui dépendent des rangées :
//!
//! | Branche | Seuil | Atteinte à 6 rangées ? |
//! | --- | --- | --- |
//! | `getCorrectTextureForSlots` (`:150-160`) | `rows <= 8` → `generic_108.png` | le `else` **jamais** |
//! | l'offset du titre d'inventaire (`:109`) | `rows > 6 ? 110 : 94` | `94`, et `110` **jamais** |
//! | l'élargissement de l'image (`:96-99`) | `rows > 8` | **jamais** |
//!
//! Les quatre textures du `else` existent pourtant dans le dépôt — `gui81.png`, `gui90.png`,
//! `gui99.png`, `gui108.png`, c'est-à-dire 9 à 12 rangées — et au-delà de douze la classe
//! demanderait `gui117.png`, qui n'existe pas. Aucune des deux ne peut arriver : le port
//! garde la fonction complète et dit, ici et par un test, que la tombe de ce mod n'en prend
//! qu'un chemin.
//!
//! # Le titre vient du bloc, pas du conteneur
//!
//! `ContainerGrave` ne porte aucun nom : `AbstractTileEntityGrave.getDisplayName()` répond
//! `Component.literal("Grave")` (`TileEntityGrave.java:218`), et `WindowGrave` dessine ce
//! titre en `:108` — un **littéral**, pas une clé de traduction, donc intraduisible. Le
//! second texte est `playerInventoryTitle` (`:109`), que le jeu fournit.
//!
//! # Aucun bouton, aucun panneau
//!
//! La classe ne fait que `renderBg` et `renderLabels` : pas un `registerButton`, pas un
//! `findPaneOfTypeByID`. [`BUTTONS`] et [`FIELDS`] sont donc vides, et c'est la fenêtre du
//! groupe qui n'a **rien** à demander — un joueur y prend son stuff et s'en va.

use super::pane::Pane;
use super::Button;

/// La classe Java.
pub const JAVA: &str = "WindowGrave";
/// Le titre : `Component.literal("Grave")` (`TileEntityGrave.java:218`). Pas une clé.
pub const TITLE: &str = "Grave";
/// La texture des tombes de huit rangées ou moins (`:21`).
pub const TEXTURE_DEFAULT: &str = "minecolonies:textures/gui/generic_108.png";
/// Le motif des textures au-delà (`:26`), `%s` remplacé par le nombre d'emplacements
/// (`:158`).
pub const TEXTURE_PATTERN: &str = "minecolonies:textures/gui/gui%s.png";
/// Les emplacements par rangée (`:31`), et la même constante que le conteneur utilise
/// (`ContainerGrave.java:68`).
pub const SLOTS_EACH_ROW: usize = 9;
/// Le nombre de rangées à partir duquel la texture change (`:46`).
pub const GOOD_SIZE: i32 = 8;
/// Le nombre de rangées à partir duquel le titre d'inventaire descend de 16 pixels (`:109`).
/// **Un autre seuil que [`GOOD_SIZE`]**, et il est écrit en clair dans l'expression.
pub const LABEL_ROW_SPLIT: i32 = 6;
/// Les deux offsets du titre d'inventaire (`:109`).
pub const LABEL_OFFSET_LOW: i32 = 94;
pub const LABEL_OFFSET_HIGH: i32 = 110;
/// Les emplacements d'une tombe : `DEFAULT_SIZE * 2` (`AbstractTileEntityGrave.java:42`,
/// `Constants.java:143`).
pub const GRAVE_SLOTS: usize = 54;
/// Les rangées d'une tombe, toujours : `GRAVE_SLOTS / SLOTS_EACH_ROW`.
pub const GRAVE_ROWS: i32 = 6;

/// Aucun panneau nommé.
///
/// Les deux textes de la fenêtre sont deux `stack.drawString` (`:108-109`) et ses
/// emplacements sont ajoutés par `ContainerGrave` (`:75-88`) : pas un id, donc pas un
/// panneau. `src/ui/MANQUANT.md` §1 dit quel widget manque pour une grille.
pub const FIELDS: &[Pane] = &[];

/// Aucun bouton : la classe n'en enregistre pas un seul (`:88-100`).
pub const BUTTONS: &[Button] = &[];

/// Le nombre de rangées que ce nombre d'emplacements donne (`:93`,
/// `inv.getSlots() / SLOTS_EACH_ROW`). Une division entière : 53 emplacements font cinq
/// rangées, et le cinquante-quatrième est perdu de vue.
pub fn rows(slots: usize) -> i32 {
    (slots / SLOTS_EACH_ROW) as i32
}

/// La texture que la fenêtre choisit (`:150-160`).
///
/// `rows <= 8` → [`TEXTURE_DEFAULT`], sinon `gui<nombre d'emplacements>.png`. Les quatre
/// fichiers que le mod livre sont `gui81`, `gui90`, `gui99` et `gui108` : au-delà de douze
/// rangées, le nom est bien formé et le fichier n'existe pas.
pub fn texture(rows: i32) -> String {
    if rows <= GOOD_SIZE {
        TEXTURE_DEFAULT.to_owned()
    } else {
        TEXTURE_PATTERN.replace("%s", &(rows * SLOTS_EACH_ROW as i32).to_string())
    }
}

/// L'offset vertical du titre d'inventaire (`:109`) : `imageHeight - (rows > 6 ? 110 : 94)`.
/// Le port rend l'écart à soustraire, pas la position, qui est du dessin.
pub fn player_label_offset(rows: i32) -> i32 {
    if rows > LABEL_ROW_SPLIT {
        LABEL_OFFSET_HIGH
    } else {
        LABEL_OFFSET_LOW
    }
}

/// Le texte que `renderLabels` écrit à gauche, en haut (`:108`) : le titre seul.
pub fn title_text() -> &'static str {
    TITLE
}

/// Le nom de la fenêtre dans son état d'ouverture : les rangées d'une tombe, sa texture et
/// son offset. C'est tout ce que cette fenêtre a à demander.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Le nombre d'emplacements de l'inventaire de la tombe.
    pub slots: usize,
    /// Les rangées affichées.
    pub rows: i32,
    /// La texture choisie.
    pub texture: String,
    /// L'offset du titre d'inventaire.
    pub label_offset: i32,
}

impl Layout {
    /// La disposition d'une tombe de ce nombre d'emplacements (`:93-99`, `:109`, `:150`).
    pub fn of(slots: usize) -> Layout {
        let rows = rows(slots);
        Layout {
            slots,
            rows,
            texture: texture(rows),
            label_offset: player_label_offset(rows),
        }
    }

    /// La disposition d'une tombe réelle : [`GRAVE_SLOTS`] emplacements.
    pub fn grave() -> Layout {
        Layout::of(GRAVE_SLOTS)
    }

    /// Si la tombe prend la branche « grande » de la texture, celle des quatre fichiers
    /// `gui*.png`.
    pub fn uses_large_texture(&self) -> bool {
        self.rows > GOOD_SIZE
    }

    /// Si le titre d'inventaire est descendu.
    pub fn pushes_label_down(&self) -> bool {
        self.rows > LABEL_ROW_SPLIT
    }
}

#[cfg(test)]
mod tests {
    use super::super::Naming;
    use super::*;

    #[test]
    fn a_grave_always_has_six_rows() {
        assert_eq!(GRAVE_SLOTS, 54, "DEFAULT_SIZE * 2, Constants.java:143");
        assert_eq!(rows(GRAVE_SLOTS), 6);
        let layout = Layout::grave();
        assert_eq!(layout.rows, GRAVE_ROWS);
        assert_eq!(layout.slots, 54);
        // Une division entière, comme Java : le reste est ignoré.
        assert_eq!(rows(53), 5);
        assert_eq!(rows(54), 6);
        assert_eq!(rows(0), 0);
        assert_eq!(rows(8), 0);
        assert_eq!(rows(9), 1);
    }

    #[test]
    fn both_thresholds_are_out_of_reach_for_a_real_grave() {
        let layout = Layout::grave();
        // Le seuil de la texture est 8, celui du libellé est 6 : deux nombres, un seul
        // atteint.
        assert!(!layout.uses_large_texture(), "6 <= 8");
        assert!(!layout.pushes_label_down(), "6 > 6 est faux");
        assert_eq!(layout.texture, TEXTURE_DEFAULT);
        assert_eq!(layout.label_offset, LABEL_OFFSET_LOW);
        // Les quatre textures `gui*.png` du dépôt correspondent aux rangées 9 à 12.
        assert_eq!(texture(9), "minecolonies:textures/gui/gui81.png");
        assert_eq!(texture(10), "minecolonies:textures/gui/gui90.png");
        assert_eq!(texture(11), "minecolonies:textures/gui/gui99.png");
        assert_eq!(texture(12), "minecolonies:textures/gui/gui108.png");
        // Au-delà de douze, le nom se forme et le fichier n'existe pas.
        assert_eq!(texture(13), "minecolonies:textures/gui/gui117.png");
        assert!(Layout::of(117).uses_large_texture());
    }

    #[test]
    fn the_two_thresholds_do_not_move_together() {
        // Sept rangées : la texture est encore la petite, le libellé est déjà descendu.
        assert_eq!(texture(LABEL_ROW_SPLIT + 1), TEXTURE_DEFAULT);
        assert_eq!(
            player_label_offset(LABEL_ROW_SPLIT + 1),
            LABEL_OFFSET_HIGH,
            "deux seuils différents dans la même classe"
        );
        assert_eq!(player_label_offset(LABEL_ROW_SPLIT), LABEL_OFFSET_LOW);
        assert_ne!(LABEL_OFFSET_HIGH, LABEL_OFFSET_LOW);
        // Et la texture ne bascule qu'à neuf.
        assert_eq!(texture(GOOD_SIZE), TEXTURE_DEFAULT);
        assert_ne!(texture(GOOD_SIZE + 1), TEXTURE_DEFAULT);
        assert_eq!(GOOD_SIZE, 8);
        assert_eq!(SLOTS_EACH_ROW, 9, "INVENTORY_COLUMNS, InventoryConstants.java:11");
    }

    #[test]
    fn the_grave_window_shows_nothing_and_does_nothing() {
        assert!(FIELDS.is_empty(), "pas un findPaneOfTypeByID dans WindowGrave.java");
        assert!(BUTTONS.is_empty(), "pas un registerButton dans WindowGrave.java");
        assert_eq!(title_text(), TITLE);
        // Un littéral, pas une clé : la tombe n'est pas traduisible.
        assert!(!TITLE.starts_with("com.minecolonies."));
        assert!(!TITLE.contains('.'));
        // Le nom du bouton ne sert à personne ici : il n'y a pas de bouton.
        assert_eq!(Naming::Xml.name(), "xml");
        assert_eq!(Naming::Ours.name(), "ours");
        assert_eq!(Layout::grave(), Layout::of(54));
    }
}
