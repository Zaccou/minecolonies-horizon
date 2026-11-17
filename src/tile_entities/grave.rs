//! La tombe : deux décompositions, puis le butin tombe et le bloc disparaît.
//!
//! Référence Java : `core/tileentities/TileEntityGrave.java` (259 lignes) sur
//! `api/tileentities/AbstractTileEntityGrave.java` (76 lignes).
//!
//! **Deux morts, pas une.** `onColonyTick(delay)` (`:228-250`) décrémente un
//! compteur ; quand il tombe à zéro la première fois, la tombe **se décompose**
//! — sa variante de bloc change (`:119-123`) et le compteur repart à
//! [`DECAY_TIMER`] —, et la seconde fois elle lâche son contenu puis disparaît
//! (`:243-245`). Un compteur à `-1` gèle la tombe pour toujours (`:230`) : c'est
//! ainsi qu'un joueur posé à la main garde son bloc.
//!
//! **Le compteur est un entier, le délai est un flottant.** `decay_timer -=
//! delay` sur un `int` tronque à chaque appel (`:232`) ; le port fait la même
//! troncature, et un test la fixe — un délai de 0,5 tick ne compte pas un demi
//! tick.
//!
//! **Ce qui est sauvé** : `decaytimer`, `decayed`, et `gravedata` quand il y en a
//! un (`:145-168`). Le contenu, lui, est l'inventaire du rack (`:54`, 54 slots :
//! `AbstractTileEntityGrave.java:44`), et le port réutilise
//! [`super::rack::Content`] au lieu d'une seconde table.
//!
//! **Ce qui n'est pas ici** : `GraveData` (`api/colony/GraveData.java`, le
//! citoyen mort, ses compétences, sa position) n'a pas de lecteur dans le port ;
//! le tag `gravedata` est donc **conservé tel quel**, jamais réécrit, jamais
//! interprété. `MANQUANT.md` porte la ligne.

use mod_api::{Compound, Tag};

use super::nbt;
use super::rack::Content;

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:31`).
pub const TYPE_ID: &str = "minecolonies:grave";

/// Les slots d'une tombe : deux fois la taille d'un rack
/// (`AbstractTileEntityGrave.java:44`).
pub const SLOTS: usize = super::rack::DEFAULT_SIZE * 2;
/// Le temps de décomposition, en ticks : dix minutes
/// (`AbstractTileEntityGrave.java:22`).
pub const DECAY_TIMER: i32 = 10 * 60 * 20;
/// Le compteur gelé (`TileEntityGrave.java:230`).
pub const FROZEN: i32 = -1;

/// Les clés sauvées (`:145-168`), et celle qu'on ne touche pas.
pub const KEY_DECAY_TIMER: &str = "decaytimer";
pub const KEY_DECAYED: &str = "decayed";
pub const KEY_GRAVE_DATA: &str = "gravedata";

/// Ce qu'un tick de colonie a fait à la tombe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les trois sorties de `onColonyTick`
pub enum Tick {
    /// Rien : le compteur tourne encore.
    Waiting,
    /// Le compteur est arrivé à zéro : la variante du bloc change
    /// (`:235-240`).
    Decayed,
    /// Elle était déjà décomposée : le butin tombe et le bloc devient de l'air
    /// (`:243-245`). `onColonyTick` rend `false` dans ce cas.
    Removed,
    /// La tombe est gelée (`decay_timer == -1`, `:230`).
    Frozen,
}

/// L'état sauvé d'une tombe, sans son contenu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les deux champs que ce fichier décide
pub struct Grave {
    /// Le compteur de décomposition, en ticks.
    timer: i32,
    /// Vrai après la première échéance (`:146`).
    decayed: bool,
}

impl Default for Grave {
    fn default() -> Grave {
        Grave {
            timer: DECAY_TIMER,
            decayed: false,
        }
    }
}

impl Grave {
    /// Une tombe neuve : le compteur plein, pas encore décomposée
    /// (`AbstractTileEntityGrave.java:46-49`).
    pub fn new() -> Grave {
        Grave::default()
    }

    /// Lectures de Java (`:145-146`) : une clé absente prend sa valeur par
    /// défaut, et `decayed` absent vaut faux.
    pub fn from_compound(data: &Compound) -> Grave {
        Grave {
            timer: nbt::int(data, KEY_DECAY_TIMER).unwrap_or(DECAY_TIMER),
            decayed: nbt::boolean(data, KEY_DECAYED).unwrap_or(false),
        }
    }

    /// Ce qu'il faut fusionner (`:161-167`) : deux clés, plus `gravedata` quand
    /// il y en a un. Le port n'en écrit jamais — il ne l'efface pas non plus,
    /// puisque `merge_data` fusionne.
    pub fn merge(&self) -> Compound {
        Compound::from([
            (KEY_DECAY_TIMER.into(), Tag::Int(self.timer)),
            (KEY_DECAYED.into(), nbt::boolean_tag(self.decayed)),
        ])
    }

    pub fn timer(&self) -> i32 {
        self.timer
    }

    pub fn is_decayed(&self) -> bool {
        self.decayed
    }

    pub fn is_frozen(&self) -> bool {
        self.timer == FROZEN
    }

    /// `delayDecayTimer(minutes)` (`AbstractTileEntityGrave.java:63-66`) :
    /// reculer l'échéance de N minutes.
    pub fn delay(&mut self, minutes: f64) {
        let ticks = minutes * f64::from(super::TICKS_PER_SECOND) * 60.0;
        self.timer = (f64::from(self.timer) + ticks) as i32;
    }

    /// `onColonyTick(delay)` (`:228-250`).
    ///
    /// La troncature de `decay_timer -= delay` est reproduite : Java affecte un
    /// `double` à un `int`, ce qui tronque vers zéro, et le port fait pareil.
    pub fn on_colony_tick(&mut self, delay: f64) -> Tick {
        if self.is_frozen() {
            return Tick::Frozen;
        }
        self.timer = (f64::from(self.timer) - delay) as i32;
        if self.timer > 0 {
            return Tick::Waiting;
        }
        if !self.decayed {
            self.decayed = true;
            self.timer = DECAY_TIMER;
            return Tick::Decayed;
        }
        Tick::Removed
    }

    /// Le contenu de la tombe, lu depuis les 54 slots (`updateContent`,
    /// `:92-112`) : la même agrégation qu'un rack, et le même type.
    pub fn content(slots: &[Option<crate::inventory::Stack>]) -> Content {
        Content::from_slots(slots)
    }

    /// `isEmpty` (`:134-138`) : la tombe est vide quand aucun slot ne porte
    /// d'objet.
    pub fn is_empty(slots: &[Option<crate::inventory::Stack>]) -> bool {
        Grave::content(slots).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ItemName, Stack};

    fn stack(name: &str, count: u32) -> Option<Stack> {
        Stack::new(ItemName::new(name).expect("nom valide"), count).ok()
    }

    #[test]
    fn une_tombe_neuve_compte_dix_minutes_et_n_est_pas_decomposee() {
        let grave = Grave::new();
        assert_eq!(grave.timer(), 12_000);
        assert_eq!(grave.timer(), DECAY_TIMER);
        assert!(!grave.is_decayed());
        assert!(!grave.is_frozen());
        assert_eq!(SLOTS, 54, "deux racks de 27");
        assert_eq!(grave.merge().len(), 2);
    }

    #[test]
    fn les_deux_cles_se_relisent_et_une_absence_prend_le_defaut() {
        assert_eq!(Grave::from_compound(&Compound::new()), Grave::new());
        let read = Grave::from_compound(&Compound::from([
            (KEY_DECAY_TIMER.into(), Tag::Int(40)),
            (KEY_DECAYED.into(), nbt::boolean_tag(true)),
        ]));
        assert_eq!(read.timer(), 40);
        assert!(read.is_decayed());
        // Une tombe gelée se relit gelée.
        let frozen = Grave::from_compound(&Compound::from([(KEY_DECAY_TIMER.into(), Tag::Int(-1))]));
        assert!(frozen.is_frozen());
    }

    #[test]
    fn un_delai_de_minutes_recule_l_echeance() {
        let mut grave = Grave::new();
        grave.delay(10.0);
        assert_eq!(grave.timer(), DECAY_TIMER * 2);
        grave.delay(-5.0);
        assert_eq!(grave.timer(), DECAY_TIMER + 6_000);
    }

    #[test]
    fn le_compteur_se_decompose_une_fois_puis_supprime_le_bloc() {
        let mut grave = Grave::new();
        assert_eq!(grave.on_colony_tick(6_000.0), Tick::Waiting);
        assert_eq!(grave.timer(), 6_000);
        assert_eq!(grave.on_colony_tick(6_000.0), Tick::Decayed);
        assert!(grave.is_decayed());
        assert_eq!(grave.timer(), DECAY_TIMER, "le compteur repart plein (`:238`)");
        // La seconde échéance retire le bloc : l'appelant lâche le butin et pose
        // de l'air (`:243-245`).
        assert_eq!(grave.on_colony_tick(12_000.0), Tick::Removed);
        assert!(grave.is_decayed(), "le drapeau ne revient pas en arrière");
    }

    #[test]
    fn une_tombe_gelee_ne_tombe_jamais() {
        let mut grave = Grave::from_compound(&Compound::from([(KEY_DECAY_TIMER.into(), Tag::Int(-1))]));
        for _ in 0..100 {
            assert_eq!(grave.on_colony_tick(1_000.0), Tick::Frozen);
        }
        assert_eq!(grave.timer(), FROZEN);
        assert!(!grave.is_decayed());
    }

    #[test]
    fn un_delai_fractionnaire_tronque_comme_l_affectation_java() {
        let mut grave = Grave::from_compound(&Compound::from([(KEY_DECAY_TIMER.into(), Tag::Int(10))]));
        assert_eq!(grave.on_colony_tick(0.5), Tick::Waiting);
        assert_eq!(grave.timer(), 9, "10 - 0,5 tronque à 9");
        assert_eq!(grave.on_colony_tick(0.9), Tick::Waiting);
        assert_eq!(grave.timer(), 8, "9 - 0,9 = 8,1 tronque à 8");
    }

    #[test]
    fn le_contenu_d_une_tombe_est_l_agregation_d_un_rack() {
        let mut slots = vec![stack("minecraft:bone", 4), stack("minecraft:bone", 2)];
        slots.extend(std::iter::repeat_n(None, SLOTS - 2));
        assert!(!Grave::is_empty(&slots));
        let content = Grave::content(&slots);
        assert_eq!(
            content.count(&ItemName::new("minecraft:bone").expect("nom valide")),
            6
        );
        assert!(Grave::is_empty(&vec![None; SLOTS]));
    }

    #[test]
    fn le_gravedata_absent_du_merge_survit_a_une_fusion() {
        let existing = Compound::from([
            (KEY_DECAY_TIMER.into(), Tag::Int(5)),
            (KEY_DECAYED.into(), nbt::boolean_tag(false)),
            (KEY_GRAVE_DATA.into(), Tag::compound([("name", Tag::String("Citizen".into()))])),
        ]);
        let mut merged = existing;
        merged.extend(Grave::new().merge());
        assert!(merged.get(&KEY_GRAVE_DATA.into()).is_some());
        assert_eq!(nbt::int(&merged, KEY_DECAY_TIMER), Some(DECAY_TIMER));
    }
}
