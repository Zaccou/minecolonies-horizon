//! La suggestion d'outil de construction : un choix, deux messages, un refus.
//!
//! Java reference : `core/client/gui/WindowSuggestBuildTool.java` (95 lignes) et son XML
//! `assets/minecolonies/gui/windowsuggestbuildtool.xml` (14 lignes).
//!
//! **Cette fenetre ne montre aucune donnee.** Son panneau `text` porte une phrase ecrite
//! dans le XML (`windowsuggestbuildtool.xml:9`,
//! `$(com.minecolonies.coremod.gui.placement.warning)`) qu'aucune ligne de la classe ne
//! touche : la classe n'a que trois `registerButton` (`:59-61`) et quatre champs prives
//! (`:32-44`). Ce qu'elle porte est donc un **choix**, pas un affichage.
//!
//! **Le bouton `direct` est cable sans exister.** `registerButton(BUTTON_DIRECT, ...)`
//! (`:60`) enregistre l'id `direct` (`WindowConstants.java:1034`), et le XML ne declare que
//! `cancel`, `text` et `buildtool` (`windowsuggestbuildtool.xml:6`, `:9`, `:11`).
//! `onButtonClicked` ne repond qu'a un panneau reellement clique
//! (`AbstractWindowSkeleton.java:191-199`) : le placement direct est donc **inatteignable
//! par cette fenetre**, alors que son message existe et part. C'est l'inverse du
//! `viewschem` de [`super::analyzer`], panneau sans gestionnaire.
//!
//! **Deux sorties differentes pour le meme bouton.** `buildtool` ouvre l'outil etendu
//! (`:81`) ; si l'inventaire n'a pas l'outil, il **n'ouvre rien**, previent le joueur
//! (`:84`) et ferme (`:85`). Un appelant qui traite le refus sans fermer laisse une fenetre
//! morte ouverte.

use crate::ui::field::Value;

use super::pane::{Button, Exit, Label, Message, Pane, Press, Source, Window};
use super::refusal::Refusal;

/// L'id de la fenetre dans cette table.
pub const SUGGEST_ID: &str = "suggest.buildtool";

/// La phrase du XML (`windowsuggestbuildtool.xml:9`).
pub const WARNING_KEY: &str = "com.minecolonies.coremod.gui.placement.warning";

/// Le libelle du bouton de l'outil (`windowsuggestbuildtool.xml:13`).
pub const BUILDTOOL_KEY: &str = "com.minecolonies.coremod.gui.placement.buildtool";

/// Le style de sol que la fenetre ouvre : relatif a l'ancre
/// (`GROUNDSTYLE_RELATIVE`, structurize `api/util/constant/Constants.java:23`), passe a
/// `new WindowExtendedBuildTool(this.pos, GROUNDSTYLE_RELATIVE)` (`:81`).
pub const GROUNDSTYLE_RELATIVE: i32 = 1;

/// Ce que le joueur a clique.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: les deux chemins de la classe, `:67-86`
pub enum Choice {
    /// Placer le bloc tout de suite (`:67-71`).
    Direct,
    /// Passer par l'outil de construction (`:76-86`).
    BuildTool,
}

impl Choice {
    /// Les deux choix, dans l'ordre du fichier.
    pub const ALL: [Choice; 2] = [Choice::Direct, Choice::BuildTool];

    /// L'id du bouton qui porte ce choix (`WindowConstants.java:1029`, `:1034`).
    pub fn button(self) -> &'static str {
        match self {
            Choice::Direct => "direct",
            Choice::BuildTool => "buildtool",
        }
    }
}

/// Ce qu'un choix laisse derriere lui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: un clic fait quelque chose, ou il est refuse avec une sortie
pub enum Answer {
    /// Le message est parti et la fenetre suit son [`Exit`].
    Done(Press),
    /// Java refuse **et sort** : [`Exit`] est la, pour qu'un appelant ne l'oublie pas.
    Refused { why: Refusal, exit: Exit },
}

/// Ce que fait un clic, selon l'inventaire (`WindowSuggestBuildTool.java:67-86`).
///
/// `has_build_tool` est la seule entree qui vient du joueur :
/// `InventoryUtils.findFirstSlotInItemHandlerWith(..., ModItems.buildTool.get()) != -1`
/// (`:78`). Le port n'a pas d'inventaire ; il a la reponse a cette question.
pub fn press(choice: Choice, has_build_tool: bool) -> Answer {
    match choice {
        // `:69` puis `:70` : le message part, la fenetre se ferme.
        Choice::Direct => Answer::Done(DIRECT_PRESS),
        // `:80-81` : l'outil est la, on annonce le changement de bloc et on ouvre.
        Choice::BuildTool if has_build_tool => Answer::Done(BUILDTOOL_PRESS),
        // `:84-85` : pas d'outil, un message au joueur, aucune fenetre ouverte, et on ferme.
        Choice::BuildTool => Answer::Refused {
            why: Refusal::MissingBuildTool,
            exit: Exit::Closes,
        },
    }
}

/// Le cablage mort : l'id `direct` est enregistre la ligne 60 et aucun XML ne le declare.
///
/// Le triplet est `(id, fichier qui le cable, ligne)`.
pub const UNREACHABLE_BINDING: (&str, &str, u32) = ("direct", "WindowSuggestBuildTool.java", 60);

/// Le clic du placement direct (`:69-70`) : un message, puis la fermeture.
///
/// **`const`, et pas un tableau ecrit dans le corps de [`press`]** : un `&[Message]` bati
/// dans une fonction n'est pas promu en `'static` (`E0716`), alors qu'un tableau dans un
/// initialiseur `const` l'est. Le clic est donc declare **une seule fois**, ici, et
/// [`press`] comme la table de boutons le reprennent.
pub const DIRECT_PRESS: Press = Press::new(
    &[Message::plain("DirectPlaceMessage")],
    Exit::Closes,
);

/// Le clic de l'outil present (`:80-81`) : annoncer le bloc, puis ouvrir l'outil etendu.
pub const BUILDTOOL_PRESS: Press = Press::new(
    &[Message::plain("SwitchBuildingWithToolMessage")],
    Exit::OpensJava("WindowExtendedBuildTool"),
);

/// Le panneau `text`, seule donnee de la fenetre — et c'est le XML qui la pose.
const PANES: &[Pane] = &[Pane::new("text", Value::Text, Source::Xml)];

const BUTTONS: &[Button] = &[
    // `:59`, `:76-86` : la branche heureuse est ici, le refus est dans `press`.
    Button {
        id: "buildtool",
        label: Label::Xml(BUILDTOOL_KEY),
        press: BUILDTOOL_PRESS,
        refusals: &[Refusal::MissingBuildTool],
        declared: Some(("windowsuggestbuildtool.xml", 11)),
    },
    // `:60`, `:67-71` : cable, mais aucun panneau ne porte cet id.
    Button {
        id: "direct",
        label: Label::Image,
        press: DIRECT_PRESS,
        refusals: &[],
        declared: None,
    },
    // `:61`, `:91-94`.
    Button {
        id: "cancel",
        label: Label::Image,
        press: Press::CLOSES,
        refusals: &[],
        declared: Some(("windowsuggestbuildtool.xml", 6)),
    },
];

/// `WindowSuggestBuildTool` : une phrase, trois boutons dont un sans panneau.
pub static WINDOW: Window = Window {
    id: SUGGEST_ID,
    java: "WindowSuggestBuildTool",
    resource: "minecolonies:gui/windowsuggestbuildtool.xml",
    title: None,
    panes: PANES,
    buttons: BUTTONS,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_tool_choice_sends_then_opens_and_the_missing_tool_only_warns() {
        let with = press(Choice::BuildTool, true);
        assert_eq!(with, Answer::Done(BUILDTOOL_PRESS));
        let Answer::Done(done) = with else { panic!("un outil present doit ouvrir") };
        assert_eq!(done.sends.len(), 1, "un seul message : :80");
        assert_eq!(done.sends[0].class, "SwitchBuildingWithToolMessage");
        assert_eq!(done.exit, Exit::OpensJava("WindowExtendedBuildTool"));
        // Sans outil : un refus **qui ferme**, et aucun message envoye.
        let without = press(Choice::BuildTool, false);
        assert_eq!(
            without,
            Answer::Refused { why: Refusal::MissingBuildTool, exit: Exit::Closes }
        );
        let Answer::Refused { why, exit } = without else { panic!("sans outil, c'est un refus") };
        assert_eq!(why.key(), Some("item.buildtool.missing"));
        assert!(why.closes());
        assert_eq!(exit, Exit::Closes);
    }

    #[test]
    fn the_direct_choice_sends_one_message_and_closes_whatever_the_inventory_holds() {
        for has_build_tool in [true, false] {
            let answer = press(Choice::Direct, has_build_tool);
            assert_eq!(
                answer,
                Answer::Done(DIRECT_PRESS),
                "le placement direct ne regarde pas l'inventaire : :67-71"
            );
        }
        assert_eq!(DIRECT_PRESS.sends[0].class, "DirectPlaceMessage");
        assert_eq!(DIRECT_PRESS.exit, Exit::Closes);
        assert_eq!(Choice::Direct.button(), "direct");
        assert_eq!(Choice::BuildTool.button(), "buildtool");
        assert_eq!(Choice::ALL.len(), 2);
    }

    #[test]
    fn the_direct_binding_has_no_pane_in_the_xml_and_the_window_says_so() {
        assert_eq!(UNREACHABLE_BINDING, ("direct", "WindowSuggestBuildTool.java", 60));
        let direct = WINDOW.button("direct").unwrap_or_else(|| panic!("direct n'est pas decrit"));
        assert_eq!(direct.declared, None);
        assert!(!direct.is_reachable());
        // Les deux autres sont atteignables, et un seul des trois est garde.
        assert_eq!(WINDOW.reachable(), ["buildtool", "cancel"]);
        assert!(WINDOW.button("buildtool").is_some_and(|button| button.is_guarded()));
        assert!(!WINDOW.button("cancel").is_some_and(|button| button.is_guarded()));
        // La seule donnee de la fenetre vient du XML, pas d'une classe.
        assert_eq!(WINDOW.panes.len(), 1);
        assert_eq!(WINDOW.pane("text").map(|pane| pane.source), Some(Source::Xml));
        assert_eq!(WINDOW.title, None);
        assert_eq!(WARNING_KEY, "com.minecolonies.coremod.gui.placement.warning");
        assert_eq!(BUILDTOOL_KEY, "com.minecolonies.coremod.gui.placement.buildtool");
        // Et le refus de l'outil absent est le seul de cette fenetre.
        let guarded = WINDOW.button("buildtool").unwrap_or_else(|| panic!("pas de buildtool"));
        assert_eq!(guarded.refusals.len(), 1);
        assert_eq!(guarded.refusals[0], Refusal::MissingBuildTool);
    }
}
