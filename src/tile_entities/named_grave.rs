//! La tombe nommée : trois lignes de texte sur une plaque, et rien d'autre.
//!
//! Référence Java : `api/tileentities/AbstractTileEntityNamedGrave.java` (94
//! lignes). La classe de `core/` (`TileEntityNamedGrave.java`, 30 lignes) ne fait
//! que choisir le type d'enregistrement : tout est dans l'abstraite.
//!
//! **Une seule clé**, `content` (`:20`, écrite à `:78`), une liste de chaînes.
//! Java la découpe lui-même ligne par ligne sur le panneau
//! (`TileEntityColonyBuilding.java:478-495` le fait pour les panneaux de
//! bâtiment) ; ici les lignes sont les données, et l'affichage reste dehors.
//!
//! **Un défaut qui disparaît à la lecture.** Le constructeur pose
//! `"Unknown Citizen"` (`:37`), mais `load` **vide la liste** avant de lire
//! (`:56-64`) : une tombe sauvée sans `content` se relit donc **vide**, pas avec
//! le texte par défaut. Le port reproduit les deux (`new` et `from_compound`),
//! et un test le dit — c'est le genre d'écart qu'un port « propre » corrigerait
//! en silence et qui changerait un bloc existant.

use mod_api::Compound;

use super::nbt;

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:33`).
pub const TYPE_ID: &str = "minecolonies:namedgrave";

/// La clé de la liste de lignes (`NbtTagConstants.java:299`).
pub const KEY_CONTENT: &str = "content";
/// Le texte qu'une tombe neuve porte (`AbstractTileEntityNamedGrave.java:37`).
pub const DEFAULT_LINE: &str = "Unknown Citizen";
/// La propriété d'orientation du bloc (`:28`), nommée ici parce qu'elle décide
/// de quel côté la plaque se lit. C'est une propriété d'état, pas une donnée
/// sauvée.
pub const FACING: &str = "facing";

/// Les lignes d'une tombe nommée.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustif : une liste de lignes, rien d'autre
pub struct NamedGrave {
    lines: Vec<String>,
}

impl Default for NamedGrave {
    fn default() -> NamedGrave {
        NamedGrave {
            lines: vec![DEFAULT_LINE.to_owned()],
        }
    }
}

impl NamedGrave {
    /// Une tombe posée : une ligne, le texte par défaut (`:37`).
    pub fn new() -> NamedGrave {
        NamedGrave::default()
    }

    /// Ce que `load` lit : la liste est vidée d'abord (`:56`), donc une clé
    /// absente donne **zéro** ligne et non le défaut.
    pub fn from_compound(data: &Compound) -> NamedGrave {
        NamedGrave {
            lines: nbt::strings(data, KEY_CONTENT),
        }
    }

    /// Ce qu'il faut fusionner pour sauver (`:70-79`) : la liste entière, dans
    /// l'ordre.
    pub fn merge(&self) -> Compound {
        Compound::from([(KEY_CONTENT.into(), nbt::string_list(&self.lines))])
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// `setTextLines` (`:45-48`) : remplacer les lignes et marquer le bloc.
    pub fn set_lines(&mut self, lines: Vec<String>) {
        self.lines = lines;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::Tag;

    #[test]
    fn une_tombe_neuve_porte_une_ligne_par_defaut() {
        let grave = NamedGrave::new();
        assert_eq!(grave.lines(), &[DEFAULT_LINE.to_owned()]);
        assert_eq!(DEFAULT_LINE, "Unknown Citizen");
    }

    #[test]
    fn une_cle_absente_donne_zero_ligne_pas_le_defaut() {
        // `load` vide la liste avant de lire (`:56`) : c'est un écart avec le
        // constructeur, et il est volontaire.
        let read = NamedGrave::from_compound(&Compound::new());
        assert_eq!(read.lines().len(), 0);
        assert_ne!(read, NamedGrave::new());
    }

    #[test]
    fn les_lignes_font_un_aller_retour_dans_l_ordre() {
        let lines = vec![
            "Sir".to_owned(),
            "Robert".to_owned(),
            "de la Colonie".to_owned(),
        ];
        let mut grave = NamedGrave::new();
        grave.set_lines(lines.clone());
        let read = NamedGrave::from_compound(&grave.merge());
        assert_eq!(read.lines(), &lines[..]);
        assert_eq!(grave.merge().len(), 1, "une seule clé");
    }

    #[test]
    fn une_liste_d_une_autre_sorte_se_lit_comme_vide() {
        let wrong = Compound::from([(KEY_CONTENT.into(), Tag::Int(3))]);
        assert_eq!(NamedGrave::from_compound(&wrong).lines().len(), 0);
        // Une liste dont un élément n'est pas une chaîne garde les autres.
        let mixed = Compound::from([(
            KEY_CONTENT.into(),
            Tag::List(vec![Tag::String("A".into()), Tag::Int(1), Tag::String("B".into())]),
        )]);
        assert_eq!(
            NamedGrave::from_compound(&mixed).lines(),
            &["A".to_owned(), "B".to_owned()]
        );
    }

    #[test]
    fn l_orientation_est_une_propriete_de_bloc() {
        assert_eq!(FACING, "facing");
    }
}
