//! Le drapeau de colonie : la bannière prend les motifs de la colonie.
//!
//! Référence Java : `core/tileentities/TileEntityColonyFlag.java` (143 lignes).
//!
//! **Deux champs, deux origines.** `patterns` (une `ListTag` de motifs) et
//! `colonyId` (un entier, `-1` pour aucun) (`:32-35`). Le premier **suit** la
//! colonie : `getPatterns` compare la liste locale à `colony.getColonyFlag()` et
//! la remplace si elles diffèrent (`:93-99`) — la colonie est la vérité, le bloc
//! n'en est qu'un cache. Le second se résout **une fois**, au chargement, quand
//! il vaut `-1` et que le bloc est posé (`:57-65`).
//!
//! **La couche de base n'est pas une donnée.** Java construit la liste complète
//! avec `DyeColor.WHITE` comme base puis retire la première entrée
//! (`:101-107`) : elle est implicite, et le port ne la stocke pas non plus.
//!
//! **Rien n'est un refus ici** : un drapeau sans colonie est un drapeau blanc,
//! pas une erreur. Les seules règles sont celles du cache et de l'adoption,
//! et elles rendent `bool` — « est-ce que j'ai changé quelque chose », ce que
//! Java teste pour décider d'un `setChanged()`.

use mod_api::{Compound, Tag};

use super::nbt;

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:44`).
pub const TYPE_ID: &str = "minecolonies:colony_flag";

/// La liste des motifs (`NbtTagConstants.java:473`).
pub const KEY_PATTERNS: &str = "Patterns";
/// L'identifiant de colonie (`NbtTagConstants.java:61`, `-1` par défaut à
/// `TileEntityColonyFlag.java:35`).
pub const KEY_COLONY: &str = "colony";
/// La valeur « aucune colonie » (`:35`).
pub const NO_COLONY: i32 = -1;
/// Les deux clés d'un motif dans un objet de bannière (`:126-127`).
pub const KEY_PATTERN: &str = "Pattern";
pub const KEY_COLOR: &str = "Color";
/// La couche de base implicite (`:102`).
pub const BASE_COLOR: i32 = 0;

/// Un motif de bannière : un nom de motif et un identifiant de couleur
/// (`:126-127`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustif : les deux champs d'un motif
pub struct BannerLayer {
    /// Le nom haché du motif (`BannerPattern.getHashname`), écrit par Java au
    /// format objet.
    pub pattern: String,
    /// L'identifiant de `DyeColor` (`DyeColor.getId`), `0` pour le blanc.
    pub color: i32,
}

impl BannerLayer {
    pub fn new(pattern: &str, color: i32) -> BannerLayer {
        BannerLayer {
            pattern: pattern.to_owned(),
            color,
        }
    }

    fn to_compound(&self) -> Compound {
        Compound::from([
            (KEY_PATTERN.into(), Tag::String(self.pattern.as_str().into())),
            (KEY_COLOR.into(), Tag::Int(self.color)),
        ])
    }

    fn from_compound(data: &Compound) -> Option<BannerLayer> {
        Some(BannerLayer {
            pattern: nbt::text(data, KEY_PATTERN)?,
            color: nbt::int(data, KEY_COLOR).unwrap_or(BASE_COLOR),
        })
    }
}

/// L'état d'un drapeau.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
// exhaustif : les deux champs de la classe Java
pub struct ColonyFlag {
    patterns: Vec<BannerLayer>,
    colony: Option<i32>,
}

impl ColonyFlag {
    pub fn new() -> ColonyFlag {
        ColonyFlag::default()
    }

    /// `load` (`:50-66`) : les motifs, l'identifiant, et `-1` ou une valeur
    /// absente sont la même chose — aucun lien.
    pub fn from_compound(data: &Compound) -> ColonyFlag {
        let patterns = nbt::compounds(data, KEY_PATTERNS)
            .into_iter()
            .filter_map(BannerLayer::from_compound)
            .collect();
        let colony = nbt::int(data, KEY_COLONY).filter(|id| *id != NO_COLONY && *id > 0);
        ColonyFlag { patterns, colony }
    }

    /// `saveAdditional` (`:40-47`) : les deux clés, l'identifiant écrit `-1`
    /// quand il n'y a pas de colonie.
    pub fn merge(&self) -> Compound {
        Compound::from([
            (
                KEY_PATTERNS.into(),
                nbt::compound_list(self.patterns.iter().map(BannerLayer::to_compound).collect()),
            ),
            (
                KEY_COLONY.into(),
                Tag::Int(self.colony.unwrap_or(NO_COLONY)),
            ),
        ])
    }

    /// Les motifs tels que le bloc les affiche (`getPatterns`, `:85-108`) :
    /// la liste sauvée, sans la couche de base qui est implicite.
    pub fn layers(&self) -> &[BannerLayer] {
        &self.patterns
    }

    pub fn colony(&self) -> Option<i32> {
        self.colony
    }

    /// L'adoption au chargement (`:57-65`) : seulement quand le bloc n'a **pas**
    /// de colonie, et rend `true` quand elle a changé — c'est le `setChanged()`
    /// de Java.
    pub fn adopt(&mut self, colony: Option<i32>) -> bool {
        if self.colony.is_some() {
            return false;
        }
        match colony.filter(|id| *id > 0) {
            Some(id) => {
                self.colony = Some(id);
                true
            }
            None => false,
        }
    }

    /// Le cache qui suit la colonie (`:93-99`) : si la colonie a des motifs et
    /// qu'ils sont différents, ils remplacent ceux du bloc.
    ///
    /// Rend `true` quand la liste a changé, ce qui est exactement la condition
    /// du `setChanged()` de Java. Une colonie sans motif ne touche à rien : le
    /// bloc garde les siens — une bannière blanche n'efface pas un blason.
    pub fn sync_from_colony(&mut self, colony: &[BannerLayer]) -> bool {
        if colony.is_empty() || colony == self.patterns.as_slice() {
            return false;
        }
        self.patterns = colony.to_vec();
        true
    }

    /// Poser les motifs localement (le blason qu'un joueur vient de choisir).
    pub fn set_layers(&mut self, layers: Vec<BannerLayer>) {
        self.patterns = layers;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blason() -> Vec<BannerLayer> {
        vec![BannerLayer::new("minecraft:base", 0), BannerLayer::new("mc:cross", 14)]
    }

    #[test]
    fn un_drapeau_neuf_n_a_ni_colonie_ni_motif() {
        let flag = ColonyFlag::new();
        assert_eq!(flag.colony(), None);
        assert!(flag.layers().is_empty());
        let merge = flag.merge();
        assert_eq!(nbt::int(&merge, KEY_COLONY), Some(NO_COLONY));
        assert_eq!(merge.len(), 2);
    }

    #[test]
    fn un_identifiant_absent_une_valeur_negative_ou_moins_un_valent_aucune_colonie() {
        for tag in [Tag::Int(NO_COLONY), Tag::Int(-9), Tag::Int(0)] {
            let data = Compound::from([(KEY_COLONY.into(), tag)]);
            assert_eq!(ColonyFlag::from_compound(&data).colony(), None);
        }
        let linked = Compound::from([(KEY_COLONY.into(), Tag::Int(4))]);
        assert_eq!(ColonyFlag::from_compound(&linked).colony(), Some(4));
        // Un octet écrit à la main se lit aussi.
        let byte = Compound::from([(KEY_COLONY.into(), Tag::Byte(4))]);
        assert_eq!(ColonyFlag::from_compound(&byte).colony(), Some(4));
    }

    #[test]
    fn les_motifs_font_un_aller_retour_avec_leur_couleur() {
        let mut flag = ColonyFlag::new();
        flag.set_layers(blason());
        let read = ColonyFlag::from_compound(&flag.merge());
        assert_eq!(read.layers(), &blason()[..]);
        assert_eq!(read.layers()[1].color, 14);
    }

    #[test]
    fn un_motif_sans_nom_est_saute_et_un_motif_sans_couleur_est_blanc() {
        let data = Compound::from([(
            KEY_PATTERNS.into(),
            nbt::compound_list(vec![
                Compound::from([(KEY_COLOR.into(), Tag::Int(3))]),
                Compound::from([(KEY_PATTERN.into(), Tag::String("mc:cross".into()))]),
            ]),
        )]);
        let flag = ColonyFlag::from_compound(&data);
        assert_eq!(flag.layers().len(), 1);
        assert_eq!(flag.layers()[0].color, BASE_COLOR);
    }

    #[test]
    fn l_adoption_ne_se_fait_qu_une_fois_et_seulement_sur_un_drapeau_sans_colonie() {
        let mut flag = ColonyFlag::new();
        assert!(flag.adopt(Some(3)));
        assert_eq!(flag.colony(), Some(3));
        assert!(!flag.adopt(Some(8)), "déjà rattaché");
        assert_eq!(flag.colony(), Some(3));
        let mut other = ColonyFlag::new();
        assert!(!other.adopt(None));
        assert!(!other.adopt(Some(0)));
        assert_eq!(other.colony(), None);
    }

    #[test]
    fn le_cache_suit_la_colonie_et_une_colonie_sans_motif_n_efface_rien() {
        let mut flag = ColonyFlag::new();
        flag.set_layers(vec![BannerLayer::new("minecraft:base", 0)]);
        assert!(flag.sync_from_colony(&blason()), "les motifs diffèrent");
        assert_eq!(flag.layers(), &blason()[..]);
        assert!(!flag.sync_from_colony(&blason()), "identiques : rien à faire");
        assert!(!flag.sync_from_colony(&[]), "une colonie blanche n'efface pas");
        assert_eq!(flag.layers().len(), 2);
    }

    #[test]
    fn la_couche_de_base_est_implicite() {
        // Java construit la bannière avec une base blanche puis retire la première
        // entrée (`:101-107`) : ce que le bloc sauve, ce sont les motifs **posés
        // sur** la base.
        assert_eq!(BASE_COLOR, 0);
        let flag = ColonyFlag::from_compound(&Compound::new());
        assert!(flag.layers().is_empty(), "aucune couche de base dans les données");
    }
}
