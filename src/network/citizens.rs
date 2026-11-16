//! Citizens: a skill raised, a citizen paused, one recalled, one restarted, and the
//! two answers a dialogue sends back to the server.
//!
//! Java reference: `core/network/messages/server/colony/citizen/`
//! `AdjustSkillCitizenMessage.java` (111), `PauseCitizenMessage.java` (72),
//! `RecallSingleCitizenMessage.java` (95), `RestartCitizenMessage.java` (85), and
//! `server/colony/InteractionClose.java` (94), `InteractionResponse.java` (104).
//!
//! **Four of the six name a citizen by id**, not by position: a citizen is looked up
//! in the colony's civilian list (`:80`, `:68`, `:68`, `:78`) and a citizen that is
//! not there is a refusal Java logs. [`RecallSingleCitizen`] is the exception that
//! proves the rule — it extends the *building* message, because the citizen is
//! teleported to a hut (`:69`, `:82`) and the hut is what has to be in range.
//!
//! **Two carry a chat component.** `InteractionClose` sends the key of the dialogue
//! that closed (`:64`) and `InteractionResponse` the key plus which answer was
//! clicked (`:73`); both are keys into the dialogue's json, and
//! `common::Component` is the port's cut of a `Component` — the key, and the arguments
//! already rendered.
//!
//! **`AdjustSkillCitizen` is a creative-only debug tool.** Java refuses a sender who
//! is not creative (`:100-105`), which is a property of the *player*, not of the
//! message: the port cannot refuse it here and says so in `RAPPORT.md` instead of
//! dropping the check.

use super::common::{
    component, component_of, envelope, envelope_of, int, int_of, text, text_of, Component,
    Message, Refusal, Rule,
};
use crate::base::Dimension;
use mod_api::Compound;

/// The eleven citizen skills, in Java's declaration order
/// (`api/entity/citizen/Skill.java:10-150`).
///
/// Java writes the ordinal (`AdjustSkillCitizenMessage.java:66`, `:74`); the port
/// writes the name and says why in `common`'s note. `Skill` is a domain value here —
/// raising a skill is a rule the port will have to honour — so it earns a type
/// rather than an integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the eleven values of Java's Skill
pub enum Skill {
    Athletics,
    Dexterity,
    Strength,
    Agility,
    Stamina,
    Mana,
    Adaptability,
    Focus,
    Creativity,
    Knowledge,
    Intelligence,
}

impl Skill {
    pub const ALL: [Skill; 11] = [
        Skill::Athletics,
        Skill::Dexterity,
        Skill::Strength,
        Skill::Agility,
        Skill::Stamina,
        Skill::Mana,
        Skill::Adaptability,
        Skill::Focus,
        Skill::Creativity,
        Skill::Knowledge,
        Skill::Intelligence,
    ];

    /// The enum constant as Java spells it — `Skill.values()[…].name()`.
    pub fn name(self) -> &'static str {
        match self {
            Skill::Athletics => "Athletics",
            Skill::Dexterity => "Dexterity",
            Skill::Strength => "Strength",
            Skill::Agility => "Agility",
            Skill::Stamina => "Stamina",
            Skill::Mana => "Mana",
            Skill::Adaptability => "Adaptability",
            Skill::Focus => "Focus",
            Skill::Creativity => "Creativity",
            Skill::Knowledge => "Knowledge",
            Skill::Intelligence => "Intelligence",
        }
    }

    pub fn parse(name: &str) -> Option<Skill> {
        Skill::ALL.into_iter().find(|skill| skill.name() == name)
    }

    /// The skill that complements this one (`Skill.java:12-13` and each block after
    /// it). A rule of the citizen layer, kept here because the enum is here.
    pub fn complementary(self) -> Skill {
        match self {
            Skill::Athletics => Skill::Strength,
            Skill::Dexterity => Skill::Agility,
            Skill::Strength => Skill::Dexterity,
            Skill::Agility => Skill::Dexterity,
            Skill::Stamina => Skill::Athletics,
            Skill::Mana => Skill::Knowledge,
            Skill::Adaptability => Skill::Focus,
            Skill::Focus => Skill::Adaptability,
            Skill::Creativity => Skill::Knowledge,
            Skill::Knowledge => Skill::Intelligence,
            Skill::Intelligence => Skill::Knowledge,
        }
    }
}

/// `AdjustSkillCitizenMessage.java:20-110` — add or remove levels of one skill
/// (`:107`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdjustSkillCitizen {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub citizen_id: i32,
    /// How many levels, in either direction (`:30`, `:107`).
    pub quantity: i32,
    pub skill: Skill,
}

impl Message for AdjustSkillCitizen {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("citizenId".into(), int(self.citizen_id));
        data.insert("quantity".into(), int(self.quantity));
        data.insert("skill".into(), text(self.skill.name()));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        let name = text_of(data, "skill")?;
        Ok(AdjustSkillCitizen {
            colony_id,
            dimension,
            citizen_id: int_of(data, "citizenId")?,
            quantity: int_of(data, "quantity")?,
            skill: Skill::parse(&name).ok_or(Refusal::Name { key: "skill" })?,
        })
    }
}

/// `PauseCitizenMessage.java:14-71` — flip a citizen's paused flag (`:69`).
///
/// A toggle, not a value: the message does not say what the new state is, it says
/// "the other one". The port keeps that shape — a `bool` here would be a different
/// message, and two clients racing would disagree about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PauseCitizen {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub citizen_id: i32,
}

impl Message for PauseCitizen {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("citizenID".into(), int(self.citizen_id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(PauseCitizen { colony_id, dimension, citizen_id: int_of(data, "citizenID")? })
    }
}

/// `RecallSingleCitizenMessage.java:24-94` — bring one citizen to this hut
/// (`:69`, `:82`).
///
/// Extends `AbstractBuildingServerMessage`, so it carries the hut and answers
/// `MANAGE_HUTS`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecallSingleCitizen {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub building: [i32; 3],
    pub citizen_id: i32,
}

impl Message for RecallSingleCitizen {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        super::common::building_envelope(&mut data, &self.dimension, self.colony_id, self.building);
        data.insert("citizenId".into(), int(self.citizen_id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id, building) = super::common::building_envelope_of(data)?;
        Ok(RecallSingleCitizen {
            colony_id,
            dimension,
            building,
            citizen_id: int_of(data, "citizenId")?,
        })
    }
}

/// `RestartCitizenMessage.java:18-84` — have a citizen respawn, telling the sender
/// which one (`:78-82`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestartCitizen {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub citizen_id: i32,
}

impl Message for RestartCitizen {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("citizenID".into(), int(self.citizen_id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(RestartCitizen { colony_id, dimension, citizen_id: int_of(data, "citizenID")? })
    }
}

/// `server/colony/InteractionClose.java:16-93` — the dialogue closed, for this
/// citizen and this dialogue key (`:90`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractionClose {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub citizen_id: i32,
    pub key: Component,
}

impl Message for InteractionClose {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("citizenId".into(), int(self.citizen_id));
        data.insert("key".into(), component(&self.key));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(InteractionClose {
            colony_id,
            dimension,
            citizen_id: int_of(data, "citizenId")?,
            key: component_of(data, "key")?,
        })
    }
}

/// `server/colony/InteractionResponse.java:16-103` — the player clicked answer
/// `response_id` of this dialogue (`:100`).
///
/// The id is an index into the dialogue's own answer list, as the dialogue json
/// wrote it; the port keeps the number because the json is not loaded here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractionResponse {
    pub colony_id: i32,
    pub dimension: Dimension,
    pub citizen_id: i32,
    pub key: Component,
    pub response_id: i32,
}

impl Message for InteractionResponse {
    const RULE: Rule = Rule::ManageHuts;

    fn to_compound(&self) -> Compound {
        let mut data = Compound::new();
        envelope(&mut data, &self.dimension, self.colony_id);
        data.insert("citizenId".into(), int(self.citizen_id));
        data.insert("key".into(), component(&self.key));
        data.insert("responseId".into(), int(self.response_id));
        data
    }

    fn from_compound(data: &Compound) -> Result<Self, Refusal> {
        let (dimension, colony_id) = envelope_of(data)?;
        Ok(InteractionResponse {
            colony_id,
            dimension,
            citizen_id: int_of(data, "citizenId")?,
            key: component_of(data, "key")?,
            response_id: int_of(data, "responseId")?,
        })
    }
}

/// The six messages of this family, by Java file and Rust type.
pub const CENSUS: &[(&str, &str)] = &[
    ("server/colony/citizen/AdjustSkillCitizenMessage.java", "AdjustSkillCitizen"),
    ("server/colony/citizen/PauseCitizenMessage.java", "PauseCitizen"),
    ("server/colony/citizen/RecallSingleCitizenMessage.java", "RecallSingleCitizen"),
    ("server/colony/citizen/RestartCitizenMessage.java", "RestartCitizen"),
    ("server/colony/InteractionClose.java", "InteractionClose"),
    ("server/colony/InteractionResponse.java", "InteractionResponse"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::common::round_trip;

    fn overworld() -> Dimension {
        Dimension::new("minecraft:overworld")
    }

    #[test]
    fn every_skill_round_trips_and_refuses_a_name_the_port_lacks() {
        for skill in Skill::ALL {
            let adjust = AdjustSkillCitizen {
                colony_id: 1,
                dimension: overworld(),
                citizen_id: 5,
                quantity: 3,
                skill,
            };
            assert_eq!(round_trip(adjust).skill, skill);
            assert_eq!(Skill::parse(skill.name()), Some(skill));
        }
        let mut data = AdjustSkillCitizen {
            colony_id: 1,
            dimension: overworld(),
            citizen_id: 5,
            quantity: -3,
            skill: Skill::Mana,
        }
        .to_compound();
        data.insert("skill".into(), text("Charisma"));
        assert_eq!(AdjustSkillCitizen::from_compound(&data), Err(Refusal::Name { key: "skill" }));
        data.insert("skill".into(), text("Mana"));
        assert_eq!(AdjustSkillCitizen::from_compound(&data).map(|m| m.quantity), Ok(-3), "a removal");
    }

    #[test]
    fn the_three_citizen_id_messages_round_trip() {
        round_trip(PauseCitizen { colony_id: 2, dimension: overworld(), citizen_id: 9 });
        round_trip(RestartCitizen { colony_id: 2, dimension: overworld(), citizen_id: 9 });
        let recalled = RecallSingleCitizen {
            colony_id: 2,
            dimension: overworld(),
            building: [3, 64, 3],
            citizen_id: 9,
        };
        assert_eq!(round_trip(recalled).building, [3, 64, 3]);
    }

    #[test]
    fn the_two_interaction_answers_round_trip_their_component() {
        let close = InteractionClose {
            colony_id: 2,
            dimension: overworld(),
            citizen_id: 9,
            key: Component::translatable("com.minecolonies.coremod.dialog.student"),
        };
        assert_eq!(round_trip(close).key.text, "com.minecolonies.coremod.dialog.student");

        let answer = InteractionResponse {
            colony_id: 2,
            dimension: overworld(),
            citizen_id: 9,
            key: Component::translatable("com.minecolonies.coremod.dialog.student"),
            response_id: 2,
        };
        assert_eq!(round_trip(answer).response_id, 2);
    }

    #[test]
    fn an_interaction_without_its_key_is_refused() {
        let mut data = InteractionResponse {
            colony_id: 1,
            dimension: overworld(),
            citizen_id: 1,
            key: Component::literal("hi"),
            response_id: 0,
        }
        .to_compound();
        data.remove(&"key".into());
        assert_eq!(InteractionResponse::from_compound(&data), Err(Refusal::Missing { key: "key" }));
    }

    #[test]
    fn the_skill_table_is_java_order_and_pairs_each_complement() {
        assert_eq!(Skill::ALL.len(), 11);
        assert_eq!(Skill::Athletics.name(), "Athletics", "Java capitalises the constant");
        assert_eq!(Skill::Athletics.complementary(), Skill::Strength);
        for skill in Skill::ALL {
            assert_ne!(skill.complementary(), skill, "{skill:?} complements itself");
        }
        assert_eq!(CENSUS.len(), 6);
        let mut names: Vec<&str> = Skill::ALL.iter().map(|skill| skill.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 11, "two skills share a name");
    }
}
