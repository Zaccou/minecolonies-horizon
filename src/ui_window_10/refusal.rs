//! Pourquoi les quatre fenêtres de ce module refusent — nommé.
//!
//! Java dit non de trois manières dans ces quatre fichiers et les trois sont ici : un
//! **panneau caché** (`WindowBarracksBuilding.java:92-96` cache le bouton des espions et
//! son icône sous le niveau 3), une **liste qui n'est jamais remplie**
//! (`:113-119` sort de `onOpened` sans poser de fournisseur de données, et
//! `WindowHutLiving.java:93` n'écrit rien quand le citoyen est introuvable), et un
//! **message au joueur** (`WindowHutLiving.java:117-121`).
//!
//! **Le rack n'a aucun refus à lui.** Il n'a ni bouton, ni panneau à id, ni gate
//! (`WindowRack.java:68-97` : un constructeur et des `blit`) ; ses deux seuls refus sont
//! ceux que ce module ajoute pour ses propres trous — un nombre de slots que la boucle de
//! `ContainerRack` ne peut pas placer, et une texture de fond dont le nom est calculé mais
//! dont le fichier n'existe pas.
//!
//! [`Refusal::Port`] reprend un refus de `src/ui` **tel quel**, exactement comme
//! `ui::action::Refusal::HutRefuses` reprend ceux de `src/jobs` : les deux tranches ne
//! peuvent pas diverger sur ce que « la hutte n'est pas construite » veut dire.

use std::fmt;

/// Un refus de ces quatre fenêtres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this slice's own closed set, plus the `src/ui` forward
pub enum Refusal {
    /// Un refus que `src/ui::Refusal` nomme déjà, repris tel quel.
    Port(crate::ui::Refusal),
    /// Une classe Java ou un id d'écran que [`crate::ui_window_10::WINDOWS`] ne porte pas.
    UnknownWindow,
    /// La liste des points de spawn n'est construite qu'à partir du niveau 3
    /// (`WindowBarracksBuilding.java:113`, `BUILDING_LEVEL_FOR_LIST` à `:57`). Java ne
    /// lève rien : `onOpened` sort simplement sans poser de fournisseur, donc la liste
    /// reste vide. Silencieux.
    SpawnListBelowLevel { level: i32 },
    /// `home.getColony().getCitizen(id)` a rendu `null` : Java n'écrit **rien** dans la
    /// ligne (`WindowHutLiving.java:92-96`), qui reste donc vide plutôt que de porter un
    /// nom inventé. Silencieux.
    UnknownResident { id: i32 },
    /// Le nom de texture que `getCorrectTextureForSlots` fabrique
    /// (`WindowRack.java:136-146`, `textures/gui/gui%s.png`) ne correspond à aucun
    /// fichier de `assets/minecolonies/textures/gui/` pour ce nombre de lignes. Java
    /// blitte alors une texture absente ; ici c'est un refus nommé.
    MissingRowTexture { rows: i32 },
    /// La boucle de `ContainerRack` ne place que `min(slots / 9, 8) * columns` slots
    /// (`ContainerRack.java:103-118`) : au-delà, des objets restent dans l'inventaire
    /// sans case pour les atteindre. Aucune taille réelle n'y tombe — 27 plus un
    /// multiple de 9 divise toujours juste —, mais la fonction est totale et le dit.
    UnreachableSlots { slots: i32 },
    /// Le rack n'enregistre **aucun** bouton (`WindowRack.java:68-97`) : demander une
    /// action par id est un refus, pas une absence silencieuse.
    RackTakesNoButton { id: &'static str },
    /// Le bloc du rack ne déclare pas la propriété `variant` dans
    /// [`crate::blocks`] (`decoration.rs`, `RACK_VARIANT`). Invariant statique : le test
    /// `the_rack_variants_are_the_block_table_s_own` le tient, donc ce refus ne doit
    /// jamais sortir — il existe pour que la lecture ne panique pas.
    RackVariantMissing,
}

impl Refusal {
    /// Un nom stable, préfixé pour ne jamais pouvoir être pris pour une phrase.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::Port(inner) => inner.name(),
            Refusal::UnknownWindow => "ui10.refusal.unknown_window",
            Refusal::SpawnListBelowLevel { .. } => "ui10.refusal.spawn_list_below_level",
            Refusal::UnknownResident { .. } => "ui10.refusal.unknown_resident",
            Refusal::MissingRowTexture { .. } => "ui10.refusal.missing_row_texture",
            Refusal::UnreachableSlots { .. } => "ui10.refusal.unreachable_slots",
            Refusal::RackTakesNoButton { .. } => "ui10.refusal.rack_takes_no_button",
            Refusal::RackVariantMissing => "ui10.refusal.rack_variant_missing",
        }
    }

    /// La clé de traduction que Java montre au joueur, quand il en montre une.
    ///
    /// Un seul cas de ces quatre fenêtres parle : `WindowHutLiving.java:119` envoie
    /// `COM_MINECOLONIES_COREMOD_GUI_WORKERHUTS_LEVEL_0` au joueur. Tout le reste est
    /// muet, et `None` veut dire « Java n'écrit rien » — pas « pas de raison ». Un
    /// appelant qui imprimerait [`Self::name`] à la place d'une phrase se tromperait de
    /// public.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Refusal::Port(inner) => inner.key(),
            Refusal::UnknownWindow
            | Refusal::SpawnListBelowLevel { .. }
            | Refusal::UnknownResident { .. }
            | Refusal::MissingRowTexture { .. }
            | Refusal::UnreachableSlots { .. }
            | Refusal::RackTakesNoButton { .. }
            | Refusal::RackVariantMissing => None,
        }
    }
}

impl From<crate::ui::Refusal> for Refusal {
    fn from(inner: crate::ui::Refusal) -> Refusal {
        Refusal::Port(inner)
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Port(inner) => write!(f, "{inner}"),
            other => f.write_str(other.name()),
        }
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Refusal as UiRefusal;

    #[test]
    fn only_the_forwarded_refusal_carries_a_sentence() {
        // `WindowHutLiving.java:117-121` est le seul endroit de ces quatre fenêtres qui
        // parle au joueur.
        let level0 = Refusal::from(UiRefusal::NotBuilt);
        assert_eq!(level0.key(), Some("com.minecolonies.coremod.gui.workerhuts.level0"));
        // Le bouton des espions est *caché*, pas refusé avec un texte (`:92-96`).
        assert_eq!(Refusal::from(UiRefusal::SpiesBelowLevel).key(), None);
        // Les refus propres à ce module sont tous muets.
        for refusal in [
            Refusal::UnknownWindow,
            Refusal::SpawnListBelowLevel { level: 2 },
            Refusal::UnknownResident { id: 7 },
            Refusal::MissingRowTexture { rows: 20 },
            Refusal::UnreachableSlots { slots: 70 },
            Refusal::RackTakesNoButton { id: "hire" },
            Refusal::RackVariantMissing,
        ] {
            assert_eq!(refusal.key(), None, "{refusal}");
            assert!(refusal.name().starts_with("ui10.refusal."), "{refusal}");
        }
    }

    #[test]
    fn a_forwarded_refusal_shares_the_name_of_the_one_it_forwards() {
        // La même règle que `ui::action::Refusal::HutRefuses` : deux tables, un seul nom.
        for inner in [UiRefusal::NotBuilt, UiRefusal::NoRoom, UiRefusal::SpiesBelowLevel] {
            assert_eq!(Refusal::from(inner).name(), inner.name());
            assert_eq!(format!("{}", Refusal::from(inner)), format!("{inner}"));
        }
        assert_ne!(Refusal::UnknownWindow.name(), Refusal::RackVariantMissing.name());
    }
}
