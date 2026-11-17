//! Les ecrans client de MineColonies : **les donnees et l'etat, pas le dessin.**
//!
//! Java reference: `core/client/gui/` — **107 fichiers**, 21 722 lignes, mesure au
//! `find`/`wc`. Les fenetres elles-memes sont deja portees, en tout ou en partie, par
//! `src/ui` (la disposition d'une trentaine de fenetres) et par [`crate::ui_window_01`] a
//! [`crate::ui_window_17`] (les fenetres une par une). Ce module porte la couche qui
//! manquait, celle qu'aucune table de panneaux ne peut tenir :
//!
//! | Fichier | Question | Java |
//! | --- | --- | --- |
//! | [`containers`] | quelles cases, et ou va une pile maj-cliquee ? | `api/inventory/container/` |
//! | [`map_data`] | une image de carte, et l'arithmetique de zoom | `core/client/gui/map/` |
//! | [`modules`] | le niveau, la progression et le refus d'un module | `core/client/gui/modules/` |
//! | [`citizen`] | les onglets et les trois barres d'un citoyen | `core/client/gui/citizen/` |
//!
//! # Ce qui etait deja la, et que ce module ne refait pas
//!
//! Le brief citait aussi les listes de l'hotel de ville et le journal de quetes. Les deux
//! sont **deja ecrits**, et les reecrire aurait ete la faute la plus facile a commettre :
//!
//! - la page citoyens de l'hotel de ville est `ui_window_15::citizen`
//!   ([`crate::ui_window_15::citizen`]) : le filtre nom-ou-metier, le tri par nom, la ligne
//!   selectionnee dont le bouton s'eteint, les quatre visages de joie, l'arrondi au plafond
//!   du titre, les trois etiquettes courtes et **les deux plafonds en dur** (`/10` et
//!   `/20`), plus la contradiction de l'icone d'entite ;
//! - le journal de quetes est `ui_window_13::quest_log` ([`crate::ui_window_13::quest_log`]) :
//!   ses trois pages, ses cinq panneaux, l'infobulle qui n'apparait que si le texte dessine
//!   depasse son panneau, et le suivi de trente secondes.
//!
//! Deux tests de ce fichier sont les **coutures** entre ces modules et celui-ci : ils
//! verifient les valeurs et l'incoherence plutot que de les recopier.
//!
//! # Ce qui est compte
//!
//! | Dossier | Fichiers | Ici |
//! | --- | --- | --- |
//! | `containers/` | 8 | 7 menus decrits, 1 fichier vide signale |
//! | `map/` | 5 | la mecanique ; la fenetre est dans `ui_window_11` |
//! | `modules/` | 4 + 24 | 15 fenetres ; 9 sont deja dans `ui_window_05`, `11`, `12`, `13` |
//! | `citizen/` | 7 | les 7 |
//! | `townhall/`, `questlog/`, racine, `huts/`, `blockui/` | 14 + 6 + 39 | deja portes |
//!
//! # Trois premices que la source contredit
//!
//! 1. **« les conteneurs sont huit » est vrai de nom seulement.** Sept fichiers se lisent ;
//!    `api/inventory/container/ContainerField.java` fait **0 octet** et rien ne
//!    l'instancie — `WindowField` n'importe aucun conteneur. Le huitieme menu n'existe donc
//!    pas, et [`containers::EMPTY_CONTAINER_FILE`] le nomme.
//! 2. **« un module = une fenetre » est faux.** Vingt-quatre fenetres pour **157** modules
//!    declares (`BuildingModules.java`) : les fenetres sont generiques, et c'est le niveau
//!    et la liste que la vue porte qui changent. [`modules::WINDOWS`] joint donc sur la
//!    **classe de vue**, pas sur une cle de module — la jointure par cle existe deja pour
//!    les cinq modules qui ouvrent un ecran de hutte ([`crate::ui::MODULE_SCREENS`]) et ce
//!    module ne la double pas.
//! 3. **« l'etat d'un module » n'est pas une valeur.** Java n'a pas de type pour ca : un
//!    niveau lu par un bouton, un pourcentage ecrit a la main, et un refus exprime par un
//!    `disable()` ou une couleur. [`modules::State`] est donc **trois champs separes**, et
//!    [`modules::Refusal`] donne un nom a des refus que Java ne nomme pas.
//!
//! # Ce qui reste dans `MANQUANT.md`
//!
//! Un widget de liste (`src/ui/MANQUANT.md` §1 le dit deja pour `src/ui`), un identifiant
//! d'ecran dans le SDK, une barre laterale d'onglets, et le rendu d'un texte dont on mesure
//! la largeur. Aucun de ces manques n'est contourne par une API inventee : les quatre
//! fichiers qui precedent n'appellent **rien** de l'hote.

pub mod citizen;
pub mod containers;
pub mod map_data;
pub mod modules;

/// Ce qu'un fichier de ce module couvre, et sur combien de fichiers Java.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    /// Le nom du fichier Rust, sans extension.
    pub module: &'static str,
    /// Le dossier Java qu'il lit, avec son `/` final.
    pub java: &'static str,
    /// Combien de fichiers Java ce dossier contient.
    pub files: usize,
}

/// Les quatre fichiers de ce module et les dossiers qu'ils lisent.
///
/// Les quatre derniers dossiers de `core/client/gui/` (la racine, `huts/`, `blockui/`),
/// `townhall/` et `questlog/` sont hors de ce compte : cinquante-neuf fichiers qui
/// appartiennent aux tranches de fenetres.
pub const PARTS: &[Part] = &[
    Part { module: "containers", java: "api/inventory/container/", files: 8 },
    Part { module: "map_data", java: "core/client/gui/map/", files: 5 },
    Part { module: "modules", java: "core/client/gui/modules/", files: 24 },
    Part { module: "citizen", java: "core/client/gui/citizen/", files: 7 },
];

/// Les deux fichiers de `modules/` qui ne sont pas des fenetres (`IWindowModule`,
/// `IWindowWithLayoutModule`) et les deux qui sont des modules de fenetre
/// (`RequestTreeWindowModule`, `TabsWindowModule`) : le compte du dossier est donc 28,
/// dont 24 fenetres.
pub const MODULE_HELPERS: usize = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_parts_and_every_one_names_a_java_directory_that_ends_with_a_slash() {
        assert_eq!(PARTS.len(), 4);
        for (index, part) in PARTS.iter().enumerate() {
            assert!(part.java.ends_with('/'), "{}", part.java);
            assert!(part.files > 0);
            for other in &PARTS[index + 1..] {
                assert_ne!(part.module, other.module);
                assert_ne!(part.java, other.java);
            }
        }
        // Les quatre modules existent, par leur nom.
        let names: Vec<&str> = PARTS.iter().map(|part| part.module).collect();
        assert_eq!(names, ["containers", "map_data", "modules", "citizen"]);
    }

    #[test]
    fn the_counts_add_up_to_the_hundred_and_seven_files_of_the_source_directory() {
        // Les dossiers decrits ici, modules/ compris en entier (24 fenetres + 4 aides).
        assert_eq!(8 + 5 + (24 + MODULE_HELPERS) + 7, 48);
        // 107 fichiers au total : il en reste 59 hors de ces quatre dossiers.
        assert_eq!(107 - 48, 59);
        assert_eq!(MODULE_HELPERS, 4);
        let mut total = 0;
        for part in PARTS {
            total += match part.module {
                "modules" => part.files + MODULE_HELPERS,
                _ => part.files,
            };
        }
        assert_eq!(total, 48);
    }

    #[test]
    fn the_town_hall_citizen_page_is_already_written_and_this_module_only_joins_it() {
        // `ui_window_15::citizen` porte les deux plafonds en dur, le filtre et les visages,
        // et il nomme l'incoherence : `/20` n'est pas le maximum du modele.
        assert_eq!(crate::ui_window_15::citizen::SATURATION_CEILING, 20.0);
        assert_eq!(crate::ui_window_15::citizen::HAPPINESS_CEILING, 10.0);
        assert_ne!(crate::ui_window_15::citizen::SATURATION_CEILING, crate::citizens::MAX_SATURATION);
        // Et la barre d'un citoyen, qui est de ce module, dessine dix icones de six
        // points : un troisieme denominateur, different des deux autres.
        let (_, _, icons) = citizen::saturation_icons(0.0);
        assert_eq!(icons, citizen::MAX_ICONS);
        assert_eq!(icons as f64 * citizen::SATURATION_PER_ICON, crate::citizens::MAX_SATURATION);
    }

    #[test]
    fn the_quest_log_is_already_written_and_this_module_does_not_carry_it() {
        // `ui_window_13::quest_log` a la cle de suivi, la duree et la mesure de largeur.
        assert_eq!(crate::ui_window_13::quest_log::TRACK_SECONDS, 30);
        assert_eq!(crate::ui_window_13::quest_log::TRACK_HIGHLIGHT_KEY, "questLogTracking");
        assert_eq!(crate::ui_window_13::quest_log::LIST, "quests");
        assert!(!crate::ui_window_13::quest_log::tooltip(100, 100));
        assert!(crate::ui_window_13::quest_log::tooltip(101, 100));
    }

    #[test]
    fn the_module_window_table_and_the_hut_screen_table_do_not_overlap() {
        // `crate::ui::MODULE_SCREENS` joint une cle de module a un ecran de hutte ; ce
        // module joint une classe de vue a une regle. Aucun identifiant n'est partage.
        for entry in modules::WINDOWS {
            assert!(
                crate::ui::table::module_screen(entry.view).is_none(),
                "{} est deja joint par ui::MODULE_SCREENS",
                entry.view
            );
            assert!(!entry.java.is_empty());
            assert!(!crate::ui::SCREENS.iter().any(|screen| screen.window == entry.java));
        }
    }

    #[test]
    fn the_container_table_does_not_repeat_the_brewing_slot_map_of_ui_window_08() {
        // `ui_window_08::brewing` porte deja les quatre cases ; ce module ne garde que
        // les deux indices qui rendent sa regle de transfert destructrice.
        assert_eq!(containers::INGREDIENT_SCREEN_SLOT, 0);
        assert_eq!(containers::INGREDIENT_HANDLER_SLOT, 3);
        assert_ne!(containers::INGREDIENT_SCREEN_SLOT, containers::INGREDIENT_HANDLER_SLOT);
        // Et les sept menus sont bien les sept conteneurs lisibles.
        assert_eq!(containers::MENUS.len(), 7);
        assert_eq!(containers::MENUS.len() + 1, PARTS[0].files, "un fichier vide");
        // La taille d'un rack vient de `ui_window_10::rack`, pas d'ici.
        assert_eq!(crate::ui_window_10::rack::RACK_SLOTS, 27);
        assert_eq!(modules::MAX_STORAGE_UPGRADE, crate::ui_window_10::rack::MAX_STORAGE_UPGRADE);
    }

    #[test]
    fn the_citizen_module_describes_seven_windows_and_three_bars() {
        // Sept onglets pour sept fenetres : cinq toujours la, plus le debogage et le
        // metier qui dependent du citoyen.
        assert_eq!(citizen::Tab::ALWAYS.len(), 5);
        assert_eq!(citizen::tabs(true, false, true).len(), 7);
        // Trois barres, trois fonctions, trois echelles.
        assert_eq!(citizen::hearts(20).filled, citizen::MAX_ICONS);
        assert_eq!(citizen::saturation_icons(0.0).2, citizen::MAX_ICONS);
        assert_eq!(citizen::happiness_icons(20).0, citizen::MAX_ICONS);
        assert_eq!(PARTS[3].files, 7);
        // La saturation maximale est celle de `citizens::needs`, pas une copie.
        assert_eq!(citizen::MAX_SATURATION, crate::citizens::MAX_SATURATION);
    }

    #[test]
    fn the_map_arithmetic_borders_two_different_floors_than_the_window_it_serves() {
        // Le zoom s'arrete a 0.02, le dessin a 0.2622 : le module garde les deux.
        assert!(map_data::MIN_SCALE < map_data::RENDER_MIN_SCALE);
        assert_eq!(map_data::MAX_SCALE, 6.0);
        // Et la tuile d'une carte se mesure a l'echelle de la fenetre, qui part de zero.
        assert_eq!(map_data::Map::tile_size(0.0), 0);
        assert_eq!(PARTS[1].files, 5);
    }
}
