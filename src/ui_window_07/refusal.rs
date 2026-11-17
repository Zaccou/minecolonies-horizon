//! Pourquoi une lecture ou un clic de ces quatre fenetres est refuse — nomme.
//!
//! Java dit non de quatre facons, et les quatre sont ici : une phrase au joueur
//! (`item.buildtool.missing`, `WindowSuggestBuildTool.java:84`), un `return` muet
//! (`WindowSupplies.java:138-141` : ni pack ni blueprint, donc rien ne se passe et rien ne
//! se dit), un avertissement de journal (`WindowSchematicAnalyzer.java:334-339`,
//! `Log.getLogger().warn("Nonexisting pane")`), et une **exception** qui tombe
//! (`WindowSchematicAnalyzer.java:129-131` lit `selected.differentBlocks` sur une selection
//! nulle : `NullPointerException`).
//!
//! [`Refusal::key`] ne repond `Some` que dans le premier cas : un appelant ne doit donc
//! **jamais** imprimer [`Refusal::name`] la ou le joueur attend une phrase.
//!
//! **Les quatre fautes de placement ne sont pas ici.** Elles ne refusent pas un *clic*,
//! elles decrivent un *balayage* du monde, et `item.supply.error.<type>` n'est pas la phrase
//! du refus mais l'etiquette d'une boite en surbrillance : elles vivent dans
//! [`super::supplies::Fault`], avec leur position et leur compte.

use std::fmt;

/// Un refus de ces quatre fenetres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: l'ensemble ferme de ce lot, et chaque variante a sa source
pub enum Refusal {
    /// Un id ou une classe que [`super::WINDOWS`] ne porte pas.
    UnknownWindow,
    /// Un id de bouton qu'aucune des quatre fenetres n'enregistre.
    UnknownButton,
    /// La boite `results` n'existe pas dans le sous-arbre vise : Java ecrit
    /// `"Nonexisting pane"` au journal et sort (`WindowSchematicAnalyzer.java:334-339`).
    /// Pas de cle : le joueur ne voit rien.
    MissingPane,
    /// `showresources` sans selection : Java lit `selected.differentBlocks` sur un `null`
    /// (`WindowSchematicAnalyzer.java:129-131`) et **leve**.
    NothingSelected,
    /// L'inventaire ne contient pas l'outil de construction : le joueur recoit
    /// `item.buildtool.missing` et la fenetre se ferme
    /// (`WindowSuggestBuildTool.java:84-85`).
    MissingBuildTool,
    /// Ni pack selectionne ni blueprint charge : `handlePlacement` revient sans un mot
    /// (`WindowSupplies.java:138-141`). Le clic ne fait donc **rien**, pas meme un son.
    NothingToPlace,
}

impl Refusal {
    /// Un nom stable, prefixe par la fenetre a laquelle il appartient.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::UnknownWindow => "window.refusal.unknown_window",
            Refusal::UnknownButton => "window.refusal.unknown_button",
            Refusal::MissingPane => "analyzer.refusal.missing_pane",
            Refusal::NothingSelected => "analyzer.refusal.nothing_selected",
            Refusal::MissingBuildTool => "suggest.refusal.missing_build_tool",
            Refusal::NothingToPlace => "supplies.refusal.nothing_to_place",
        }
    }

    /// La cle de traduction que le joueur recoit, quand il en recoit une.
    ///
    /// Une seule de ces six parle (`TranslationConstants.java:496`) : les cinq autres
    /// sortent en silence, journalisent, ou levent.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Refusal::MissingBuildTool => Some("item.buildtool.missing"),
            Refusal::UnknownWindow
            | Refusal::UnknownButton
            | Refusal::MissingPane
            | Refusal::NothingSelected
            | Refusal::NothingToPlace => None,
        }
    }

    /// Vrai quand le clic **ferme** la fenetre en plus de refuser.
    ///
    /// Un refus qui ferme n'est pas un refus qui laisse la fenetre ouverte : Java envoie le
    /// message puis `close()` (`WindowSuggestBuildTool.java:84-85`), donc un appelant qui
    /// traite le refus sans fermer laisse le joueur devant une fenetre morte.
    pub fn closes(self) -> bool {
        matches!(self, Refusal::MissingBuildTool)
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

    /// Les six refus de ce lot, dans l'ordre de la declaration.
    const ALL: [Refusal; 6] = [
        Refusal::UnknownWindow,
        Refusal::UnknownButton,
        Refusal::MissingPane,
        Refusal::NothingSelected,
        Refusal::MissingBuildTool,
        Refusal::NothingToPlace,
    ];

    #[test]
    fn every_refusal_has_its_own_namespaced_name() {
        for (index, refusal) in ALL.iter().enumerate() {
            assert!(refusal.name().starts_with("window.refusal.")
                || refusal.name().starts_with("analyzer.refusal.")
                || refusal.name().starts_with("suggest.refusal.")
                || refusal.name().starts_with("supplies.refusal."),
                "{refusal} n'est pas namespacé");
            assert_eq!(refusal.to_string(), refusal.name());
            for other in &ALL[index + 1..] {
                assert_ne!(refusal.name(), other.name(), "deux refus, un nom");
            }
        }
    }

    #[test]
    fn one_refusal_speaks_and_the_other_five_do_not() {
        let spoken: Vec<&str> = ALL
            .iter()
            .filter(|refusal| refusal.key().is_some())
            .map(|refusal| refusal.name())
            .collect();
        assert_eq!(spoken, ["suggest.refusal.missing_build_tool"]);
        assert_eq!(Refusal::MissingBuildTool.key(), Some("item.buildtool.missing"));
        // Le `return` muet de `handlePlacement` et le `warn` de l'analyseur n'ont pas de cle.
        assert_eq!(Refusal::NothingToPlace.key(), None);
        assert_eq!(Refusal::MissingPane.key(), None);
        // Un refus qui ferme, et cinq qui laissent la fenetre en place.
        assert!(Refusal::MissingBuildTool.closes());
        assert!(!Refusal::NothingToPlace.closes());
        assert!(!Refusal::NothingSelected.closes());
    }
}
