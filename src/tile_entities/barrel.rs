//! Le tonneau à compost : ce qu'il retient, quand il mûrit, et ce qu'il rend.
//!
//! Référence Java : `core/tileentities/TileEntityBarrel.java` (326 lignes) sur
//! `api/tileentities/AbstractTileEntityBarrel.java` (le seul champ partagé est
//! `MAX_ITEMS`).
//!
//! **Le compteur n'avance pas à chaque tick.** Java ne travaille que sur les
//! ticks où `getGameTime() % (random.nextInt(40) + 1) == 0`
//! (`TileEntityBarrel.java:69`) : un tick sur un à trente-neuf, tiré au hasard à
//! chaque passage, soit 20 en moyenne — d'où [`AVERAGE_TICKS`]. Le mûrissement
//! demande `TIMER_END / AVERAGE_TICKS` = **1 200 passages de ce tick échantillonné**
//! (`:101-107`), c'est-à-dire deux jours de jeu en moyenne.
//!
//! **Le facteur de force est une unité de compost, pas un objet.** Une recette
//! de force 4 fait entrer 4 unités par objet (`:163-171`), et Java retire
//! `unités / force` objets **arrondis vers le bas** (`:174`) : remplir 10 unités
//! avec une force de 4 coûte 2 objets et laisse 2 unités de rab. C'est reproduit
//! tel quel, et un test le dit, parce que corriger en silence changerait le
//! rendement du tonneau.
//!
//! **Ce qui n'est pas ici, et pourquoi.** Les particules `HAPPY_VILLAGER`
//! (`:90-96`) et le son `ITEM_FRAME_REMOVE_ITEM` (`:126`, `:135`) **existent**
//! dans le SDK (`level_send_particles` et `level_play_sound`,
//! `crates/horizon-mod-vocab/src/host/levels.rs:1219`, `:1201`) : ce module ne
//! fait aucun appel d'hôte, c'est donc l'appelant qui les émet, et ce n'est pas
//! une lacune. Ce qui en est une : la recette de compost vient de
//! `getCompatibilityManager()` (`:179-185`), un registre du jeu que le SDK
//! n'expose pas. `MANQUANT.md` porte cette ligne.

use mod_api::{Compound, Tag};

use super::nbt;

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:21`).
pub const TYPE_ID: &str = "minecolonies:barrel";

/// Ce qu'il faut d'unités de compost pour lancer la fermentation
/// (`AbstractTileEntityBarrel.java:14`).
pub const MAX_ITEMS: i32 = 64;
/// Durée de fermentation en ticks de jeu : deux jours (`TileEntityBarrel.java:50`).
pub const TIMER_END: i32 = 24_000;
/// Un tick de travail tous les 20 ticks de jeu en moyenne (`:54`, `:69`).
pub const AVERAGE_TICKS: i32 = 20;
/// Le nombre de ticks de travail, donc de passages, que `TIMER_END` demande
/// (`:101`) : `24 000 / 20 = 1 200`.
pub const TIMER_STEPS: i32 = TIMER_END / AVERAGE_TICKS;
/// Ce qu'une récolte donne, avant le multiplicateur du ramasseur (`:123`, `:322`).
pub const COMPOST_PER_HARVEST: f64 = 6.0;

/// Les trois clés sauvées (`:207-209`), lues avec les mêmes noms (`:216-218`).
pub const KEY_ITEMS: &str = "items";
pub const KEY_TIMER: &str = "timer";
pub const KEY_DONE: &str = "done";

/// Pourquoi le tonneau n'a pas pris ce qu'on lui tendait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : le jeu de refus du tonneau, fermé volontairement
pub enum Refusal {
    /// Il est déjà plein et fermente : Java le dit au joueur
    /// (`entity.barrel.working`, `:147-151`) et ne prend rien.
    Working,
    /// Une force de zéro ne compterait aucun objet et ferait une division par
    /// zéro (`:174`). Java ne peut pas la recevoir d'une recette ; un appelant
    /// du port, si.
    ZeroStrength,
}

impl Refusal {
    /// Le nom stable, pour un journal et pour un test. Ce n'est pas une clé de
    /// `lang` : `entity.barrel.working` en est une, et elle reste dans les
    /// ressources du paquet.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::Working => "tile_entities.barrel.working",
            Refusal::ZeroStrength => "tile_entities.barrel.zero_strength",
        }
    }
}

/// L'état du tonneau : trois champs, tous sauvés.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustif : les trois champs de la classe Java
pub struct Barrel {
    /// Les unités de compost accumulées, de 0 à [`MAX_ITEMS`].
    items: i32,
    /// Les ticks de travail déjà faits, de 0 à [`TIMER_STEPS`].
    timer: i32,
    /// Vrai quand la fermentation est finie et la récolte disponible.
    done: bool,
}

/// Ce qu'un remplissage a réellement fait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : deux quantités, et rien d'autre
pub struct Feed {
    /// Les unités de compost gagnées.
    pub units: i32,
    /// Les objets retirés de la main du joueur (`:174`).
    pub items_used: u32,
}

impl Barrel {
    /// Un tonneau neuf, vide et arrêté : Java laisse les trois champs à leur
    /// zéro et ne les écrit qu'à la sauvegarde (`:207-209`).
    pub fn new() -> Barrel {
        Barrel::default()
    }

    /// Les données d'un compound. Un champ absent vaut zéro, comme
    /// `getInt`/`getBoolean` sur une clé absente (`:216-218`) : un tonneau sauvé
    /// par une version qui ne connaissait pas `timer` se lit sans erreur.
    pub fn from_compound(data: &Compound) -> Barrel {
        Barrel {
            items: nbt::int(data, KEY_ITEMS).unwrap_or(0).clamp(0, MAX_ITEMS),
            timer: nbt::int(data, KEY_TIMER).unwrap_or(0).max(0),
            done: nbt::boolean(data, KEY_DONE).unwrap_or(false),
        }
    }

    /// Ce qu'il faut fusionner pour sauver l'état (`saveAdditional`, `:205-211`).
    /// Trois clés et pas une de plus : `merge_data` fusionne, il ne remplace pas
    /// (`.horizon/sdk/api/crates/horizon-mod-vocab/src/host/block_entities.rs:107-120`).
    pub fn merge(&self) -> Compound {
        Compound::from([
            (KEY_ITEMS.into(), Tag::Int(self.items)),
            (KEY_TIMER.into(), Tag::Int(self.timer)),
            (KEY_DONE.into(), nbt::boolean_tag(self.done)),
        ])
    }

    pub fn items(&self) -> i32 {
        self.items
    }

    pub fn timer(&self) -> i32 {
        self.timer
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    /// `checkIfWorking` (`:288-291`) : le tonneau fermente dès qu'il est plein.
    pub fn is_working(&self) -> bool {
        self.items == MAX_ITEMS
    }

    /// Le tonneau peut-il encore prendre quelque chose ?
    pub fn is_open(&self) -> bool {
        self.items < MAX_ITEMS
    }

    /// Ce tick est-il un tick de travail ? Java tire `1 + random(40)` puis teste
    /// `gameTime % ce_nombre == 0` (`:69`) ; `draw` est ce tirage, que l'appelant
    /// fournit pour que la règle reste pure.
    pub fn sample_due(game_time: u64, draw: u32) -> bool {
        draw != 0 && game_time % u64::from(draw) == 0
    }

    /// Un passage de `updateTick` (`:83-107`) : le compteur n'avance que si le
    /// tonneau est **plein**, et la fin de fermentation vide le tonneau.
    ///
    /// Rend `true` quand cet appel a terminé la fermentation, c'est-à-dire quand
    /// le bloc doit être resynchronisé (`updateBlock`, `:106`).
    pub fn advance_sample(&mut self) -> bool {
        if self.items != MAX_ITEMS {
            return false;
        }
        self.timer += 1;
        if self.timer >= TIMER_STEPS {
            self.timer = 0;
            self.items = 0;
            self.done = true;
            return true;
        }
        false
    }

    /// `consumeNeededItems` (`:159-176`) : `units = min(64 - items, objets × force)`,
    /// puis `items += units` et `objets retirés = units / force` (division
    /// entière, comme Java).
    ///
    /// Le tonneau refuse quand il est plein (`:147-151`) ou quand la force est
    /// nulle : deux refus nommés, jamais un `false`.
    pub fn feed(&mut self, count: u32, strength: u32) -> Result<Feed, Refusal> {
        if strength == 0 {
            return Err(Refusal::ZeroStrength);
        }
        if !self.is_open() {
            return Err(Refusal::Working);
        }
        let strength = i32::try_from(strength).unwrap_or(i32::MAX);
        let available = i32::try_from(count).unwrap_or(i32::MAX).saturating_mul(strength);
        let needed = MAX_ITEMS - self.items;
        let units = needed.min(available);
        self.items += units;
        Ok(Feed {
            units,
            items_used: u32::try_from(units / strength).unwrap_or(0),
        })
    }

    /// `retrieveCompost(multiplier)` (`:316-325`) : une récolte par fermentation,
    /// `6 × multiplicateur` objets, et le tonneau redevient vide.
    ///
    /// `None` quand il n'y a rien à récolter — c'est ce que Java rend comme
    /// `ItemStack.EMPTY`, nommé au lieu d'être un objet de compte zéro.
    pub fn harvest(&mut self, multiplier: f64) -> Option<u32> {
        if !self.done {
            return None;
        }
        self.done = false;
        // `(int) (6 * multiplier)` : un `as` sature, et une récolte négative
        // donne zéro objet plutôt qu'un compte impossible.
        Some((COMPOST_PER_HARVEST * multiplier).max(0.0) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_tonneau_neuf_est_vide_et_ecrit_trois_cles() {
        let barrel = Barrel::new();
        assert_eq!((barrel.items(), barrel.timer(), barrel.is_done()), (0, 0, false));
        assert!(!barrel.is_working());
        assert!(barrel.is_open());
        let merge = barrel.merge();
        assert_eq!(merge.len(), 3, "items, timer, done");
        assert_eq!(Barrel::from_compound(&merge), barrel);
    }

    #[test]
    fn un_compound_absent_ou_d_une_autre_largeur_se_lit_quand_meme() {
        assert_eq!(Barrel::from_compound(&Compound::new()), Barrel::new());
        let old = Compound::from([
            (KEY_ITEMS.into(), Tag::Byte(64)),
            (KEY_TIMER.into(), Tag::Short(12)),
            (KEY_DONE.into(), nbt::boolean_tag(true)),
        ]);
        let barrel = Barrel::from_compound(&old);
        assert_eq!(barrel.items(), 64);
        assert_eq!(barrel.timer(), 12);
        assert!(barrel.is_done());
        // Un `items` hors bornes (un `/data merge` à la main) est ramené à 64.
        let over = Compound::from([(KEY_ITEMS.into(), Tag::Int(9_999))]);
        assert_eq!(Barrel::from_compound(&over).items(), MAX_ITEMS);
    }

    #[test]
    fn un_objet_de_force_quatre_donne_quatre_unites() {
        let mut barrel = Barrel::new();
        let feed = barrel.feed(1, 4).expect("vide");
        assert_eq!(feed, Feed { units: 4, items_used: 1 });
        assert_eq!(barrel.items(), 4);
    }

    #[test]
    fn le_tonneau_ne_prend_que_ce_qui_lui_manque_et_garde_le_reste() {
        let mut barrel = Barrel::new();
        // 16 objets de force 4 = 64 unités : plein.
        assert_eq!(barrel.feed(16, 4).map(|f| f.units), Ok(64));
        assert!(barrel.is_working());
        // Le suivant est refusé, avec le message de Java.
        assert_eq!(barrel.feed(1, 4), Err(Refusal::Working));
        assert_eq!(Refusal::Working.name(), "tile_entities.barrel.working");
    }

    #[test]
    fn une_force_de_trois_laisse_un_rab_d_unite() {
        let mut barrel = Barrel::new();
        // 15 objets × 3 = 45 unités, il en manquait 64.
        assert_eq!(barrel.feed(15, 3).map(|f| f.units), Ok(45));
        // Encore 19 unités à trouver ; un objet de force 3 en donne 3.
        assert_eq!(barrel.feed(1, 3).map(|f| f.units), Ok(3));
        assert_eq!(barrel.items(), 48);
        // Et le dernier remplissage s'arrête pile à 64 : 16 unités pour 5 objets
        // (5×3 = 15 unités entières, mais `min(16, 15)` = 15, puis 15/3 = 5).
        let feed = barrel.feed(5, 3).expect("il reste de la place");
        assert_eq!(feed, Feed { units: 15, items_used: 5 });
        assert_eq!(barrel.items(), 63);
        // Un objet de plus : 1 unité, 1/3 = 0 objet retiré — le Java rend le même
        // résultat, l'unité est gratuite.
        let last = barrel.feed(1, 3).expect("unité manquante");
        assert_eq!(last, Feed { units: 1, items_used: 0 });
        assert_eq!(barrel.items(), 64);
    }

    #[test]
    fn une_force_nulle_est_un_refus_nomme_pas_une_division_par_zero() {
        let mut barrel = Barrel::new();
        assert_eq!(barrel.feed(64, 0), Err(Refusal::ZeroStrength));
        assert_ne!(Refusal::ZeroStrength.name(), Refusal::Working.name());
        assert_eq!(barrel.items(), 0);
    }

    #[test]
    fn un_tick_de_travail_est_un_tick_sur_le_tirage() {
        assert!(Barrel::sample_due(0, 1), "un tirage de 1 travaille à chaque tick");
        assert!(Barrel::sample_due(39, 39));
        assert!(!Barrel::sample_due(38, 39));
        assert!(!Barrel::sample_due(7, 0), "un tirage nul ne travaille jamais");
        // Sur 1 000 ticks et un tirage de 20, un tick sur vingt travaille.
        assert_eq!((0..1_000).filter(|t| Barrel::sample_due(*t, 20)).count(), 50);
    }

    #[test]
    fn un_tonneau_qui_n_est_pas_plein_ne_fermente_jamais() {
        let mut barrel = Barrel::new();
        barrel.feed(63, 1).expect("presque plein");
        for _ in 0..(TIMER_STEPS * 2) {
            assert!(!barrel.advance_sample());
        }
        assert_eq!(barrel.timer(), 0, "le compteur n'a pas bougé");
        assert!(!barrel.is_done());
    }

    #[test]
    fn mille_deux_cents_passages_pleins_terminent_la_fermentation() {
        let mut barrel = Barrel::new();
        barrel.feed(64, 1).expect("plein");
        for _ in 0..TIMER_STEPS - 1 {
            assert!(!barrel.advance_sample());
        }
        assert_eq!(barrel.timer(), TIMER_STEPS - 1);
        assert!(barrel.advance_sample(), "le 1 200e passage termine");
        assert_eq!((barrel.items(), barrel.timer(), barrel.is_done()), (0, 0, true));
    }

    #[test]
    fn une_recolte_par_fermentation_et_le_multiplicateur_s_applique() {
        let mut barrel = Barrel::new();
        assert_eq!(barrel.harvest(1.0), None, "rien à récolter");
        barrel.feed(64, 1).expect("plein");
        for _ in 0..TIMER_STEPS {
            barrel.advance_sample();
        }
        assert_eq!(barrel.harvest(1.0), Some(6));
        assert_eq!(barrel.harvest(1.0), None, "une seule récolte par fermentation");
        // Le multiplicateur du ramasseur (`retrieveCompost`), arrondi vers le bas.
        let mut second = Barrel::new();
        second.feed(64, 1).expect("plein");
        for _ in 0..TIMER_STEPS {
            second.advance_sample();
        }
        assert_eq!(second.harvest(1.5), Some(9));
        assert_eq!(second.harvest(-1.0), None, "déjà récolté");
    }
}
