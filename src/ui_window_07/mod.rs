//! Les quatre fenetres d'outil de `core/client/gui/` : ce qu'elles demandent, ce qu'elles
//! rendent, et ce qu'un clic y fait.
//!
//! Java reference : `WindowSchematicAnalyzer.java` (394 lignes),
//! `WindowSuggestBuildTool.java` (95), `WindowSupplies.java` (195),
//! `WindowSupplyStory.java` (124) — **808 lignes** pour quatre classes — leurs quatre XML
//! sous `assets/minecolonies/gui/` (`analyzer/windowanalyze.xml`,
//! `analyzer/analyzedisplay.xml`, `windowsuggestbuildtool.xml`, `windowsupplies.xml`,
//! `windowsupplystory.xml`), et les classes qu'elles appellent :
//! `SchemAnalyzerUtil` (le calcul), les deux `ItemSupply*Deployer` (le balayage du monde),
//! `ColonyStoryListener` (le corpus de recits) et la fenetre de manipulation de Structurize
//! dont `WindowSupplies` herite.
//!
//! **Ces quatre-la ne sont pas des huttes.** `src/ui` decrit les treize ecrans qu'une hutte
//! ouvre ; ces quatre-ci s'ouvrent depuis un **item** (l'analyseur de schema, la caisse, le
//! navire) ou depuis un **clic qui suggere un outil**. Aucune n'a de titre, aucune ne parle
//! d'un batiment, et deux d'entre elles ne montrent presque rien : la suggestion affiche
//! **une** phrase figee, le recit deux paragraphes. La table de `src/ui` n'a donc pas de
//! ligne a reprendre ici, et ce module n'en duplique aucune : il reprend ses types
//! ([`crate::ui::field::Value`], [`crate::ui::field::Scope`]) et sa forme.
//!
//! # Les comptes, mesures dans le source
//!
//! | Quoi | Combien | Ou |
//! | --- | --- | --- |
//! | fenetres decrites | 4 | [`WINDOWS`] |
//! | classes Java | 4 — 808 lignes | ci-dessus |
//! | panneaux a valeur | 12 | [`Window::panes`] |
//! | dont dans un sous-arbre | 9 | la boite de l'analyseur et sa liste |
//! | panneaux qui existent **deux fois** | 9 — les deux boites | [`analyzer`] |
//! | boutons decrits | 14 | [`Window::buttons`] |
//! | dont inatteignables | **1** — `direct` | [`suggest::UNREACHABLE_BINDING`] |
//! | panneaux-boutons sans gestionnaire | **1** — `viewschem` | [`analyzer::UNWIRED_PANE`] |
//! | boutons herites, nommes non decrits | 12 — dont 2 remplaces | [`supplies::INHERITED`] |
//! | fenetres avec un panneau de titre | **0** | [`Window::title`] |
//! | messages nommes, aucun envoye | 4 | [`Message`] |
//! | refus | 6 | [`Refusal`] |
//! | fautes de placement | 4 | [`supplies::Fault`] |
//!
//! # Six premisses que le source contredit
//!
//! 1. **« Ces ecrans montrent nom, niveau, progression et stock. »** Aucun des quatre ne
//!    montre de **niveau**, de **progression** ni de **stock**. Un seul nom est affiche, et
//!    c'est celui d'un `Blueprint` ([`analyzer::Analysis::display_name`]) ; le reste est
//!    trois comptes et un texte bati de trois entiers. Le test
//!    `the_window_shows_scores_and_a_name_and_no_level_no_progress_no_stock` le tient.
//! 2. **« `WindowSupplies` est une fenetre a elle. »** Elle etend
//!    `AbstractBlueprintManipulationWindow` (`:40`) : **douze** boutons sont ceux de la
//!    classe mere (`AbstractBlueprintManipulationWindow.java:107-118`), elle en possede
//!    **un** (`switch`, `:65`) et en remplace **deux** (`confirm`, `cancel`).
//! 3. **« Un bouton cable existe. »** Faux dans les deux sens, et les deux cas sont ici :
//!    `direct` est cable sans panneau (`WindowSuggestBuildTool.java:60`) donc inatteignable,
//!    et `viewschem` est un `<button>` sans gestionnaire (`analyzedisplay.xml:6`) donc
//!    inerte. `registerButton` n'est qu'un `put` dans une `Map`
//!    (`AbstractWindowSkeleton.java:130-133`) ; c'est `onButtonClicked` qui ne repond qu'a
//!    un panneau clique (`:191-199`).
//! 4. **« Confirmer un placement dit toujours pourquoi il echoue. »** Non : sans pack ou
//!    sans blueprint, `handlePlacement` revient **sans un mot** (`WindowSupplies.java:138-141`).
//!    [`Refusal::NothingToPlace`] est ce silence, nomme.
//! 5. **« Le recit s'affiche. »** Il n'est ajoute que si l'etiquette NBT `Placement` vaut
//!    `instant` (`WindowSupplyStory.java:76`) : sans elle le panneau porte trois lignes au
//!    lieu de cinq, et le debut du texte change ([`supplies::story`]).
//! 6. **« Les deux caisses se ressemblent. »** Le **type** choisit le blueprint et le style
//!    de sol (`:64`) ; l'**item** tenu en main choisit le corpus de recits (`:81`, `:95`).
//!    Rien ne verifie qu'ils s'accordent.
//!
//! Deux details de plus, qui ne sont pas des premisses mais des pieges : le panneau nomme
//! `up` **avance** et celui nomme `plus` **monte** (Structurize
//! `WindowConstants.java:166`, `:21`), et `Value::is_computed` de `src/ui` repond `false`
//! pour le panneau `size` alors que Java le bati de trois entiers
//! (`WindowSchematicAnalyzer.java:373-377`) — le predicat de `src/ui` couvre les quatre
//! panneaux calcules d'une **hutte**, pas ceux d'un outil. Je ne l'etends pas : le fichier
//! est a un autre agent.
//!
//! # Ce qui n'est pas ici
//!
//! - **Le dessin** : positions, tailles, sprites, l'image de fond du recit, les deux boites
//!   de l'analyseur. `TRANCHE.md` §3 M2 dit deja que cette tranche n'en porte aucun.
//! - **Le cout par bloc** de l'analyseur (`SchemAnalyzerUtil.getScoreFor`, `:67-115`) : six
//!   tags de tier et huit tags de bloc que ce port n'a pas. [`analyzer::complexity`] porte la
//!   fin de la formule, celle qui ne depend d'aucun tag, et prend la somme des couts de blocs
//!   en argument. `MANQUANT.md` §1 dit ce qu'il faudrait.
//! - **Le balayage du monde** : [`supplies::attempt`] prend le **compte** des fautes, pas le
//!   monde. Le port n'a ni niveau ni joueur.
//! - **Les caches clients** : `RenderingCache`, `StructurePacks`, `HighlightManager`,
//!   `RenderTaskManager` (`:391`) sont nommes par leurs cles (`placement`, `supplies`) et
//!   jamais appeles.
//! - **Le reseau** : les quatre messages sont des noms ([`Message`]). Aucun n'est envoye.
//! - **Les fenetres qu'elles ouvrent** : `WindowExtendedBuildTool`, `WindowSwitchPack` — deux
//!   classes de Structurize, nommees par [`Exit::OpensJava`].
//! - **Le widget de liste** : la liste des ressources de l'analyseur en est une, et le SDK
//!   n'en a pas (`src/ui/MANQUANT.md` §1). `MANQUANT.md` §2 de ce module dit ce qui manque
//!   ici en plus.
//!
//! # Les fichiers
//!
//! | Fichier | Question |
//! | --- | --- |
//! | [`pane`] | ce qu'un panneau tient, et ce qu'un clic envoie puis devient |
//! | [`refusal`] | pourquoi un clic ne fait rien, et quand le joueur l'apprend |
//! | [`analyzer`] | les deux selections, le score, et les ressources d'un blueprint |
//! | [`suggest`] | deux choix, quatre messages, un outil absent |
//! | [`supplies`] | le placement et ses fautes, le recit et ses lignes |

pub mod analyzer;
pub mod pane;
pub mod refusal;
pub mod suggest;
pub mod supplies;

pub use analyzer::{Analysis, Resource, ResourceRow, Side};
pub use pane::{Button, Exit, Label, Message, Pane, Press, Source, Window};
pub use refusal::Refusal;
pub use supplies::{Facing, Fault, Hand, Kind, Outcome, StoryLine};

/// Les quatre fenetres, dans l'ordre de la mission.
///
/// **`static`, pas `const`** : un `const` n'a pas le droit de referencer un `static`
/// (`E0013`), et ces quatre lignes sont justement des `static` — chacune a une adresse, ce
/// qui permet a `Exit::OpensHere` de porter un id stable.
pub static WINDOWS: &[&Window] = &[
    &analyzer::WINDOW,
    &suggest::WINDOW,
    &supplies::PLACEMENT_WINDOW,
    &supplies::STORY_WINDOW,
];

/// La fenetre de cet id, ou [`Refusal::UnknownWindow`].
///
/// Un id de `src/ui` — `hut.worker`, `hire` — est refuse : cette table ne repond que de ses
/// quatre fenetres, et c'est `crate::ui::table::screen_by_id` qui repond des treize autres.
pub fn window(id: &str) -> Result<&'static Window, Refusal> {
    WINDOWS
        .iter()
        .copied()
        .find(|window| window.id == id)
        .ok_or(Refusal::UnknownWindow)
}

/// La fenetre de cette classe Java, ou [`Refusal::UnknownWindow`].
pub fn window_of(java: &str) -> Result<&'static Window, Refusal> {
    WINDOWS
        .iter()
        .copied()
        .find(|window| window.java == java)
        .ok_or(Refusal::UnknownWindow)
}

/// Le bouton de cette fenetre, ou [`Refusal::UnknownButton`].
pub fn button(id: &str, button: &str) -> Result<Button, Refusal> {
    window(id)?.button(button).ok_or(Refusal::UnknownButton)
}

#[cfg(test)]
mod tests {
    use crate::ui::field::Value;

    use super::*;

    #[test]
    fn four_windows_each_naming_its_class_and_its_xml() {
        assert_eq!(WINDOWS.len(), 4);
        for (index, entry) in WINDOWS.iter().enumerate() {
            assert!(!entry.id.is_empty());
            assert!(entry.java.starts_with("Window"), "{} ", entry.java);
            assert!(
                entry.resource.starts_with("minecolonies:gui/"),
                "{} lit {}",
                entry.id,
                entry.resource
            );
            assert!(entry.resource.ends_with(".xml"));
            assert!(!entry.panes.is_empty(), "{} ne montre rien", entry.id);
            assert!(!entry.buttons.is_empty(), "{} ne clique rien", entry.id);
            for other in &WINDOWS[index + 1..] {
                assert_ne!(entry.id, other.id, "deux fenetres, un id");
                assert_ne!(entry.java, other.java, "deux fenetres, une classe");
            }
            // Les deux jointures repondent la meme fenetre, donc la table est une bijection.
            assert_eq!(window(entry.id).map(|found| found.java), Ok(entry.java));
            assert_eq!(window_of(entry.java).map(|found| found.id), Ok(entry.id));
        }
        assert_eq!(
            WINDOWS.iter().map(|entry| entry.java).collect::<Vec<_>>(),
            [
                "WindowSchematicAnalyzer",
                "WindowSuggestBuildTool",
                "WindowSupplies",
                "WindowSupplyStory"
            ]
        );
    }

    #[test]
    fn no_window_of_this_lot_has_a_title_pane_and_none_is_a_src_ui_row() {
        // Les quatre XML n'ont aucun panneau de titre : `title` est `None` quatre fois, et
        // c'est la difference de forme avec `crate::ui::screen::Screen`.
        for entry in WINDOWS {
            assert_eq!(entry.title, None, "{} a un titre ?", entry.id);
            // Et aucune n'est deja une ligne de `src/ui` : pas de deuxieme description.
            assert!(
                crate::ui::screen_by_id(entry.id).is_err(),
                "{} est deja un ecran de src/ui",
                entry.id
            );
        }
    }

    #[test]
    fn every_window_this_lot_opens_is_one_of_the_four() {
        let mut opened = 0;
        for entry in WINDOWS {
            for action in entry.buttons {
                if let Exit::OpensHere(target) = action.press.exit {
                    opened += 1;
                    assert!(
                        window(target).is_ok(),
                        "{}.{} ouvre {target}, que cette table n'a pas",
                        entry.id,
                        action.id
                    );
                }
            }
        }
        // Un seul bouton de ce lot ouvre un autre ecran du lot : `place` du recit.
        assert_eq!(opened, 1);
        assert_eq!(
            supplies::STORY_WINDOW.button("place").map(|action| action.press.exit),
            Some(Exit::OpensHere(supplies::SUPPLIES_ID))
        );
    }

    #[test]
    fn twelve_panes_and_fourteen_buttons_and_the_two_anomalies_are_counted() {
        let panes: usize = WINDOWS.iter().map(|entry| entry.panes.len()).sum();
        let buttons: usize = WINDOWS.iter().map(|entry| entry.buttons.len()).sum();
        assert_eq!(panes, 12);
        assert_eq!(buttons, 14);
        // Neuf panneaux de l'analyseur vivent dans un sous-arbre : six dans la boite, trois
        // dans sa liste.
        assert_eq!(WINDOWS[0].panes.iter().filter(|pane| pane.is_row_pane()).count(), 9);
        assert_eq!(WINDOWS[0].values().len(), 5);
        // Un bouton sur quatorze n'a pas de panneau, et treize en ont un.
        let reachable: usize = WINDOWS.iter().map(|entry| entry.reachable().len()).sum();
        assert_eq!(reachable, 13);
        assert_eq!(buttons - reachable, 1);
        // Le panneau-bouton sans gestionnaire est le seul des douze a etre un `<button>`
        // du XML sans figurer dans une table de boutons.
        let (id, _, _) = analyzer::UNWIRED_PANE;
        assert_eq!(WINDOWS[0].button(id), None);
        assert!(WINDOWS[0].pane(id).is_some());
        // Aucune fenetre ne montre un niveau, une progression ou un stock.
        for entry in WINDOWS {
            assert!(!entry.shows(Value::Level), "{} ", entry.id);
            assert!(!entry.shows(Value::Progress), "{} ", entry.id);
            assert!(!entry.shows(Value::Stock), "{} ", entry.id);
        }
    }

    #[test]
    fn an_id_or_a_class_that_is_not_here_is_refused_by_name() {
        // Un id de `src/ui` n'est pas un id de ce lot, et reciproquement.
        assert_eq!(window("hut.worker"), Err(Refusal::UnknownWindow));
        assert_eq!(window("hire"), Err(Refusal::UnknownWindow));
        assert_eq!(window_of("WindowHireWorker"), Err(Refusal::UnknownWindow));
        assert_eq!(Refusal::UnknownWindow.name(), "window.refusal.unknown_window");
        // Et un bouton qui n'existe pas a son propre refus.
        assert_eq!(button("analyzer", "hire"), Err(Refusal::UnknownButton));
        assert_eq!(button("analyzer", "showresources").map(|found| found.press), Ok(Press::STAYS));
        assert_eq!(button("nope", "cancel"), Err(Refusal::UnknownWindow));
    }
}
