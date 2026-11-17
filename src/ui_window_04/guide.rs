//! `WindowHutGuide` : l'écran que l'atelier du builder ouvre **une fois**, avant de s'ouvrir.
//!
//! Java reference : `core/client/gui/WindowHutGuide.java` (41 lignes) et
//! `gui/windowhutguide.xml` (10). Deux `registerButton` (`:33-34`), un gestionnaire de six lignes
//! (`:37-41`), et **aucune écriture de panneau** : c'est la fenêtre la plus simple des quatre, et
//! celle dont `src/ui` se trompe le plus.
//!
//! # Un écran sans données
//!
//! Le XML est une image plein écran (`:5`, 960×540, `type="FULLSCREEN_VANILLA"` et `lightbox="true"`
//! à `:2`) et **trois phrases figées** (`:7-9`), écrites en `$(…)` donc résolues par BlockUI au
//! chargement. Le constructeur ne prend qu'un bâtiment (`:28`) et ne s'en sert que pour le passer au
//! gestionnaire (`:40`) : il n'y a pas une seule valeur de modèle à l'écran.
//!
//! **`src/ui` attribue ces panneaux à l'avancement, et c'est faux.** `ui/screen.rs` les déclare
//! `Source::Advancement` ; l'avancement ne remplit rien du tout, il décide seulement si la fenêtre
//! s'ouvre ([`should_open`]). Pire, il est accordé **par le clic sur `confirm` de cette fenêtre**
//! (`DefaultAdvancementsProvider.java:176-181`, critère `ClickGuiButtonCriterionInstance(
//! WindowConstants.GUIDE_CONFIRM, WindowHutGuide.WINDOW_ID)`) : s'en servir comme source serait
//! circulaire. [`PANE_ORIGINS`] est l'origine réelle, et [`Origin`] existe parce que
//! `crate::ui::field::Source` n'a pas de variant pour « le fichier de mise en page le porte ».
//!
//! # Le bouton qui n'existe pas
//!
//! Java câble `GUIDE_CONFIRM` **et** `GUIDE_CLOSE` (`:33-34`) ; `windowhutguide.xml` ne porte que
//! `confirm` (`:6`). `close` (`WindowConstants.java:523`) est donc une liaison morte : le clic ne
//! peut pas arriver, et le critère d'avancement, lui, ne regarde que `confirm`. Le test
//! [`tests::the_guide_binds_a_second_button_that_its_xml_does_not_have`] compare la ligne de
//! `SCREENS` au fichier, parce qu'une divergence entre les deux est exactement ce que ce module
//! existe pour rendre visible.
//!
//! # La sortie, et pourquoi le guide ne boucle pas
//!
//! Les deux boutons font la même chose : fermer, puis rouvrir l'atelier avec `needGuide = false`
//! (`:39-40`). Le second constructeur existe pour ça (`WindowHutBuilderModule.java:46-50`) ; sans
//! lui, `onOpened` refermerait l'atelier et rouvrirait le guide indéfiniment (`:55-63`). Le port
//! porte donc ce booléen comme une valeur de retour : [`handoff`].

use crate::building::Kind;
use crate::ui::field::Value;
use crate::ui::screen::{HUT_BUILDER_ID, HUT_GUIDE_ID, Screen};

use super::refusal::Refusal;

/// La classe Java (`WindowHutGuide.java:14`, qui étend `AbstractWindowSkeleton`).
pub const JAVA: &str = "WindowHutGuide";

/// Le XML (`WindowHutGuide.java:16`).
pub const RESOURCE: &str = "minecolonies:gui/windowhutguide.xml";

/// L'avancement qui ferme la porte (`WindowHutBuilderModule.java:23`) : acquis seulement par un
/// clic sur `confirm` de cette fenêtre, et jamais retiré ensuite.
pub const GATE: &str = "minecolonies:minecolonies/check_out_guide";

/// L'écran vers lequel les deux boutons renvoient (`src/ui::screen::HUT_BUILDER_ID`).
pub const HANDS_TO: &str = HUT_BUILDER_ID;

/// Les boutons que le XML porte réellement : **un** (`windowhutguide.xml:6`).
pub const BUTTONS_IN_XML: &[&str] = &["confirm"];

/// Le bouton que Java câble et que le XML n'a pas (`:34`, `WindowConstants.java:523`).
pub const DEAD_BINDING: &str = "close";

/// Le critère d'avancement : ce bouton-là, dans cette fenêtre-là
/// (`DefaultAdvancementsProvider.java:179-180`).
///
/// Java nomme le critère `click_gui_button_close` et le câble sur `GUIDE_CONFIRM` : le nom du
/// critère contredit le bouton qu'il surveille, et c'est le bouton qui a raison.
pub const CRITERION: (&str, &str) = ("confirm", RESOURCE);

/// La fenêtre est plein écran et pose un voile sur le jeu (`windowhutguide.xml:2`).
pub const FULLSCREEN: bool = true;

/// D'où vient la valeur d'un panneau de cette fenêtre.
///
/// Deux cas, et aucun n'est un modèle : le texte est **dans le XML**, l'image aussi. C'est
/// précisément ce que [`crate::ui::field::Source`] ne peut pas dire — ses neuf variants nomment tous
/// une lecture de modèle — et c'est pourquoi ce module déclare le sien au lieu de forcer un variant
/// qui mentirait (`MANQUANT.md` §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two ways the guide's XML carries a value
pub enum Origin {
    /// Le XML écrit le texte, en `$(clé)` : BlockUI le traduit au chargement, la classe n'y touche
    /// pas (`windowhutguide.xml:7-9`).
    XmlLiteral,
    /// Le XML nomme une texture (`windowhutguide.xml:5`).
    XmlImage,
}

/// Les quatre panneaux du guide, avec leur genre et leur origine réelle.
///
/// **Quatre panneaux, zéro écriture par la classe.** Le test de ce module confronte cette table à
/// celle de `src/ui`, qui dit `Source::Advancement` pour les quatre.
pub const PANE_ORIGINS: &[(&str, Value, Origin)] = &[
    ("guide1", Value::Icon, Origin::XmlImage),
    ("questionmark", Value::Text, Origin::XmlLiteral),
    ("arrow", Value::Text, Origin::XmlLiteral),
    ("chest", Value::Text, Origin::XmlLiteral),
];

/// La ligne de `SCREENS` que cette fenêtre **est**, ou [`Refusal::UnknownWindow`].
pub fn screen() -> Result<&'static Screen, Refusal> {
    crate::ui::table::screen_by_id(HUT_GUIDE_ID).map_err(|_| Refusal::UnknownWindow)
}

/// Le guide doit-il s'ouvrir, sachant si l'avancement est acquis ?
///
/// `WindowHutBuilderModule.java:55-63` : l'atelier se ferme et ouvre le guide quand l'avancement
/// **n'est pas** fait ; un avancement absent du gestionnaire compte comme non fait (`ad == null`),
/// ce que le booléen d'appelant doit refléter.
pub fn should_open(advancement_done: bool) -> bool {
    !advancement_done
}

/// Le guide de ce type de bâtiment, ou [`Refusal::GuideNotForBuilder`].
///
/// Le constructeur Java prend un `BuildingBuilder.View` (`:28`) et **pas** un `IBuildingView` : la
/// contrainte est dans le type, donc à l'appel ; le port la rend explicite, parce qu'un
/// `ClassCastException` n'est pas un refus nommé.
pub fn open_for(hut: Kind) -> Result<&'static Screen, Refusal> {
    if hut == Kind::Builder {
        screen()
    } else {
        Err(Refusal::GuideNotForBuilder { hut })
    }
}

/// L'événement que le clic sur `confirm` enregistre, dans le vocabulaire du port.
///
/// `Event::GuiClicked { button, window }` (`crate::advancements::state`), qui est le paramètre
/// `click_gui_button` de `ClickGuiButtonCriterionInstance.java:56-68`. La ligne
/// `minecolonies/check_out_guide` de `crate::advancements::tree::TREE` porte exactement cet
/// événement, et un test le vérifie : deux tables, une seule vérité.
pub fn clicked() -> crate::advancements::Event {
    crate::advancements::Event::GuiClicked { button: CRITERION.0, window: CRITERION.1 }
}

/// Le message que le client enverrait, tel que `src/network` le décode déjà
/// (`network/windows.rs:284-310`, `ClickGuiButtonTriggerMessage.java:12-69`).
///
/// **Rien ne l'envoie** : le port n'a pas de client, et `src/ui/MANQUANT.md` §5 dit pourquoi. Le
/// type existe, le contenu est décidé ici, et la jonction s'arrête là.
pub fn trigger() -> crate::network::windows::ClickGuiButtonTrigger {
    crate::network::windows::ClickGuiButtonTrigger {
        button_id: CRITERION.0.to_owned(),
        resource: CRITERION.1.to_owned(),
    }
}

/// Ce que la fermeture du guide fait ensuite : l'écran, et si le guide doit se réarmer.
///
/// `false` est le cœur de l'affaire : `new WindowHutBuilderModule(building, false).open()`
/// (`:40`). Un appelant qui répondrait `true` ferait boucler l'atelier sur son propre guide.
pub fn handoff() -> (&'static str, bool) {
    (HANDS_TO, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::action::Effect;
    use crate::ui::field::Source;

    fn row() -> &'static Screen {
        screen().unwrap_or_else(|refusal| panic!("{refusal}"))
    }

    #[test]
    fn the_guide_binds_a_second_button_that_its_xml_does_not_have() {
        assert_eq!(BUTTONS_IN_XML, &["confirm"][..]);
        assert_eq!(DEAD_BINDING, "close");
        // `src/ui` garde les deux, et l'un des deux n'a pas de panneau : c'est la divergence.
        let screen = row();
        let listed: Vec<&str> = screen.actions.iter().map(|action| action.id).collect();
        assert_eq!(listed, ["confirm", "close"]);
        for id in BUTTONS_IN_XML {
            assert!(listed.contains(id), "{id} du XML n'est pas dans SCREENS");
        }
        assert!(
            !BUTTONS_IN_XML.contains(&DEAD_BINDING),
            "le bouton mort ne doit pas être dans le XML"
        );
        // Le bouton mort porte le même effet que l'autre : Java ne l'aurait pas distingué.
        assert_eq!(
            screen.action(DEAD_BINDING).map(|action| action.effect),
            screen.action("confirm").map(|action| action.effect)
        );
    }

    #[test]
    fn the_advancement_gates_the_open_and_fills_no_pane_at_all() {
        assert!(should_open(false), "avancement absent : le guide s'ouvre");
        assert!(!should_open(true));
        assert_eq!(GATE, "minecolonies:minecolonies/check_out_guide");
        // Quatre panneaux, et pas un seul n'est une lecture du modèle.
        assert_eq!(PANE_ORIGINS.len(), 4);
        for (id, value, origin) in PANE_ORIGINS {
            assert!(matches!(*origin, Origin::XmlLiteral | Origin::XmlImage), "{id}");
            let field = row().field(id).unwrap_or_else(|| panic!("{id} n'est pas dans SCREENS"));
            assert_eq!(field.value, *value, "{id}");
            // Ce que `src/ui` en dit, et qui ne peut pas être la source d'un panneau du XML.
            assert_eq!(
                field.source,
                Source::Advancement,
                "{id} : la ligne de SCREENS attribue ce panneau à l'avancement"
            );
        }
        // Une seule image, trois textes.
        assert_eq!(PANE_ORIGINS.iter().filter(|entry| entry.2 == Origin::XmlImage).count(), 1);
        // La porte n'est pas sur le guide : elle est sur l'atelier qu'il rouvre.
        assert_eq!(row().gate, None);
    }

    #[test]
    fn the_criterion_is_the_confirm_button_of_this_very_window() {
        assert_eq!(CRITERION, ("confirm", RESOURCE));
        assert_eq!(CRITERION.1, row().resource);
        // Java nomme le critère « click_gui_button_close » et surveille « confirm »
        // (`DefaultAdvancementsProvider.java:178-180`) : le bouton du critère n'est pas le mort.
        assert_ne!(CRITERION.0, DEAD_BINDING);
        assert!(BUTTONS_IN_XML.contains(&CRITERION.0));
        // La fenêtre est plein écran, ce que `Screen` ne porte pas.
        assert!(FULLSCREEN);
    }

    #[test]
    fn the_advancement_tree_spells_the_same_criterion_and_the_message_carries_it() {
        use crate::advancements::{Condition, Event};
        // La ligne de l'arbre, par son chemin de fichier, et par l'id complet de la porte.
        let row_adv = crate::advancements::tree::find("minecolonies/check_out_guide")
            .unwrap_or_else(|| panic!("check_out_guide n'est pas dans TREE"));
        assert_eq!(row_adv.full_id(), GATE);
        assert_eq!(
            row_adv.condition,
            Condition::Happened(Event::GuiClicked { button: "confirm", window: RESOURCE })
        );
        assert_eq!(row_adv.condition, Condition::Happened(clicked()));
        // Le message : deux champs, les deux du critère, et rien d'autre.
        let trigger = trigger();
        assert_eq!(trigger.button_id.as_str(), CRITERION.0);
        assert_eq!(trigger.resource.as_str(), CRITERION.1);
        // Le critère est donc écrit trois fois dans deux langages et ne dit qu'une chose :
        // `DefaultAdvancementsProvider` le câble, `advancements::tree` le range, `network::windows`
        // le transporte.
        assert_eq!(CRITERION.1, "minecolonies:gui/windowhutguide.xml");
    }

    #[test]
    fn both_buttons_land_on_the_builder_hut_without_rearming_the_guide() {
        let (screen_id, need_guide) = handoff();
        assert_eq!(screen_id, HUT_BUILDER_ID);
        assert!(!need_guide, "WindowHutGuide.java:40 : le second constructeur, sans guide");
        for action in row().actions {
            assert_eq!(action.effect, Effect::Opens(HUT_BUILDER_ID), "{}", action.id);
        }
        // La cible existe dans la table, et c'est elle qui porte la porte.
        let target = crate::ui::table::screen_by_id(screen_id)
            .unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(target.window, "WindowHutBuilderModule");
        assert!(target.gate.is_some());
        // Le guide n'existe que pour l'atelier du builder.
        assert_eq!(
            open_for(Kind::Farmer),
            Err(Refusal::GuideNotForBuilder { hut: Kind::Farmer })
        );
        assert_eq!(open_for(Kind::Builder).map(|screen| screen.id), Ok(HUT_GUIDE_ID));
        // Et le guide lui-même n'a aucune porte : c'est l'atelier qui en a une.
        assert_eq!(row().gate, None);
    }
}
