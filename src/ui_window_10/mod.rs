//! Quatre fenêtres, leur état, et ce que leurs boutons font passer.
//!
//! Java reference : `core/client/gui/containers/WindowRack.java` (155 lignes),
//! `core/client/gui/huts/WindowBarracksBuilding.java` (173),
//! `core/client/gui/huts/WindowHutBuilderModule.java` (67) et
//! `core/client/gui/huts/WindowHutLiving.java` (125) ; leurs XML sous
//! `assets/minecolonies/gui/`, sauf le rack qui n'en a pas.
//!
//! **Trois des quatre sont déjà des lignes de `src/ui`, et ce module ne les redécrit
//! pas.** `SCREENS` porte `hut.barracks` (`WindowBarracksBuilding`), `hut.builder`
//! (`WindowHutBuilderModule`) et `hut.living` (`WindowHutLiving`) avec leurs panneaux,
//! leurs boutons, leur XML et **la source de chaque valeur**
//! ([`crate::ui::Source`]). Ce qui manquait à cette table est d'un autre ordre, et c'est
//! tout ce que ce module ajoute :
//!
//! - **l'état** : le niveau, les points de spawn, les résidents, la variante du rack —
//!   ce qu'un écran lit à l'ouverture et qui n'est pas une déclaration statique ;
//! - **les transitions** : ce qu'un clic fait passer, exprimé avec les [`crate::ui::Effect`]
//!   de `src/ui` et non avec un vocabulaire à nous ;
//! - **les calculs que Java fait au moment de remplir** : la géométrie du rack, les
//!   paliers de distance, la ligne d'un résident, la porte du builder.
//!
//! La quatrième, `WindowRack`, n'est qu'un **chemin** ailleurs : `crate::blocks` dit
//! qu'un clic sur le bloc du rack ouvre `Window::Rack` (`BlockMinecoloniesRack.java:234-241`),
//! et personne ne décrit ce qu'il y a derrière. C'est le seul écran de cette tranche
//! décrit en entier ici — [`rack::WINDOW`].
//!
//! # Les comptes, mesurés dans le source
//!
//! | Quoi | Combien | Où |
//! | --- | --- | --- |
//! | fenêtres décrites | 4 | [`WINDOWS`] |
//! | déjà une ligne de `src/ui` | 3 | [`describe`] |
//! | décrite en entier ici | **1** — `WindowRack` | [`rack::WINDOW`] |
//! | panneaux à id dans le rack | **0** | [`rack::WINDOW`] |
//! | boutons enregistrés par le rack | **0** | [`rack::try_action`] |
//! | boutons propres aux quatre | 3 — `hireSpies`, `assign`, `recall` | [`describe`] |
//! | textes écrits sans panneau (rack) | 2 | [`rack::inventory_title_pos`] |
//! | combinaisons de rack atteignables | 20 → 27 à 108 slots | [`rack::REACHABLE`] |
//! | textures de fond du rack | 5 — `generic_108` + 4 nommées | [`rack::ROW_TEXTURES`] |
//! | paliers de distance | 3 | [`barracks::Distance`] |
//! | directions | 11 | [`barracks::Direction`] |
//! | refus nommés | 8, dont 1 repris de `src/ui` | [`refusal::Refusal`] |
//!
//! # Trois prémisses que le source contredit
//!
//! 1. **« Ces écrans montrent nom, niveau, progression et stock. »** Aucun des quatre
//!    n'affiche de **progression** ni de **stock**, et le rack n'affiche même pas de
//!    niveau. Le partage réel :
//!
//!    | fenêtre | nom | niveau | progression | stock |
//!    | --- | --- | --- | --- | --- |
//!    | `WindowRack` | le titre du conteneur (`:105`), **pas** celui d'un bâtiment | — | — | des cases, aucun texte de stock |
//!    | `WindowBarracksBuilding` | titre du bâtiment | titre | — | — |
//!    | `WindowHutBuilderModule` | titre du bâtiment | titre | — | — |
//!    | `WindowHutLiving` | un nom par résident (`:95`) | titre | — | un **remplissage** `n/max` (`:108`), pas une progression de construction |
//!
//!    La progression (`WindowBuilderResModule.java:105`) et le stock
//!    (`:209`) sont dans l'onglet des réserves du builder, qui est **une autre fenêtre**
//!    et une ligne de `src/ui` (`tab.resources`). Le seul couple de nombres de cette
//!    tranche est l'occupation d'une maison, que `src/ui` classe
//!    [`crate::ui::Value::Occupancy`] — un remplissage, pas un avancement.
//!
//! 2. **« Les actions sont embaucher, renvoyer, prioriser. »** Aucune des quatre ne fait
//!    les trois. `hire`/`fire` vivent dans `WindowHireWorker` et
//!    `WindowAssignCitizen` (`src/ui_window_03`), la priorité de livraison dans
//!    `AbstractWindowWorkerModuleBuilding.java:113-131` — la fenêtre du worker, que le
//!    builder **hérite** mais n'écrit pas. Ce que ces quatre enregistrent vraiment est
//!    trois boutons : `hireSpies` (`:90`, ouvre une fenêtre que ce port ne décrit pas),
//!    `assign` (`:62`, ouvre `WindowAssignCitizen`) et `recall` (`:63`, envoie
//!    `RecallCitizenHutMessage`). Le rack n'en enregistre **aucun**.
//! 3. **« Ce sont quatre fenêtres de hutte. »** Le rack n'en est pas une : il étend
//!    `AbstractContainerScreen` (`WindowRack.java:21`), n'a ni XML, ni `registerButton`,
//!    ni `findPaneOfTypeByID`, et `src/ui` ne peut pas le porter sans mentir sur sa
//!    forme. Il est ici pour cette raison exacte, et [`Entry`] a deux variantes parce
//!    qu'il y a deux cas.
//!
//! # Ce qui n'est pas ici, et ce que chaque trou coûte
//!
//! - **le dessin** : ni `pos`, ni `size`, ni sprite. `src/ui/MANQUANT.md` §1 dit déjà
//!   qu'il n'y a pas de widget de liste dans le SDK ; le rack ajoute un manque à lui —
//!   **aucun widget de grille de cases** n'existe, et `src/ui_window_10/MANQUANT.md` §1
//!   le nomme avec les deux textes sans panneau.
//! - **les valeurs** : comme `src/ui`, ce module dit *d'où* vient une valeur et
//!   *comment* Java la compose, jamais ce qu'elle vaut. `crate::colony::Citizen` porte un
//!   nom mais **pas de métier**, alors que la ligne d'un résident en a besoin
//!   ([`living::Resident`]) ; c'est le manque le plus concret de cette tranche.
//! - **l'envoi** : deux actions de ces quatre fenêtres portent le nom d'un message Java
//!   (`RecallCitizenHutMessage`, et le clic de case du `ContainerRack`) et rien n'est
//!   envoyé. Le port n'a pas de tranche réseau.
//! - **la fenêtre des espions** : `hireSpies` ouvre `WindowsBarracksSpies`
//!   (`WindowBarracksBuilding.java:106`), que ce module nomme
//!   ([`crate::ui::Effect::OpensJava`]) et ne décrit pas.

pub mod barracks;
pub mod builder;
pub mod living;
pub mod rack;
pub mod refusal;

use crate::ui::Screen;

use refusal::Refusal;

/// Les quatre classes Java que ce module décrit, dans l'ordre de la mission.
pub const WINDOWS: &[&str] = &[rack::JAVA, barracks::JAVA, builder::JAVA, living::JAVA];

/// Une des quatre fenêtres, telle que cette table la répond.
///
/// Deux formes, parce qu'il y a deux cas et qu'un seul type mentirait : trois fenêtres
/// **sont** des lignes de `src/ui` (on rend la ligne, pas une copie), et la quatrième
/// n'en est pas une et n'en sera jamais une.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two shapes, and a third would be another module's screen
pub enum Entry {
    /// Une ligne de [`crate::ui::screen::SCREENS`], rendue telle quelle.
    Table(&'static Screen),
    /// `WindowRack`, que `src/ui` ne peut pas porter : ce n'est pas une fenêtre BlockUI.
    Rack(&'static rack::Window),
}

impl Entry {
    /// L'id de l'écran : celui de `src/ui` pour les trois lignes, celui de
    /// [`rack::ID`] pour le rack.
    pub fn id(self) -> &'static str {
        match self {
            Entry::Table(screen) => screen.id,
            Entry::Rack(window) => window.id,
        }
    }

    /// La classe Java qui le construit.
    pub fn java(self) -> &'static str {
        match self {
            Entry::Table(screen) => screen.window,
            Entry::Rack(window) => window.java,
        }
    }

    /// La ressource : un XML pour les trois huttes, une **texture** pour le rack.
    pub fn resource(self) -> &'static str {
        match self {
            Entry::Table(screen) => screen.resource,
            Entry::Rack(window) => window.resource,
        }
    }

    /// La ligne de `src/ui`, quand il y en a une — `None` pour le rack, et ce `None` est
    /// la raison d'être de cette énumération.
    pub fn screen(self) -> Option<&'static Screen> {
        match self {
            Entry::Table(screen) => Some(screen),
            Entry::Rack(_) => None,
        }
    }

    /// Les panneaux à id, dans l'ordre de la fenêtre — vide pour le rack.
    pub fn fields(self) -> &'static [crate::ui::Field] {
        match self {
            Entry::Table(screen) => screen.fields,
            Entry::Rack(window) => window.fields,
        }
    }

    /// Les boutons, dans l'ordre de la fenêtre — vide pour le rack.
    pub fn actions(self) -> &'static [crate::ui::Action] {
        match self {
            Entry::Table(screen) => screen.actions,
            Entry::Rack(window) => window.actions,
        }
    }
}

/// La fenêtre de cette classe Java, ou [`Refusal::UnknownWindow`].
///
/// C'est la jointure avec `src/ui` : `crate::blocks::Window::Rack` tombe sur le rack, et
/// `hut.barracks` / `hut.builder` / `hut.living` tombent sur leur ligne.
pub fn describe(java: &str) -> Result<Entry, Refusal> {
    match java {
        rack::JAVA => Ok(Entry::Rack(&rack::WINDOW)),
        barracks::JAVA => Ok(Entry::Table(crate::ui::screen_by_id(
            crate::ui::screen::HUT_BARRACKS_ID,
        )?)),
        builder::JAVA => Ok(Entry::Table(crate::ui::screen_by_id(
            crate::ui::screen::HUT_BUILDER_ID,
        )?)),
        living::JAVA => Ok(Entry::Table(crate::ui::screen_by_id(
            crate::ui::screen::HUT_LIVING_ID,
        )?)),
        _ => Err(Refusal::UnknownWindow),
    }
}

/// La fenêtre de cet id, ou [`Refusal::UnknownWindow`].
///
/// Trois ids sont ceux de `src/ui` (`hut.barracks`, `hut.builder`, `hut.living`) et le
/// quatrième est celui de ce module (`rack`). Un id d'un **autre** écran de `src/ui` —
/// `hire`, `tab.resources` — est refusé : cette table ne répond que de ses quatre
/// fenêtres, même quand `src/ui` sait les nommer.
pub fn describe_id(id: &str) -> Result<Entry, Refusal> {
    if id == rack::ID {
        return Ok(Entry::Rack(&rack::WINDOW));
    }
    let screen = crate::ui::screen_by_id(id).map_err(Refusal::Port)?;
    if WINDOWS.contains(&screen.window) {
        Ok(Entry::Table(screen))
    } else {
        Err(Refusal::UnknownWindow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Value;

    #[test]
    fn the_four_windows_are_answered_by_class_and_three_of_them_are_src_ui_rows() {
        assert_eq!(WINDOWS.len(), 4);
        let mut from_table = 0;
        let mut described_here = 0;
        for java in WINDOWS {
            let entry = describe(java).unwrap_or_else(|refusal| panic!("{java}: {refusal}"));
            assert_eq!(entry.java(), *java);
            // L'id répond la même fenêtre que la classe : la table est une bijection.
            assert_eq!(describe_id(entry.id()).map(Entry::java), Ok(*java));
            match entry.screen() {
                Some(screen) => {
                    from_table += 1;
                    assert_eq!(screen.window, *java);
                    assert!(screen.resource.starts_with("minecolonies:gui/"));
                }
                None => {
                    described_here += 1;
                    assert_eq!(*java, rack::JAVA);
                }
            }
        }
        assert_eq!(from_table, 3, "barracks, builder, living sont des lignes de src/ui");
        assert_eq!(described_here, 1, "WindowRack");
    }

    #[test]
    fn the_rack_is_the_one_screen_src_ui_does_not_carry() {
        // Ce que `crate::blocks` dit : un clic sur le bloc ouvre cette fenêtre-là
        // (`BlockMinecoloniesRack.java:234-241`), et c'est le seul chemin vers le rack.
        let rack_block = crate::blocks::block(rack::RACK_BLOCK)
            .unwrap_or_else(|| panic!("le rack manque à la table des blocs"));
        assert_eq!(rack_block.click, crate::blocks::Click::Rack);
        let rack_entry = describe(rack::JAVA).unwrap_or_else(|refusal| panic!("{refusal}"));
        assert!(rack_entry.screen().is_none(), "aucune ligne de SCREENS ne la décrit");
        assert!(crate::ui::screen_by_id(rack::ID).is_err(), "et l'id n'est pas dans src/ui");
        // Elle n'a ni XML, ni panneau, ni bouton : c'est ce qui la met à part.
        assert!(rack_entry.resource().ends_with(".png"));
        assert!(rack_entry.fields().is_empty());
        assert!(rack_entry.actions().is_empty());
        // Les trois autres, elles, sont dans la table.
        for java in [barracks::JAVA, builder::JAVA, living::JAVA] {
            let entry = describe(java).unwrap_or_else(|refusal| panic!("{refusal}"));
            assert!(entry.screen().is_some(), "{java}");
            assert!(!entry.fields().is_empty(), "{java} a des panneaux");
            assert!(!entry.actions().is_empty(), "{java} a des boutons");
        }
        // Et les trois boutons propres à ces quatre fenêtres sont bien ceux-là :
        // `hireSpies` dans la caserne, `assign` et `recall` dans la maison.
        let barracks_entry =
            describe(barracks::JAVA).unwrap_or_else(|refusal| panic!("{refusal}"));
        let living_entry = describe(living::JAVA).unwrap_or_else(|refusal| panic!("{refusal}"));
        assert!(barracks_entry
            .actions()
            .iter()
            .any(|action| action.id == barracks::SPIES_BUTTON));
        assert!(living_entry
            .actions()
            .iter()
            .any(|action| action.id == living::BUTTON_ASSIGN));
        assert!(living_entry
            .actions()
            .iter()
            .any(|action| action.id == living::BUTTON_RECALL));
        assert!(!living_entry
            .actions()
            .iter()
            .any(|action| action.id == barracks::SPIES_BUTTON));
    }

    #[test]
    fn a_window_outside_this_slice_is_refused_by_name() {
        // `src/ui` les nomme, mais cette table ne les décrit pas.
        assert_eq!(describe("WindowHireWorker"), Err(Refusal::UnknownWindow));
        assert_eq!(describe("WindowMainPage"), Err(Refusal::UnknownWindow));
        assert_eq!(describe(""), Err(Refusal::UnknownWindow));
        assert_eq!(describe_id("hire"), Err(Refusal::UnknownWindow));
        assert_eq!(describe_id("tab.resources"), Err(Refusal::UnknownWindow));
        assert_eq!(describe_id("hut.worker"), Err(Refusal::UnknownWindow));
        // `src/ui` sait pourtant nommer ces trois-là : le refus est bien celui de ce
        // module, pas une méconnaissance de la table.
        assert!(crate::ui::screen_by_id("hire").is_ok());
        assert!(crate::ui::screen_by_id("hut.worker").is_ok());
        // Le refus est nommé et muet.
        let refusal = describe_id("hire").err().unwrap_or_else(|| panic!("refus"));
        assert_eq!(refusal.name(), "ui10.refusal.unknown_window");
        assert_eq!(refusal.key(), None);
        // Et aucun des quatre écrans ne montre une progression ou un stock.
        for java in WINDOWS {
            let entry = describe(java).unwrap_or_else(|refusal| panic!("{refusal}"));
            for field in entry.fields() {
                assert_ne!(field.value, Value::Progress, "{java} / {}", field.id);
                assert_ne!(field.value, Value::Stock, "{java} / {}", field.id);
                assert_ne!(field.value, Value::Step, "{java} / {}", field.id);
            }
        }
    }
}
