//! The marks a citizen's AI sets, and what they make the model do.
//!
//! Java reference: `AbstractEntityCitizen.getRenderMetadata`/`setRenderMetadata`
//! (`:441-495`), the `RENDER_META_*` constants of seventeen worker AIs,
//! `CitizenModel.setupAnim` (`:34-75`), `displayHat` (`:114-121`), the
//! `getActualRotation` override of the courier, `core/client/render/RenderUtils.java`
//! (the arm pose) and `RenderBipedCitizen.setupMainModelFrom` (`:85-99`).
//!
//! **The states are strings, not an enum, and matching is a substring test.**
//! A worker appends its marks to one string — `renderMeta += RENDER_META_BOWL`
//! (`EntityAIWorkCowboy.java:85`) — and the model tests `contains`
//! (`CitizenModel.java:104-108`). So the port keeps a `&str` of marks and does the
//! same `contains`, instead of inventing a set that Java does not have. It is
//! only safe because no key is a substring of another; a test states that.
//!
//! **Seventeen marks, and one of them is written but never read.** [`MARKS`] is
//! every `RENDER_META_*` constant in the tree; [`SHOWN`] lists the parts a model
//! actually toggles. `bowl` appears only in the writer (`:45`, `:85`) — the cow
//! herder model has no bowl, so that mark is dead today.
//!
//! **Four gates, and they are not the same question.**
//!
//! | Gate | Java | Here |
//! | --- | --- | --- |
//! | at work | `isWorking` = marks contain `working` | [`is_working`] |
//! | may wear a hat | not sleeping, head slot empty, no custom skin | [`display_hat`] |
//! | asleep | `Pose.SLEEPING` | [`Condition::NotSleeping`] |
//! | carrying | a mark, plus sometimes the two above | [`Condition`] |
//!
//! And **portage moves the body**: the courier's `getActualRotation` answers
//! `0.1745` rad (10°) while its backpack shows (`FemaleCourierModel.java:68`), and
//! `CitizenModel.setupAnim` applies it to `body.xRot` and `head.xRot` when those
//! are still 0 (`:36-44`). A loaded courier leans forward; a loaded courier that
//! sleeps does not.

/// `CitizenModel.RENDER_META_WORKING` (`:22`), the same string the worker AI
/// writes (`AbstractEntityAIInteract.java:44`).
pub const WORKING: &str = "working";

/// One `RENDER_META_*` constant: the string, who writes it, and where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: four strings, all compared by the tests
pub struct Mark {
    /// The value the AI appends to the render metadata.
    pub key: &'static str,
    /// The AI that writes it, and what being there means.
    pub written_by: &'static str,
    /// `fichier.java:ligne` of the constant.
    pub source: &'static str,
    /// The model part it turns on, or `""` when no model reads it.
    pub shown_as: &'static str,
}

/// Every `RENDER_META_*` constant in the tree, in key order.
pub const MARKS: &[Mark] = &[
    Mark { key: "arrow", written_by: "an archer guard with an arrow out", source: "EntityAIRange.java:25", shown_as: "quiver (FemaleArcherModel.java:87)" },
    Mark { key: "backpack", written_by: "a courier carrying something", source: "EntityAIWorkDeliveryman.java:93", shown_as: "backpack, and the body lean (FemaleCourierModel.java:68-77)" },
    Mark { key: "book", written_by: "a student holding a book", source: "EntityAIStudy.java:39", shown_as: "right arm's book (FemaleStudentModel.java:97)" },
    Mark { key: "bowl", written_by: "a cowhand carrying a bowl", source: "EntityAIWorkCowboy.java:45", shown_as: "" },
    Mark { key: "bucket", written_by: "a cowhand carrying a bucket", source: "EntityAIWorkCowboy.java:44", shown_as: "bucketwithHandle (FemaleCowHerderModel.java:85)" },
    Mark { key: "carrot", written_by: "a rabbit or swine herder carrying carrots", source: "EntityAIWorkRabbitHerder.java:25", shown_as: "carrotBag (FemaleRabbitHerderModel.java:95, FemaleSwineHerderModel.java:105)" },
    Mark { key: "fish", written_by: "a fisher with a fish", source: "EntityAIWorkFisherman.java:70", shown_as: "fish (FemaleFisherModel.java:105)" },
    Mark { key: "flowers", written_by: "a florist with flowers", source: "EntityAIWorkFlorist.java:101", shown_as: "WoodsmanHat/flowers (FemaleFloristModel.java:94)" },
    Mark { key: "logs", written_by: "a forester carrying logs", source: "EntityAIWorkLumberjack.java:67", shown_as: "LogPack/Logs (FemaleForesterModel.java:103)" },
    Mark { key: "pickaxe", written_by: "a miner whose pickaxe shows", source: "EntityAIStructureMiner.java:98", shown_as: "backpack/pick (FemaleMinerModel.java:121)" },
    Mark { key: "potion", written_by: "a druid with a potion out", source: "EntityAIDruid.java:27", shown_as: "potionSatchet (FemaleDruidModel.java:118)" },
    Mark { key: "rod", written_by: "a fisher with the rod out", source: "EntityAIWorkFisherman.java:75", shown_as: "fishingPole (FemaleFisherModel.java:104)" },
    Mark { key: "shovel", written_by: "a miner whose shovel shows", source: "EntityAIStructureMiner.java:97", shown_as: "backpack/shovel (FemaleMinerModel.java:120)" },
    Mark { key: "stone", written_by: "a miner carrying stone", source: "EntityAIStructureMiner.java:96", shown_as: "not the lid of the backpack (FemaleMinerModel.java:118)" },
    Mark { key: "study", written_by: "a student at its lectern", source: "EntityAIStudy.java:44", shown_as: "folded arms, both arms hidden (FemaleStudentModel.java:95-102)" },
    Mark { key: "torch", written_by: "a miner with torches", source: "EntityAIStructureMiner.java:95", shown_as: "backpack/torches (FemaleMinerModel.java:119)" },
    Mark { key: WORKING, written_by: "every worker that is busy", source: "AbstractEntityAIInteract.java:44", shown_as: "every `isWorking` part, 40 models (FemaleBuilderModel.java:106 and following)" },
];

/// `CitizenModel.isWorking` (`:104-108`): the marks contain `working`.
///
/// **A substring test, on purpose**: that is Java's `contains`.
pub fn is_working(marks: &str) -> bool {
    marks.contains(WORKING)
}

/// The three answers `displayHat` needs (`CitizenModel.java:114-121`,
/// `RenderBipedCitizen.java:92-96`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three fields, all read by `display_hat`
pub struct HatInputs {
    /// `entity.getPose() == Pose.SLEEPING`.
    pub sleeping: bool,
    /// The entity's own head slot is empty (`getItemBySlot(HEAD).isEmpty()`).
    pub head_empty: bool,
    /// The data view's answer for "no displayed helmet and no custom skin";
    /// `None` when the citizen has no data view at all, which Java treats as
    /// "hat allowed" (`:121`).
    pub view_allows: Option<bool>,
}

/// `CitizenModel.displayHat` (`:114-121`): a sleeping citizen wears no hat, one
/// with something on its head wears none either, and a citizen with no view keeps
/// its hat.
pub fn display_hat(inputs: HatInputs) -> bool {
    if inputs.sleeping || !inputs.head_empty {
        return false;
    }
    inputs.view_allows.unwrap_or(true)
}

/// The render metadata a frame is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three fields, one per gate
pub struct Ctx<'a> {
    /// `getRenderMetadata()`: the marks concatenated, as the AI writes them.
    pub marks: &'a str,
    /// `getPose() == Pose.SLEEPING`.
    pub sleeping: bool,
    /// [`display_hat`]'s answer for this frame.
    pub hat: bool,
}

/// When a model part is visible, in the ten combinations the 86 citizen models
/// actually use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: ten gates, each one a Java line
pub enum Condition {
    /// `isWorking(entity)`.
    Working,
    /// `isWorking(entity) && displayHat(entity)`.
    WorkingAndHat,
    /// `entity.getPose() != Pose.SLEEPING`.
    NotSleeping,
    /// `entity.getPose() != Pose.SLEEPING && isWorking(entity)`.
    NotSleepingWorking,
    /// `marks.contains(key)`.
    Mark(&'static str),
    /// `!marks.contains(key)` — the miner's lid, the only negation.
    NotMark(&'static str),
    /// `marks.contains(key) && isWorking(entity)`.
    MarkAndWorking(&'static str),
    /// `entity.getPose() != Pose.SLEEPING && marks.contains(key)`.
    MarkNotSleeping(&'static str),
    /// `entity.getPose() != Pose.SLEEPING && marks.contains(key) && isWorking(entity)`.
    MarkNotSleepingWorking(&'static str),
}

impl Condition {
    /// Whether the part shows this frame.
    pub fn holds(self, ctx: &Ctx<'_>) -> bool {
        let working = is_working(ctx.marks);
        match self {
            Condition::Working => working,
            Condition::WorkingAndHat => working && ctx.hat,
            Condition::NotSleeping => !ctx.sleeping,
            Condition::NotSleepingWorking => !ctx.sleeping && working,
            Condition::Mark(key) => ctx.marks.contains(key),
            Condition::NotMark(key) => !ctx.marks.contains(key),
            Condition::MarkAndWorking(key) => ctx.marks.contains(key) && working,
            Condition::MarkNotSleeping(key) => !ctx.sleeping && ctx.marks.contains(key),
            Condition::MarkNotSleepingWorking(key) => {
                !ctx.sleeping && ctx.marks.contains(key) && working
            }
        }
    }
}

/// One model part and the condition that shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three fields, all compared
pub struct Shown {
    /// The part's name in the model tree, `body.getChild("seedBox")` written as
    /// `body/seedBox`.
    pub part: &'static str,
    /// What has to hold for it to be drawn.
    pub when: Condition,
    /// `fichier.java:ligne` of the assignment.
    pub source: &'static str,
}

/// The part conditions, one row per distinct rule (not one per model: the male
/// and female models of a trade always agree).
pub const SHOWN: &[Shown] = &[
    Shown { part: "body/toolbag", when: Condition::Working, source: "FemaleBuilderModel.java:106" },
    Shown { part: "head/Cap", when: Condition::WorkingAndHat, source: "FemaleBuilderModel.java:107" },
    Shown { part: "head/glasses", when: Condition::Working, source: "FemaleAlchemistModel.java:118" },
    Shown { part: "head/RoundHat", when: Condition::WorkingAndHat, source: "FemaleApiaryModel.java:79" },
    Shown { part: "head/WoodsmanHat", when: Condition::WorkingAndHat, source: "FemaleArcherModel.java:86" },
    Shown { part: "body/quiver", when: Condition::Mark("arrow"), source: "FemaleArcherModel.java:87" },
    Shown { part: "body/LogPack/Logs", when: Condition::Mark("logs"), source: "FemaleForesterModel.java:103" },
    Shown { part: "body/backpack", when: Condition::MarkNotSleeping("backpack"), source: "FemaleCourierModel.java:77" },
    Shown { part: "body/fishingPole", when: Condition::Mark("rod"), source: "FemaleFisherModel.java:104" },
    Shown { part: "body/fish", when: Condition::Mark("fish"), source: "FemaleFisherModel.java:105" },
    Shown { part: "head/WoodsmanHat/flowers", when: Condition::Mark("flowers"), source: "FemaleFloristModel.java:94" },
    Shown { part: "head/straw", when: Condition::NotSleeping, source: "FemaleFarmerModel.java:87" },
    Shown { part: "body/seedBox", when: Condition::NotSleepingWorking, source: "FemaleFarmerModel.java:88" },
    Shown { part: "body/carrotBag", when: Condition::MarkNotSleepingWorking("carrot"), source: "FemaleRabbitHerderModel.java:95" },
    Shown { part: "body/bucketwithHandle", when: Condition::Mark("bucket"), source: "FemaleCowHerderModel.java:85" },
    Shown { part: "backpack/lid_r1", when: Condition::NotMark("stone"), source: "FemaleMinerModel.java:118" },
    Shown { part: "backpack/torches", when: Condition::Mark("torch"), source: "FemaleMinerModel.java:119" },
    Shown { part: "backpack/shovel", when: Condition::Mark("shovel"), source: "FemaleMinerModel.java:120" },
    Shown { part: "backpack/pick", when: Condition::Mark("pickaxe"), source: "FemaleMinerModel.java:121" },
    Shown { part: "body/potionSatchet", when: Condition::MarkAndWorking("potion"), source: "FemaleDruidModel.java:118" },
    Shown { part: "right_arm/book", when: Condition::Mark("book"), source: "FemaleStudentModel.java:97" },
    Shown { part: "body/Left_Arm_Folded", when: Condition::Mark("study"), source: "FemaleStudentModel.java:100" },
    Shown { part: "left_arm", when: Condition::NotMark("study"), source: "FemaleStudentModel.java:101" },
    Shown { part: "head/plagueMask", when: Condition::NotSleepingWorking, source: "FemaleHealerModel.java:136" },
];

/// The body lean a loaded courier gets, in radians: `0.1745` is `getActualRotation`
/// (`FemaleCourierModel.java:68`), ten degrees, and `Mth`-free.
pub const CARRY_LEAN: f32 = 0.1745;

/// The rotation `CitizenModel.setupAnim` applies to `body.xRot` and `head.xRot`
/// when they are still 0 (`:36-44`).
///
/// Only the courier overrides it (`FemaleCourierModel.java:66-69`); every other
/// model answers 0 (`CitizenModel.java:88-91`).
pub fn body_rotation(sleeping: bool, marks: &str) -> f32 {
    if sleeping || !marks.contains("backpack") {
        0.0
    } else {
        CARRY_LEAN
    }
}

/// The `UseAnim` a held item asks for, the seven `RenderUtils` tests
/// (`RenderUtils.java:38-70`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the seven animations the method names
pub enum UseAnim {
    /// `UseAnim.BLOCK` — a shield.
    Block,
    /// `UseAnim.BOW`.
    Bow,
    /// `UseAnim.SPEAR`.
    Spear,
    /// `UseAnim.CROSSBOW`.
    Crossbow,
    /// `UseAnim.SPYGLASS`.
    Spyglass,
    /// `UseAnim.TOOT_HORN` — a goat horn.
    TootHorn,
    /// `UseAnim.BRUSH`.
    Brush,
}

/// What one hand holds, for the arm pose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: six fields, all read
pub struct Hand {
    /// `itemstack.isEmpty()`.
    pub empty: bool,
    /// The stack's `getUseAnimation`, when the entity is using it.
    pub use_anim: Option<UseAnim>,
    /// `entity.getUsedItemHand() == this hand`.
    pub using_this_hand: bool,
    /// `getUseItemRemainingTicks()`.
    pub use_ticks_left: i32,
    /// `entity.swinging`.
    pub swinging: bool,
    /// `CrossbowItem.isCharged(stack)`.
    pub charged: bool,
}

/// `HumanoidModel.ArmPose`, the nine values `RenderUtils.getArmPose` can answer
/// (`:24-83`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: nine answers, one per return of the method
pub enum ArmPose {
    /// Nothing in the hand.
    Empty,
    /// A shield raised.
    Block,
    /// A bow drawn.
    BowAndArrow,
    /// A spear about to be thrown.
    ThrowSpear,
    /// A crossbow being loaded.
    CrossbowCharge,
    /// A charged crossbow held still.
    CrossbowHold,
    /// A spyglass at the eye.
    Spyglass,
    /// A goat horn at the mouth.
    TootHorn,
    /// A brush in use.
    Brush,
    /// Anything else.
    Item,
}

/// Which hand an arm-pose question is really about: a left-handed entity swaps
/// `MAIN_HAND` for `OFF_HAND` before reading the stack (`RenderUtils.java:26-29`).
pub fn effective_hand(main_hand: bool, left_handed: bool) -> bool {
    if left_handed { !main_hand } else { main_hand }
}

/// `RenderUtils.getArmPose` (`:37-83`), in its order: the use animation first,
/// the charged crossbow second, `ITEM` last.
pub fn arm_pose(hand: Hand) -> ArmPose {
    if hand.empty {
        return ArmPose::Empty;
    }
    if hand.using_this_hand && hand.use_ticks_left > 0 {
        return match hand.use_anim {
            Some(UseAnim::Block) => ArmPose::Block,
            Some(UseAnim::Bow) => ArmPose::BowAndArrow,
            Some(UseAnim::Spear) => ArmPose::ThrowSpear,
            Some(UseAnim::Crossbow) => ArmPose::CrossbowCharge,
            Some(UseAnim::Spyglass) => ArmPose::Spyglass,
            Some(UseAnim::TootHorn) => ArmPose::TootHorn,
            Some(UseAnim::Brush) => ArmPose::Brush,
            None => ArmPose::Item,
        };
    }
    if !hand.swinging && hand.charged {
        return ArmPose::CrossbowHold;
    }
    ArmPose::Item
}

/// What `setupMainModelFrom` sets on the model besides the arm poses
/// (`RenderBipedCitizen.java:93-97`), and where Horizon's render state carries it.
///
/// The fourth column is the finding: two of the five have no field in
/// `LivingRenderState` (`crates/horizon-mod-vocab/src/client/entity_renderers.rs:38-71`),
/// so a ported citizen model cannot be posed as Java poses it.
pub const MODEL_INPUTS: &[(&str, &str, bool, &str)] = &[
    ("young", "isBaby()", true, "LivingRenderState.is_baby"),
    ("riding", "getVehicle() != null", false, "no field — MANQUANT §3"),
    ("attackTime", "attackAnim", false, "no field — MANQUANT §3"),
    ("scale", "getScale()", true, "LivingRenderState.scale"),
    ("bodyRot", "yBodyRot", true, "LivingRenderState.body_rot"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seventeen_marks_and_no_key_contains_another() {
        assert_eq!(MARKS.len(), 17);
        // `contains` is a substring test on one concatenated string
        // (`EntityAIWorkCowboy.java:85`): it only works while no key is a
        // substring of another, and this is the test that keeps it true.
        for outer in MARKS {
            for inner in MARKS {
                if outer.key != inner.key {
                    assert!(
                        !outer.key.contains(inner.key),
                        "{} contains {}",
                        outer.key,
                        inner.key
                    );
                }
            }
        }
    }

    #[test]
    fn every_mark_is_written_once_and_named_once() {
        let mut keys: Vec<&str> = MARKS.iter().map(|mark| mark.key).collect();
        keys.sort_unstable();
        let mut unique = keys.clone();
        unique.dedup();
        assert_eq!(unique.len(), 17);
        assert!(MARKS.iter().all(|mark| mark.source.contains(".java:")));
        assert!(MARKS.iter().all(|mark| !mark.written_by.is_empty()));
    }

    #[test]
    fn every_mark_but_bowl_is_read_by_a_part() {
        let mut read: Vec<&str> = SHOWN
            .iter()
            .filter_map(|row| match row.when {
                Condition::Mark(key)
                | Condition::NotMark(key)
                | Condition::MarkAndWorking(key)
                | Condition::MarkNotSleeping(key)
                | Condition::MarkNotSleepingWorking(key) => Some(key),
                _ => None,
            })
            .collect();
        read.sort_unstable();
        read.dedup();
        let mut unread: Vec<&str> = MARKS
            .iter()
            .map(|mark| mark.key)
            .filter(|key| *key != WORKING && !read.contains(key))
            .collect();
        unread.sort_unstable();
        // `bowl` is the whole unread set: the cow herder writes it
        // (`EntityAIWorkCowboy.java:85`) and no model has a bowl.
        assert_eq!(unread, vec!["bowl"]);
        assert_eq!(
            MARKS.iter().find(|mark| mark.key == "bowl").map(|mark| mark.shown_as),
            Some("")
        );
    }

    #[test]
    fn working_is_the_substring_test_java_does() {
        assert!(is_working("working"));
        assert!(is_working("workingtorch"));
        assert!(!is_working("torch"));
        assert!(!is_working(""));
        // The one that would break a naive equality test and must not:
        assert!(is_working("logsbackpackworking"));
    }

    #[test]
    fn the_sleeping_flag_beats_a_mark_and_a_missing_view_allows_the_hat() {
        let awake = HatInputs { sleeping: false, head_empty: true, view_allows: None };
        assert!(display_hat(awake));
        let asleep = HatInputs { sleeping: true, head_empty: true, view_allows: Some(true) };
        assert!(!display_hat(asleep));
        let helmet = HatInputs { sleeping: false, head_empty: false, view_allows: Some(true) };
        assert!(!display_hat(helmet));
        let disguised = HatInputs { sleeping: false, head_empty: true, view_allows: Some(false) };
        assert!(!display_hat(disguised));
    }

    #[test]
    fn the_ten_conditions_answer_the_java_expressions() {
        let asleep = Ctx { marks: "", sleeping: true, hat: false };
        let bleary = Ctx { marks: "workingtorch", sleeping: false, hat: true };
        let carrying = Ctx { marks: "backpack", sleeping: false, hat: true };

        assert!(Condition::Working.holds(&bleary));
        assert!(!Condition::Working.holds(&carrying));
        assert!(Condition::WorkingAndHat.holds(&bleary));
        assert!(!Condition::WorkingAndHat.holds(&Ctx { hat: false, ..bleary }));
        assert!(!Condition::NotSleeping.holds(&asleep));
        assert!(!Condition::NotSleepingWorking.holds(&asleep));
        assert!(Condition::Mark("torch").holds(&bleary));
        assert!(!Condition::Mark("torch").holds(&carrying));
        assert!(Condition::NotMark("stone").holds(&bleary));
        assert!(!Condition::NotMark("torch").holds(&bleary));
        assert!(!Condition::MarkAndWorking("backpack").holds(&carrying));
        assert!(Condition::MarkAndWorking("backpack").holds(&Ctx { marks: "backpackworking", ..carrying }));
        assert!(!Condition::MarkNotSleeping("backpack").holds(&asleep));
        assert!(Condition::MarkNotSleeping("backpack").holds(&carrying));
        assert!(!Condition::MarkNotSleepingWorking("backpack").holds(&asleep));
    }

    #[test]
    fn the_miner_lid_is_the_only_negation_and_it_is_not_a_typo() {
        // `backpack.getChild("lid_r1").visible = !contains(STONE)`
        // (`FemaleMinerModel.java:118`): the lid closes while stone is carried.
        let row = SHOWN
            .iter()
            .find(|row| row.part == "backpack/lid_r1")
            .expect("the lid row");
        assert_eq!(row.when, Condition::NotMark("stone"));
    }

    #[test]
    fn a_loaded_courier_leans_and_a_sleeping_one_does_not() {
        assert_eq!(body_rotation(false, ""), 0.0);
        assert_eq!(body_rotation(false, "working"), 0.0);
        assert!((body_rotation(false, "backpack") - CARRY_LEAN).abs() < f32::EPSILON);
        assert!((body_rotation(false, "carryingbackpack") - CARRY_LEAN).abs() < f32::EPSILON);
        assert_eq!(body_rotation(true, "backpack"), 0.0);
    }

    #[test]
    fn an_empty_hand_is_empty_whatever_the_animation_says() {
        let hand = Hand {
            empty: true,
            use_anim: Some(UseAnim::Bow),
            using_this_hand: true,
            use_ticks_left: 20,
            swinging: false,
            charged: false,
        };
        assert_eq!(arm_pose(hand), ArmPose::Empty);
    }

    #[test]
    fn the_seven_use_animations_map_to_seven_poses() {
        for (anim, pose) in [
            (UseAnim::Block, ArmPose::Block),
            (UseAnim::Bow, ArmPose::BowAndArrow),
            (UseAnim::Spear, ArmPose::ThrowSpear),
            (UseAnim::Crossbow, ArmPose::CrossbowCharge),
            (UseAnim::Spyglass, ArmPose::Spyglass),
            (UseAnim::TootHorn, ArmPose::TootHorn),
            (UseAnim::Brush, ArmPose::Brush),
        ] {
            let hand = Hand {
                empty: false,
                use_anim: Some(anim),
                using_this_hand: true,
                use_ticks_left: 1,
                swinging: false,
                charged: false,
            };
            assert_eq!(arm_pose(hand), pose, "{anim:?}");
        }
    }

    #[test]
    fn the_use_animation_needs_the_hand_and_a_remaining_tick() {
        let base = Hand {
            empty: false,
            use_anim: Some(UseAnim::Bow),
            using_this_hand: true,
            use_ticks_left: 1,
            swinging: false,
            charged: false,
        };
        assert_eq!(arm_pose(base), ArmPose::BowAndArrow);
        assert_eq!(arm_pose(Hand { use_ticks_left: 0, ..base }), ArmPose::Item);
        assert_eq!(arm_pose(Hand { using_this_hand: false, ..base }), ArmPose::Item);
        // A used item with no animation the method names falls through to ITEM.
        assert_eq!(arm_pose(Hand { use_anim: None, ..base }), ArmPose::Item);
    }

    #[test]
    fn a_charged_crossbow_is_held_only_while_not_swinging() {
        let hand = Hand {
            empty: false,
            use_anim: None,
            using_this_hand: false,
            use_ticks_left: 0,
            swinging: false,
            charged: true,
        };
        assert_eq!(arm_pose(hand), ArmPose::CrossbowHold);
        assert_eq!(arm_pose(Hand { swinging: true, ..hand }), ArmPose::Item);
        assert_eq!(arm_pose(Hand { charged: false, ..hand }), ArmPose::Item);
    }

    #[test]
    fn a_left_handed_citizen_reads_the_other_hand() {
        assert!(effective_hand(true, false), "right-handed: main is main");
        assert!(!effective_hand(true, true), "left-handed: main reads off");
        assert!(effective_hand(false, true), "left-handed: off is the item hand");
        assert!(!effective_hand(false, false));
    }

    #[test]
    fn two_model_inputs_have_no_field_in_the_render_state() {
        let missing: Vec<&str> = MODEL_INPUTS
            .iter()
            .filter(|entry| !entry.2)
            .map(|entry| entry.0)
            .collect();
        assert_eq!(missing, vec!["riding", "attackTime"]);
        assert_eq!(MODEL_INPUTS.len(), 5);
    }
}
