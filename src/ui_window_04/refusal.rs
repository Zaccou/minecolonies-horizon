//! Pourquoi une de ces quatre fenêtres ne s'ouvre pas, ou refuse de changer de page — nommé.
//!
//! Java dit non de trois manières, et ces fenêtres n'emploient que les deux premières : un bouton
//! **caché** (`AbstractBuildingMainWindow.java:56-59` : le `info` disparaît sans un mot) et un
//! `return` en haut du gestionnaire (`AbstractWindowSkeleton.java:220-223` : `setPage` sans
//! `switchView` ne fait rien). **Aucune des quatre n'écrit une phrase au joueur** : [`Refusal::key`]
//! répond donc `None` pour tous les variants, et un appelant ne doit jamais imprimer
//! [`Refusal::name`] là où le joueur attend une phrase.
//!
//! La seule chose que ces fenêtres disent au joueur est un **avertissement**, pas un refus : le nom
//! trop long (`WARNING_NAME_TOO_LONG`, `TranslationConstants.java:480`, levé à
//! `WindowHutNameEntry.java:62`). Il vit dans [`super::rename::Renamed`] comme une valeur de retour,
//! parce que le renommage **a lieu quand même** — exactement la raison pour laquelle
//! `crate::ui::action::rename` rend un booléen et non un `Err`.
//!
//! [`Refusal`] ne reprend aucun variant de `src/ui::action::Refusal` : les trois refus que ces
//! fenêtres partageaient avec les écrans de huttes (`NotBuilt`, `NoRoom`, `Travelling`) sont ceux
//! des boutons `hire`/`recall`, qui ne sont pas dans ce périmètre. Le jour où le minimum en
//! déclencherait un, il faudra une variante `Port(crate::ui::action::Refusal)` comme celle de
//! `src/ui_window_02`.

use std::fmt;

use crate::building::Kind;

/// Un refus d'ouverture ou de pagination, dans ces quatre fenêtres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own closed set, disjoint from `src/ui`'s
pub enum Refusal {
    /// Le bouton `info` est **caché** pour ce type de bâtiment : `I18n.exists(prefix + ".0")` est
    /// faux (`AbstractBuildingMainWindow.java:56-59`). Silencieux : le joueur ne voit pas de bouton,
    /// pas un message.
    NoInfoPage {
        /// Le type de bâtiment dont le manuel n'existe pas.
        hut: Kind,
    },
    /// Le guide ne se construit que pour l'atelier du builder : son constructeur prend un
    /// `BuildingBuilder.View` (`WindowHutGuide.java:28`), pas un `IBuildingView`. Sur un autre
    /// bâtiment Java lève un `ClassCastException` à l'appelant ; le port rend un nom.
    GuideNotForBuilder {
        /// Le type de bâtiment pour lequel le guide a été demandé.
        hut: Kind,
    },
    /// Un changement de page demandé à une fenêtre qui n'a pas de `pages` : le skeleton rend la
    /// main sans rien faire (`AbstractWindowSkeleton.java:220-223`). Le guide et le renommage sont
    /// dans ce cas, `WindowInfo` non.
    NoPages,
    /// Une classe ou un id que cette table ne décrit pas : les treize écrans de `src/ui` sauf
    /// trois, `WindowHutAllInventory`, le reste de `core/client/gui/`.
    UnknownWindow,
}

impl Refusal {
    /// Un nom stable, préfixé par la fenêtre à laquelle il appartient.
    ///
    /// Les quatre préfixes (`info.`, `guide.`, `page.`, `hut.`) sont ceux de ce module et ne
    /// peuvent pas se confondre avec les `ui.refusal.*` de `src/ui` ni les `build.refusal.*` de
    /// `src/ui_window_02`.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::NoInfoPage { .. } => "info.refusal.no_page",
            Refusal::GuideNotForBuilder { .. } => "guide.refusal.not_builder",
            Refusal::NoPages => "page.refusal.no_switch",
            Refusal::UnknownWindow => "hut.refusal.unknown_window",
        }
    }

    /// La clé de traduction que le joueur reçoit, quand il en reçoit une.
    ///
    /// `None` pour les quatre : Java cache un bouton, retourne, ou lève une exception. Un appelant
    /// doit donc répondre `None` à l'écran, pas [`Refusal::name`].
    pub fn key(self) -> Option<&'static str> {
        None
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_window_04::{guide, info, minimum};

    #[test]
    fn the_four_refusals_are_four_names_and_none_of_them_is_a_sentence() {
        let all = [
            Refusal::NoInfoPage { hut: Kind::Baker },
            Refusal::GuideNotForBuilder { hut: Kind::Farmer },
            Refusal::NoPages,
            Refusal::UnknownWindow,
        ];
        for (index, refusal) in all.iter().enumerate() {
            assert!(refusal.name().contains(".refusal."), "{refusal}");
            assert!(!refusal.name().contains(' '), "{refusal} ressemble à une phrase");
            assert_eq!(refusal.key(), None, "{refusal} est silencieux dans Java");
            for other in &all[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "deux refus, un nom");
            }
        }
        // Le nom porte la règle, pas le bâtiment : deux huttes, un seul nom.
        assert_eq!(
            Refusal::NoInfoPage { hut: Kind::Baker }.name(),
            Refusal::NoInfoPage { hut: Kind::Stash }.name()
        );
        assert_ne!(
            Refusal::NoInfoPage { hut: Kind::Baker },
            Refusal::NoInfoPage { hut: Kind::Stash }
        );
    }

    #[test]
    fn the_info_button_refuses_for_the_kinds_without_a_manual_and_the_guide_only_for_the_builder() {
        // Sans manuel : le bouton est caché, et le refus porte le type demandé.
        let nothing = |_: &str| None;
        assert_eq!(
            minimum::open_info(Kind::Baker, nothing),
            Err(Refusal::NoInfoPage { hut: Kind::Baker })
        );
        let baker = |key: &str| {
            (key == "com.minecolonies.coremod.info.baker.0").then(|| "Introduction".to_owned())
        };
        assert!(minimum::open_info(Kind::Baker, baker).is_ok());

        // Le guide n'existe que pour l'atelier du builder.
        assert_eq!(
            guide::open_for(Kind::Farmer),
            Err(Refusal::GuideNotForBuilder { hut: Kind::Farmer })
        );
        assert_eq!(guide::open_for(Kind::Builder).map(|_| ()), Ok(()));
    }

    #[test]
    fn a_window_without_a_pages_switch_refuses_a_page_change_and_window_info_does_not() {
        let g = guide::screen().unwrap_or_else(|refusal| panic!("{refusal}"));
        let n = crate::ui_window_04::rename::screen().unwrap_or_else(|refusal| panic!("{refusal}"));
        let m = minimum::screen().unwrap_or_else(|refusal| panic!("{refusal}"));
        // Les trois fenêtres de huttes n'ont pas de `pages` : le skeleton rend la main.
        for screen in [g, n, m] {
            assert_eq!(info::may_turn_page(screen), Err(Refusal::NoPages), "{}", screen.id);
        }
        // `WindowInfo` en a un (`windowinfo.xml:9`), donc la même règle passe.
        assert_eq!(info::may_turn_page(&info::WINDOW), Ok(()));
        // Et l'erreur est bien une `Error`, pour un `?` dans un futur appelant.
        let as_error: &dyn std::error::Error = &Refusal::NoPages;
        assert_eq!(as_error.to_string(), "page.refusal.no_switch");
    }
}
