//! La terre compostée : elle fait pousser une fleur au-dessus d'elle, un temps.
//!
//! Référence Java : `core/tileentities/TileEntityCompostedDirt.java` (173 lignes).
//!
//! **Un compteur qui tourne même à l'arrêt.** `tick()` incrémente `ticker` à
//! chaque passage (`:75`), mais ne travaille que si la terre est compostée **et**
//! que `ticker % 20 == 0` (`:71`). La conséquence, réelle et reproduite : une
//! terre compostée reprend son compteur là où il s'était arrêté, pas à zéro, et
//! le délai avant qu'elle s'éteigne dépend du temps écoulé **avant** le
//! compostage.
//!
//! **Deux horloges qui ne tombent pas ensemble.** Le travail se fait sur le
//! `ticker % 20` (`:71`), et la limite est `ticker >= 300 × 20` (`:122-126`),
//! testée **dans** le travail : la terre s'arrête donc au premier multiple de 20
//! au-delà de 6 000, pas exactement à 6 000.
//!
//! **`compost` accepte tout pourcentage et n'écrit que les valides.** Java
//! n'écrit `percentage` et `flower` que si le pourcentage est dans 0..=100
//! (`:146-156`), mais pose `composted = true` **hors** du test (`:158`). Un
//! appelant qui passe 150 démarre donc une pousse avec l'ancien réglage. Le port
//! rend un booléen pour dire si le réglage a été pris, et il composte quand
//! même — corriger changerait le comportement d'un bloc existant.

use crate::inventory::ItemName;

/// L'identifiant d'enregistrement (`TileEntityInitializer.java:37`).
pub const TYPE_ID: &str = "minecolonies:composteddirt";

/// Le nombre de secondes avant que la terre ne s'éteigne (`:47`).
pub const TICKER_LIMIT: u32 = 300;
/// La même limite en ticks : `TICKER_LIMIT × TICKS_PER_SECOND` (`:122`).
pub const TICKER_LIMIT_TICKS: u32 = TICKER_LIMIT * super::TICKS_PER_SECOND;
/// La chance par seconde d'un compost frais (`:42`).
pub const DEFAULT_PERCENTAGE: f64 = 1.0;

/// Ce qu'un tick a fait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les quatre sorties de `tick` et `updateTick`
pub enum Growth {
    /// Pas un tick de travail : le compteur avance, rien d'autre.
    Idle,
    /// Tick de travail, mais pas de pousse cette fois (hasard, ou place
    /// occupée).
    Nothing,
    /// Une fleur a poussé au-dessus (`:99-119`).
    Grew,
    /// La terre s'arrête : plus de fleur (`:85-89`) ou limite atteinte
    /// (`:122-126`).
    Stopped,
}

/// L'état d'une terre compostée. Java ne sauve **aucun** de ces champs
/// (`saveAdditional` n'existe pas dans la classe) : tout est perdu au
/// rechargement, et le port le reproduit en ne sauvant rien.
#[derive(Clone, Debug, PartialEq)]
// exhaustif : les quatre champs de la classe Java
pub struct CompostedDirt {
    composted: bool,
    ticker: u32,
    percentage: f64,
    flower: Option<ItemName>,
}

impl Default for CompostedDirt {
    fn default() -> CompostedDirt {
        CompostedDirt {
            composted: false,
            ticker: 0,
            percentage: DEFAULT_PERCENTAGE,
            flower: None,
        }
    }
}

impl CompostedDirt {
    pub fn new() -> CompostedDirt {
        CompostedDirt::default()
    }

    pub fn is_composted(&self) -> bool {
        self.composted
    }

    pub fn ticker(&self) -> u32 {
        self.ticker
    }

    pub fn percentage(&self) -> f64 {
        self.percentage
    }

    pub fn flower(&self) -> Option<&ItemName> {
        self.flower.as_ref()
    }

    /// `compost(percentage, flower)` (`:144-160`). Rend `true` quand le réglage
    /// a été pris — le pourcentage est dans 0..=100 **et** la fleur est un nom
    /// d'objet valide —, et composte dans tous les cas, comme Java.
    pub fn compost(&mut self, percentage: f64, flower: Option<ItemName>) -> bool {
        let accepted = (0.0..=100.0).contains(&percentage) && flower.is_some();
        if (0.0..=100.0).contains(&percentage) {
            self.percentage = percentage;
            // Java enveloppe son affectation dans un try/catch qui imprime la
            // pile (`:149-155`) : le port refuse une fleur absente au lieu de
            // l'avaler, et `accepted` le dit.
            if flower.is_some() {
                self.flower = flower;
            }
        }
        self.composted = true;
        accepted
    }

    /// Le tick d'une terre compostée (`:69-76`), déroulé en entier.
    ///
    /// `above_is_air` est `world.getBlockState(pos.above()).getBlock() instanceof
    /// AirBlock` (`:102`) ; `draw` est `random.nextDouble()` (`:99`), fourni par
    /// l'appelant pour que la règle reste pure.
    pub fn advance(&mut self, above_is_air: bool, draw: f64) -> Growth {
        if !self.composted {
            // Java incrémente le compteur même à l'arrêt (`:75`), et sans
            // jamais le remettre à zéro : c'est ce qui fait qu'un compost
            // entamé plus tard s'éteint plus tôt.
            self.ticker = self.ticker.saturating_add(1);
            return Growth::Idle;
        }
        if self.ticker % super::TICKS_PER_SECOND != 0 {
            self.ticker = self.ticker.saturating_add(1);
            return Growth::Idle;
        }
        // Pas de fleur : Java s'arrête **avant** le test de limite (`:85-89` a
        // un `return`), donc le compteur n'est pas remis à zéro dans ce cas.
        if self.flower.is_none() {
            self.composted = false;
            self.ticker = self.ticker.saturating_add(1);
            return Growth::Stopped;
        }
        let mut outcome = Growth::Nothing;
        if draw * 100.0 <= self.percentage && above_is_air {
            outcome = Growth::Grew;
        }
        if self.ticker >= TICKER_LIMIT_TICKS {
            self.ticker = 0;
            self.composted = false;
            outcome = Growth::Stopped;
        }
        self.ticker = self.ticker.saturating_add(1);
        outcome
    }

    /// La probabilité, dite en pour cent par seconde, qu'un tick de travail
    /// fasse pousser — telle que le test de Java l'écrit
    /// (`draw * 100 <= percentage`).
    pub fn grows_on(&self, draw: f64) -> bool {
        draw * 100.0 <= self.percentage
    }
}

/// Les clés qu'un compound de terre compostée porte. **Aucune** : la classe Java
/// n'a pas de `saveAdditional`, donc rien n'est sauvé, et le port ne lit rien.
/// La fonction existe pour que l'absence soit dite une fois, et testée.
pub fn saved_keys() -> &'static [&'static str] {
    &[]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flower() -> ItemName {
        ItemName::new("minecraft:poppy").expect("nom valide")
    }

    #[test]
    fn une_terre_neuve_n_est_pas_compostee_et_ne_sauve_rien() {
        let dirt = CompostedDirt::new();
        assert!(!dirt.is_composted());
        assert_eq!(dirt.ticker(), 0);
        assert_eq!(dirt.percentage(), 1.0);
        assert_eq!(dirt.flower(), None);
        assert!(saved_keys().is_empty(), "aucun champ n'est sauvé");
    }

    #[test]
    fn un_compost_valide_prend_le_pourcentage_et_la_fleur() {
        let mut dirt = CompostedDirt::new();
        assert!(dirt.compost(50.0, Some(flower())));
        assert!(dirt.is_composted());
        assert_eq!(dirt.percentage(), 50.0);
        assert_eq!(dirt.flower(), Some(&flower()));
    }

    #[test]
    fn un_pourcentage_hors_bornes_n_est_pas_pris_mais_composte_quand_meme() {
        let mut dirt = CompostedDirt::new();
        dirt.compost(50.0, Some(flower()));
        assert!(!dirt.compost(150.0, Some(flower())), "hors de 0..=100");
        assert_eq!(dirt.percentage(), 50.0, "l'ancien réglage survit");
        assert!(dirt.is_composted(), "`composted = true` est hors du test en Java");
        // Et une fleur absente n'est pas un réglage non plus.
        let mut second = CompostedDirt::new();
        assert!(!second.compost(10.0, None));
        assert!(second.is_composted());
        assert_eq!(second.flower(), None);
    }

    #[test]
    fn le_travail_se_fait_un_tick_sur_vingt_et_le_compteur_avance_toujours() {
        let mut dirt = CompostedDirt::new();
        // À l'arrêt, chaque passage avance le compteur sans rien faire.
        for expected in 1..=5u32 {
            assert_eq!(dirt.advance(true, 0.0), Growth::Idle);
            assert_eq!(dirt.ticker(), expected);
        }
        dirt.compost(100.0, Some(flower()));
        assert_eq!(dirt.ticker(), 5, "le compost ne remet pas le compteur à zéro");
        // Tick 5 : pas un multiple de 20.
        assert_eq!(dirt.advance(true, 0.0), Growth::Idle);
        while dirt.ticker() < 20 {
            dirt.advance(true, 0.0);
        }
        assert_eq!(dirt.advance(true, 0.0), Growth::Grew, "tick 20 : il pousse");
        // La pousse ne change pas le compteur d'un cran de plus que la normale.
        assert_eq!(dirt.ticker(), 21, "le travail ne compte qu'un tick");
    }

    #[test]
    fn la_chance_se_compare_au_pourcentage_et_il_faut_de_la_place() {
        let mut dirt = CompostedDirt::new();
        dirt.compost(50.0, Some(flower()));
        assert_eq!(dirt.advance(true, 0.49), Growth::Grew, "49 % <= 50 %");
        let mut second = CompostedDirt::new();
        second.compost(10.0, Some(flower()));
        assert_eq!(second.advance(true, 0.11), Growth::Nothing, "11 % > 10 %");
        let mut third = CompostedDirt::new();
        third.compost(100.0, Some(flower()));
        assert_eq!(
            third.advance(false, 0.0),
            Growth::Nothing,
            "le bloc au-dessus n'est pas de l'air (`:102`)"
        );
    }

    #[test]
    fn une_terre_sans_fleur_s_arrete_au_premier_tick_de_travail() {
        let mut dirt = CompostedDirt::new();
        assert!(!dirt.compost(100.0, None));
        assert!(dirt.is_composted(), "Java pose `composted = true` quand même");
        assert_eq!(dirt.advance(true, 0.0), Growth::Stopped);
        assert!(!dirt.is_composted());
    }

    #[test]
    fn la_limite_de_secondes_s_arrete_au_premier_multiple_de_vingt_au_dela() {
        let mut dirt = CompostedDirt::new();
        dirt.compost(100.0, Some(flower()));
        let mut stopped_at = None;
        for tick in 0..TICKER_LIMIT_TICKS + 40 {
            if dirt.advance(true, 0.0) == Growth::Stopped {
                stopped_at = Some(tick as usize);
                break;
            }
        }
        assert_eq!(
            stopped_at,
            Some(TICKER_LIMIT_TICKS as usize),
            "6 000 est un multiple de 20 : c'est là que Java coupe (`:122`)"
        );
        assert!(!dirt.is_composted());
        assert_eq!(dirt.ticker(), 1, "le compteur repart de zéro puis avance d'un cran");
    }

    #[test]
    fn la_chance_par_seconde_se_lit_sur_le_tirage() {
        let mut dirt = CompostedDirt::new();
        dirt.compost(25.0, Some(flower()));
        assert!(dirt.grows_on(0.25), "25 % pile passe");
        assert!(!dirt.grows_on(0.2501));
    }
}
