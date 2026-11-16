//! The 106 effect categories: what a research unlocks, and how strongly.
//!
//! Java reference: `core/research/ResearchEffectCategory.java` (120 lines) and
//! the declarations `researches/effects/*.json`.
//!
//! **A category is a ladder of levels, not a boolean.** Its JSON is
//! `{"effect": true, "levels": [...]}`; `ResearchEffectCategory` prepends a zero
//! to that list (`:55-63`), so the *absolute* strength at level `L` is
//! `levels[L-1]` and the *relative* strength the GUI shows is
//! `levels[L-1] - levels[L-2]` (`:65-84`). The two are different numbers and
//! a port that shipped one as the other would be off by the whole previous rung.
//!
//! **`levels` is optional, and 44 of the 106 files omit it.** Java substitutes
//! `[1]` (`core/datalistener/ResearchListener.java:39-44`), so a `&[1.0]` row
//! below is either an explicit `[1.0]` or that default: both mean "level 1 is
//! worth 1.0", which is how the 40-odd `...unlock` effects say "unlocked".
//!
//! **The ladder lengths are not uniform.** 20 categories have five rungs, 6
//! have four, 2 have six, 5 have two, 2 have three, and 71 have one. A research
//! that asks for a level above its category's top is *dropped* by Java with a
//! warning (`ResearchListener.java:437-443`); [`EffectCategory::strength`]
//! refuses it with [`LevelError::AboveMax`] instead, and [`super::tree`] refuses
//! the row at load, which is the same decision taken one step earlier.
//!
//! **`hut` effects carry a building level, not a bonus.** `blockhutlibrary` has
//! `levels: [5.0]` and every research that grants it asks for level 1: the
//! strength is what the *unlocked hut* may reach, and the port keeps the number
//! without interpreting it, because interpreting it belongs to the buildings
//! module.
//!
//! **Each row names its own file.** `id` is the path after
//! `minecolonies:effects/`, so the row `air` is `researches/effects/air.json`
//! under `src/datagen/generated/minecolonies/data/minecolonies/`; a per-row
//! `source` field on 106 rows would only repeat that.

/// One effect category: its short id and its ladder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectCategory {
    /// The path after `minecolonies:effects/`, e.g. `levelingmultiplier`.
    pub id: &'static str,
    /// `levels`, with Java's implicit `[1]` made explicit.
    pub levels: &'static [f64],
}

/// Why an effect has no strength at that level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one rule, one reason
pub enum LevelError {
    /// The category's ladder stops below this level
    /// (`ResearchListener.java:437-443`).
    AboveMax { level: u8, max: u8 },
}

impl LevelError {
    /// A stable name for logs and tests, namespaced like a registry entry so it
    /// cannot be mistaken for a translated sentence.
    pub fn name(self) -> &'static str {
        match self {
            LevelError::AboveMax { .. } => "minecolonies:research/effect-level-above-max",
        }
    }
}

impl std::fmt::Display for LevelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LevelError::AboveMax { level, max } => {
                write!(formatter, "{}: level {level} is above {max}", self.name())
            }
        }
    }
}

impl EffectCategory {
    /// The highest level that exists: `levelsAbsolute.size() - 1`, i.e. the
    /// number of rungs (`ResearchEffectCategory.java:86-89`).
    pub fn max_level(&self) -> u8 {
        // The data's longest ladder is 6, and a `u8` holds 255.
        u8::try_from(self.levels.len()).unwrap_or(u8::MAX)
    }

    /// The absolute strength at `level`: `levels[level - 1]`, and `0.0` at level
    /// 0, which is the zero `ResearchEffectCategory`'s constructor prepends
    /// (`:55-63`). Above the last rung it is [`LevelError::AboveMax`], where Java
    /// logs and drops the effect.
    pub fn strength(&self, level: u8) -> Result<f64, LevelError> {
        if level == 0 {
            return Ok(0.0);
        }
        let index = usize::from(level) - 1;
        self.levels
            .get(index)
            .copied()
            .ok_or(LevelError::AboveMax {
                level,
                max: self.max_level(),
            })
    }

    /// The relative strength at `level`: this rung minus the one below
    /// (`:65-74`). Level 1 is `levels[0] - 0`, because the zero rung is real.
    pub fn display(&self, level: u8) -> Result<f64, LevelError> {
        let here = self.strength(level)?;
        let below = if level <= 1 {
            0.0
        } else {
            self.strength(level - 1)?
        };
        Ok(here - below)
    }
}

/// The category a short id names, or `None`.
pub fn category(id: &str) -> Option<&'static EffectCategory> {
    EFFECTS.iter().find(|category| category.id == id)
}

/// The 106 declarations, in file-name order.
pub const EFFECTS: &[EffectCategory] = &[
    EffectCategory { id: "air", levels: &[1.0] },
    EffectCategory { id: "archerarmormultiplier", levels: &[0.05, 0.1, 0.25, 0.5, 1.0] },
    EffectCategory { id: "archerdamageaddition", levels: &[0.5, 1.0, 1.5, 2.0, 4.0] },
    EffectCategory { id: "armordurabilitymultiplier", levels: &[0.1, 0.2, 0.3, 0.4, 0.5, 1.0] },
    EffectCategory { id: "assistanthammerunlock", levels: &[1.0] },
    EffectCategory { id: "beekeep2unlock", levels: &[1.0] },
    EffectCategory { id: "blockattacksmultiplier", levels: &[0.05, 0.1, 0.25, 0.5] },
    EffectCategory { id: "blockbreakspeedmultiplier", levels: &[0.1, 0.25, 0.5, 1.0, 2.0] },
    EffectCategory { id: "blockhutalchemist", levels: &[5.0] },
    EffectCategory { id: "blockhutarchery", levels: &[5.0] },
    EffectCategory { id: "blockhutbarracks", levels: &[5.0] },
    EffectCategory { id: "blockhutblacksmith", levels: &[5.0] },
    EffectCategory { id: "blockhutcombatacademy", levels: &[5.0] },
    EffectCategory { id: "blockhutcomposter", levels: &[5.0] },
    EffectCategory { id: "blockhutconcretemixer", levels: &[5.0] },
    EffectCategory { id: "blockhutcrusher", levels: &[5.0] },
    EffectCategory { id: "blockhutdyer", levels: &[5.0] },
    EffectCategory { id: "blockhutfletcher", levels: &[5.0] },
    EffectCategory { id: "blockhutflorist", levels: &[5.0] },
    EffectCategory { id: "blockhutglassblower", levels: &[5.0] },
    EffectCategory { id: "blockhutgraveyard", levels: &[5.0] },
    EffectCategory { id: "blockhuthospital", levels: &[5.0] },
    EffectCategory { id: "blockhutlibrary", levels: &[5.0] },
    EffectCategory { id: "blockhutmechanic", levels: &[5.0] },
    EffectCategory { id: "blockhutmysticalsite", levels: &[5.0] },
    EffectCategory { id: "blockhutnetherworker", levels: &[5.0] },
    EffectCategory { id: "blockhutplantation", levels: &[5.0] },
    EffectCategory { id: "blockhutsawmill", levels: &[5.0] },
    EffectCategory { id: "blockhutschool", levels: &[5.0] },
    EffectCategory { id: "blockhutsifter", levels: &[5.0] },
    EffectCategory { id: "blockhutsmeltery", levels: &[5.0] },
    EffectCategory { id: "blockhutstable", levels: &[5.0] },
    EffectCategory { id: "blockhutstonemason", levels: &[5.0] },
    EffectCategory { id: "blockhutstonesmeltery", levels: &[5.0] },
    EffectCategory { id: "blockplacespeedmultiplier", levels: &[0.1, 0.25, 0.5, 1.0, 2.0] },
    EffectCategory { id: "buildermodeunlock", levels: &[1.0] },
    EffectCategory { id: "citizencapaddition", levels: &[25.0, 50.0, 100.0, 150.0, 500.0] },
    EffectCategory { id: "citizeninvslotsaddition", levels: &[9.0, 18.0, 27.0] },
    EffectCategory { id: "consumearrowsunlock", levels: &[1.0] },
    EffectCategory { id: "consumepotions", levels: &[1.0] },
    EffectCategory { id: "crushing11unlock", levels: &[1.0] },
    EffectCategory { id: "doublearrowsmultiplier", levels: &[0.05, 0.1, 0.25, 0.5] },
    EffectCategory { id: "enhancesgatedurabilityaddition", levels: &[5.0, 15.0] },
    EffectCategory { id: "farmingmultiplier", levels: &[0.1, 0.25, 0.5, 0.75, 2.0] },
    EffectCategory { id: "fishingtreasure", levels: &[1.0] },
    EffectCategory { id: "fleeingdamagemultiplier", levels: &[0.2, 0.3, 0.4, 0.75] },
    EffectCategory { id: "fleeingspeedaddition", levels: &[0.0, 1.0, 2.0, 3.0] },
    EffectCategory { id: "gravedecaybonus", levels: &[5.0, 10.0] },
    EffectCategory { id: "greenrevolution", levels: &[1.0] },
    EffectCategory { id: "growthmultiplier", levels: &[0.05, 0.1, 0.25, 0.5, 1.0] },
    EffectCategory { id: "guardcrit", levels: &[0.2, 0.3, 0.4, 0.5] },
    EffectCategory { id: "happinessmultiplier", levels: &[0.05, 0.1, 0.15, 0.2, 0.5] },
    EffectCategory { id: "healingsaturationlimitaddition", levels: &[-0.5, -1.0, -1.5, -2.0, -5.0] },
    EffectCategory { id: "healthaddition", levels: &[2.0, 4.0, 6.0, 8.0, 10.0, 20.0] },
    EffectCategory { id: "huscarl", levels: &[1.0] },
    EffectCategory { id: "knighttauntmobsunlock", levels: &[1.0] },
    EffectCategory { id: "knowledgeofthedepthsunlock", levels: &[1.0] },
    EffectCategory { id: "knowledgeoftheendunlock", levels: &[1.0] },
    EffectCategory { id: "levelingmultiplier", levels: &[0.05, 0.1, 0.25, 0.5, 1.0] },
    EffectCategory { id: "looting", levels: &[1.0] },
    EffectCategory { id: "marksman", levels: &[1.0] },
    EffectCategory { id: "masks", levels: &[1.0] },
    EffectCategory { id: "meleearmormultiplier", levels: &[0.05, 0.1, 0.25, 0.5, 1.0] },
    EffectCategory { id: "meleedamageaddition", levels: &[0.5, 1.0, 1.5, 2.0, 4.0] },
    EffectCategory { id: "min_order", levels: &[1.0] },
    EffectCategory { id: "minerfireresunlock", levels: &[1.0] },
    EffectCategory { id: "minimumstockmultiplier", levels: &[0.5, 1.0, 2.0] },
    EffectCategory { id: "moreoresmultiplier", levels: &[0.1, 0.25, 0.5, 1.0, 2.0] },
    EffectCategory { id: "morescrollsunlock", levels: &[1.0] },
    EffectCategory { id: "mountarmor", levels: &[0.15, 0.3, 0.45, 0.6, 0.75] },
    EffectCategory { id: "mounthealth", levels: &[0.2, 0.4, 0.6, 0.8, 1.0] },
    EffectCategory { id: "netherexpeditionlog", levels: &[1.0] },
    EffectCategory { id: "piercingarrowsunlock", levels: &[1.0] },
    EffectCategory { id: "plantationexotic", levels: &[1.0] },
    EffectCategory { id: "plantationjungle", levels: &[1.0] },
    EffectCategory { id: "plantationlarge", levels: &[1.0] },
    EffectCategory { id: "plantationnether", levels: &[1.0] },
    EffectCategory { id: "plantationsea", levels: &[1.0] },
    EffectCategory { id: "platearmorunlock", levels: &[1.0] },
    EffectCategory { id: "podzolchancemultiplier", levels: &[1.0, 2.0] },
    EffectCategory { id: "railsunlock", levels: &[1.0] },
    EffectCategory { id: "recipemodeunlock", levels: &[1.0] },
    EffectCategory { id: "recipesmultiplier", levels: &[0.25, 0.5, 1.0, 2.0] },
    EffectCategory { id: "regenerationmultiplier", levels: &[0.1, 0.25, 0.5, 1.0, 2.0] },
    EffectCategory { id: "resurrectchanceaddition", levels: &[0.01, 0.03] },
    EffectCategory { id: "retreatunlock", levels: &[1.0] },
    EffectCategory { id: "saturationmultiplier", levels: &[0.1, 0.15, 0.2, 0.25, 0.3] },
    EffectCategory { id: "shieldusageunlock", levels: &[1.0] },
    EffectCategory { id: "sifterdiamondunlock", levels: &[1.0] },
    EffectCategory { id: "sifterflintunlock", levels: &[1.0] },
    EffectCategory { id: "sifterironunlock", levels: &[1.0] },
    EffectCategory { id: "sifterstringunlock", levels: &[1.0] },
    EffectCategory { id: "sleeplessmultiplier", levels: &[1.0] },
    EffectCategory { id: "softshoesunlock", levels: &[1.0] },
    EffectCategory { id: "standard", levels: &[1.0] },
    EffectCategory { id: "teachingmultiplier", levels: &[0.05, 0.1, 0.25, 0.5, 1.0] },
    EffectCategory { id: "telescope", levels: &[1.0] },
    EffectCategory { id: "tooldurabilitymultiplier", levels: &[0.05, 0.1, 0.25, 0.5, 0.9] },
    EffectCategory { id: "undertakerrun", levels: &[1.0] },
    EffectCategory { id: "usetotemunlock", levels: &[1.0] },
    EffectCategory { id: "vaccines", levels: &[1.0] },
    EffectCategory { id: "vinesunlock", levels: &[1.0] },
    EffectCategory { id: "walkingmultiplier", levels: &[0.05, 0.1, 0.15, 0.2, 0.25] },
    EffectCategory { id: "whirlwindabilityunlock", levels: &[1.0] },
    EffectCategory { id: "workingdayhaddition", levels: &[1.0, 2.0] },
    EffectCategory { id: "workinginrainunlock", levels: &[1.0] },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_106_categories_and_every_id_is_unique() {
        assert_eq!(EFFECTS.len(), 106);
        let mut ids: Vec<&str> = EFFECTS.iter().map(|effect| effect.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), EFFECTS.len(), "a duplicate effect id");
    }

    #[test]
    fn the_ladder_lengths_are_the_ones_the_files_have() {
        let mut histogram = [0usize; 7];
        for effect in EFFECTS {
            assert!(effect.levels.len() <= 6, "{} rungs", effect.id);
            histogram[effect.levels.len()] += 1;
        }
        // 71 one-rung categories, 5 two, 2 three, 6 four, 20 five, 2 six.
        assert_eq!(histogram, [0, 71, 5, 2, 6, 20, 2]);
        assert_eq!(histogram.iter().sum::<usize>(), 106);
    }

    #[test]
    fn level_one_is_the_first_rung_and_level_zero_is_the_zero_rung() {
        let levelling = category("levelingmultiplier").expect("declared");
        assert_eq!(levelling.levels, &[0.05, 0.1, 0.25, 0.5, 1.0]);
        assert_eq!(levelling.max_level(), 5);
        assert_eq!(levelling.strength(0), Ok(0.0));
        assert_eq!(levelling.strength(1), Ok(0.05));
        assert_eq!(levelling.strength(5), Ok(1.0));
        assert_eq!(
            levelling.strength(6),
            Err(LevelError::AboveMax { level: 6, max: 5 })
        );
    }

    #[test]
    fn display_is_this_rung_minus_the_one_below() {
        let levelling = category("levelingmultiplier").expect("declared");
        assert_eq!(levelling.display(1), Ok(0.05));
        assert_eq!(levelling.display(2), Ok(0.05));
        assert_eq!(levelling.display(3), Ok(0.15));
        assert_eq!(levelling.display(5), Ok(0.5));
        // The rungs add back up to the last absolute value.
        let sum: f64 = (1..=5).filter_map(|l| levelling.display(l).ok()).sum();
        assert!((sum - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_one_rung_category_is_one_value() {
        let air = category("air").expect("declared");
        assert_eq!(air.levels, &[1.0]);
        assert_eq!(air.max_level(), 1);
        assert_eq!(air.strength(1), Ok(1.0));
        assert_eq!(air.strength(2), Err(LevelError::AboveMax { level: 2, max: 1 }));
        assert_eq!(air.display(1), Ok(1.0));
    }

    #[test]
    fn a_six_rung_ladder_exists_and_reads_to_the_end() {
        let armor = category("armordurabilitymultiplier").expect("declared");
        assert_eq!(armor.max_level(), 6);
        assert_eq!(armor.strength(6), Ok(1.0));
        assert_eq!(armor.display(6), Ok(0.5));
    }

    #[test]
    fn the_hut_effects_carry_the_level_the_hut_may_reach() {
        for hut in [
            "blockhutlibrary",
            "blockhutsmeltery",
            "blockhutgraveyard",
            "blockhutsifter",
        ] {
            let effect = category(hut).unwrap_or_else(|| panic!("{hut} is declared"));
            assert_eq!(effect.levels, &[5.0], "{hut}");
        }
        // There is no `blockhutbuilder`: the builder's hut is the first one a
        // colony gets, so no research unlocks it.
        assert!(category("blockhutbuilder").is_none());
    }

    #[test]
    fn an_unknown_effect_is_absent_not_empty() {
        assert!(category("no_such_effect").is_none());
        assert!(category("").is_none());
        assert!(category("effects/air").is_none());
    }

    #[test]
    fn the_refusal_names_itself() {
        let error = LevelError::AboveMax { level: 3, max: 1 };
        assert_eq!(error.name(), "minecolonies:research/effect-level-above-max");
        assert_eq!(
            error.to_string(),
            "minecolonies:research/effect-level-above-max: level 3 is above 1"
        );
    }
}
