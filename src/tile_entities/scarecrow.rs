//! L'épouvantail : la taille du champ qu'il délimite, une valeur par direction.
//!
//! Référence Java : `core/tileentities/TileEntityScarecrow.java` (131 lignes) sur
//! `api/tileentities/AbstractTileEntityScarecrow.java` (44 lignes), plus les deux
//! bornes de `core/colony/buildingextensions/FarmField.java:40-41`.
//!
//! **Quatre rayons, dans l'ordre de `Direction`.** `get2DDataValue()` numérote
//! le plan horizontal **S, W, N, E** — c'est le commentaire de la classe
//! elle-même (`:41-46`), et c'est l'ordre du tableau. Une valeur rangée à
//! l'indice 0 est donc le rayon **sud**, pas le nord : le port nomme les côtés
//! pour que l'erreur ne soit pas possible.
//!
//! **Un seul côté est borné.** `setFieldSize` fait `Math.min(radius, MAX_RANGE)`
//! (`:106`) : aucun plancher, donc un rayon négatif est **rangé tel quel**. Le
//! port reproduit la règle et un test la fixe ; ajouter un plancher changerait
//! la valeur relue d'un bloc existant.
//!
//! **Le type d'épouvantail est tiré au hasard une fois.** `getScarecrowType`
//! tire parmi deux textures au premier appel puis garde le résultat
//! (`:58-67`) : ce n'est pas une donnée sauvée, seulement un choix de rendu.

use mod_api::Compound;

use super::nbt;

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:17`).
pub const TYPE_ID: &str = "minecolonies:scarecrow";

/// Le rayon par défaut, dans les quatre directions
/// (`FarmField.java:41`, `TileEntityScarecrow.java:47`).
pub const DEFAULT_RANGE: i32 = 5;
/// Le rayon maximal (`FarmField.java:40`).
pub const MAX_RANGE: i32 = 20;
/// La clé du tableau, partagée avec `FarmField` (`FarmField.java:44`, écrite à
/// `TileEntityScarecrow.java:87`).
pub const KEY_RADIUS: &str = "radius";

/// Les quatre côtés, **dans l'ordre de `Direction.get2DDataValue()`**
/// (`TileEntityScarecrow.java:41-46`) : sud, ouest, nord, est.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les quatre directions horizontales de vanilla
pub enum Side {
    South,
    West,
    North,
    East,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::South, Side::West, Side::North, Side::East];

    /// L'indice du tableau, `get2DDataValue()` (`:106`).
    pub fn index(self) -> usize {
        match self {
            Side::South => 0,
            Side::West => 1,
            Side::North => 2,
            Side::East => 3,
        }
    }

    pub fn from_index(index: usize) -> Option<Side> {
        Side::ALL.get(index).copied()
    }

    /// Le nom que le port écrit dans un rapport ou un message.
    pub fn name(self) -> &'static str {
        match self {
            Side::South => "south",
            Side::West => "west",
            Side::North => "north",
            Side::East => "east",
        }
    }

    /// Le vecteur unitaire du côté, sur le plan XZ, avec la convention de
    /// `crate::hut::facing_offset` (nord = -Z, est = +X).
    pub fn offset(self) -> [i32; 2] {
        match self {
            Side::South => [0, 1],
            Side::West => [-1, 0],
            Side::North => [0, -1],
            Side::East => [1, 0],
        }
    }
}

/// Les quatre rayons du champ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : quatre entiers, comme le tableau Java
pub struct FieldSize {
    radii: [i32; 4],
}

impl Default for FieldSize {
    fn default() -> FieldSize {
        FieldSize {
            radii: [DEFAULT_RANGE; 4],
        }
    }
}

impl FieldSize {
    pub fn new() -> FieldSize {
        FieldSize::default()
    }

    /// Le tableau sauvé (`:94-98`).
    ///
    /// **Écart nommé.** Java remplace son tableau par ce qu'il lit, quelle que
    /// soit la longueur ; un tableau de trois entrées ferait ensuite paniquer
    /// `setFieldSize` à l'indice 3 (`ArrayIndexOutOfBoundsException`). Le port
    /// garde les valeurs par défaut quand la longueur n'est pas 4, et le dit.
    pub fn from_compound(data: &Compound) -> FieldSize {
        match nbt::int_array(data, KEY_RADIUS) {
            Some(values) if values.len() == 4 => FieldSize {
                radii: [values[0], values[1], values[2], values[3]],
            },
            _ => FieldSize::new(),
        }
    }

    /// Ce qu'il faut fusionner pour sauver (`:85-88`) : un tableau de quatre
    /// entiers sous `radius`.
    pub fn merge(&self) -> Compound {
        Compound::from([(
            KEY_RADIUS.into(),
            nbt::int_array_tag(self.radii.to_vec()),
        )])
    }

    pub fn get(&self, side: Side) -> i32 {
        self.radii[side.index()]
    }

    /// `setFieldSize` (`:104-108`) : `min(radius, MAX_RANGE)`, **sans plancher**.
    /// Rend la valeur réellement rangée.
    pub fn set(&mut self, side: Side, radius: i32) -> i32 {
        let stored = radius.min(MAX_RANGE);
        self.radii[side.index()] = stored;
        stored
    }

    pub fn radii(&self) -> [i32; 4] {
        self.radii
    }

    /// La case couverte par ce côté, à partir de la position de l'épouvantail :
    /// `radius` blocs d'un côté et de l'autre sur un axe, `2 × radius + 1`
    /// blocs de large. Le port la rend en positions, pas en boîte : un champ
    /// n'est pas carré, puisqu'il a quatre rayons indépendants.
    pub fn span(&self, at: [i32; 3], side: Side) -> Vec<[i32; 3]> {
        let radius = self.get(side);
        let [dx, dz] = side.offset();
        let mut cells = Vec::new();
        if radius <= 0 {
            return cells;
        }
        for step in 1..=radius {
            cells.push([at[0] + dx * step, at[1], at[2] + dz * step]);
        }
        cells
    }
}

/// Les deux textures d'épouvantail (`ScareCrowType.java:8-10`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les deux valeurs de l'énumération Java
pub enum ScarecrowType {
    PumpkinHead,
    Normal,
}

impl ScarecrowType {
    pub const ALL: [ScarecrowType; 2] = [ScarecrowType::PumpkinHead, ScarecrowType::Normal];

    /// Le tirage de Java : `values[random.nextInt(values.length)]`
    /// (`:61-64`), donc un indice que l'appelant fournit.
    pub fn from_draw(draw: usize) -> ScarecrowType {
        ScarecrowType::ALL[draw % ScarecrowType::ALL.len()]
    }

    pub fn name(self) -> &'static str {
        match self {
            ScarecrowType::PumpkinHead => "PUMPKINHEAD",
            ScarecrowType::Normal => "NORMAL",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::Tag;

    #[test]
    fn un_epouvantail_neuf_couvre_cinq_blocs_dans_les_quatre_sens() {
        let field = FieldSize::new();
        assert_eq!(field.radii(), [5, 5, 5, 5]);
        for side in Side::ALL {
            assert_eq!(field.get(side), DEFAULT_RANGE);
        }
    }

    #[test]
    fn les_indices_sont_ceux_de_la_direction_vanilla() {
        // `get2DDataValue()` : S=0, W=1, N=2, E=3 (`:41-46`).
        assert_eq!(Side::South.index(), 0);
        assert_eq!(Side::West.index(), 1);
        assert_eq!(Side::North.index(), 2);
        assert_eq!(Side::East.index(), 3);
        for index in 0..4 {
            let side = Side::from_index(index).expect("quatre côtés");
            assert_eq!(side.index(), index);
            assert_eq!(Side::from_index(index + 4), None);
        }
        assert_eq!(Side::North.offset(), [0, -1], "nord = -Z");
        assert_eq!(Side::East.offset(), [1, 0]);
    }

    #[test]
    fn le_maximum_est_vingt_et_il_n_y_a_pas_de_plancher() {
        let mut field = FieldSize::new();
        assert_eq!(field.set(Side::North, 40), MAX_RANGE);
        assert_eq!(field.get(Side::North), 20);
        // Java n'a pas de `Math.max` : un rayon négatif est rangé tel quel.
        assert_eq!(field.set(Side::East, -3), -3);
        assert_eq!(field.get(Side::East), -3);
        // Les autres côtés ne bougent pas.
        assert_eq!(field.get(Side::South), DEFAULT_RANGE);
    }

    #[test]
    fn les_quatre_rayons_sauves_font_un_aller_retour() {
        let mut field = FieldSize::new();
        field.set(Side::South, 2);
        field.set(Side::West, 7);
        let read = FieldSize::from_compound(&field.merge());
        assert_eq!(read, field);
        assert_eq!(read.get(Side::South), 2);
        assert_eq!(read.get(Side::West), 7);
        // Un tableau d'une autre longueur est ignoré, et le défaut revient.
        let short = Compound::from([(KEY_RADIUS.into(), nbt::int_array_tag(vec![1, 2, 3]))]);
        assert_eq!(FieldSize::from_compound(&short), FieldSize::new());
        // Une autre sorte de tag aussi.
        let wrong = Compound::from([(KEY_RADIUS.into(), Tag::Int(4))]);
        assert_eq!(FieldSize::from_compound(&wrong), FieldSize::new());
    }

    #[test]
    fn un_cote_couvre_son_rayon_sans_toucher_au_centre() {
        let mut field = FieldSize::new();
        field.set(Side::North, 3);
        let cells = field.span([10, 64, 10], Side::North);
        assert_eq!(cells, vec![[10, 64, 9], [10, 64, 8], [10, 64, 7]]);
        assert!(!cells.contains(&[10, 64, 10]), "le centre n'est pas le champ");
        // Un rayon nul ou négatif ne couvre rien.
        let mut empty = FieldSize::new();
        empty.set(Side::East, 0);
        assert!(empty.span([0, 0, 0], Side::East).is_empty());
    }

    #[test]
    fn le_type_d_epouvantail_se_tire_parmi_deux() {
        assert_eq!(ScarecrowType::ALL.len(), 2);
        assert_eq!(ScarecrowType::from_draw(0), ScarecrowType::PumpkinHead);
        assert_eq!(ScarecrowType::from_draw(1), ScarecrowType::Normal);
        assert_eq!(ScarecrowType::from_draw(2), ScarecrowType::PumpkinHead);
        assert_eq!(ScarecrowType::PumpkinHead.name(), "PUMPKINHEAD");
    }
}
