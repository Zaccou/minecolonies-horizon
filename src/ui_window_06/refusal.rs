//! Pourquoi une lecture ou un clic de ces quatre fenêtres est refusé — nommé.
//!
//! **Aucune de ces quatre fenêtres ne parle au joueur.** Java dit non de trois
//! façons, et les trois sont muettes : un bouton qu'on n'ajoute jamais
//! (`WindowResearchTree.java:956-967` : les états verrouillés dessinent une `Image`
//! sans id, donc il n'y a rien à cliquer), un bouton qu'on éteint
//! (`WindowRequestDetail.java:160-161`), et un `return` au milieu d'`onOpened`
//! (`:151`). D'où [`Refusal::key`] qui rend `None` **partout** : un appelant ne doit
//! jamais imprimer [`Refusal::name`] là où le joueur attend une phrase.
//!
//! [`Refusal::Gate`] reprend un refus de `crate::research::gate` **tel quel** — les
//! deux tables ne peuvent pas diverger sur ce que « le parent n'est pas fini » veut
//! dire, exactement comme `ui::action::Refusal::HutRefuses` reprend `src/jobs`.

use std::fmt;

use crate::research::Refusal as GateRefusal;

/// Un refus de ces quatre fenêtres.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set for four windows, closed on purpose
pub enum Refusal {
    /// Un id ou une classe que cette table ne décrit pas.
    UnknownWindow,
    /// La requête n'est pas à la racine de l'arbre : seul un nœud de profondeur
    /// zéro s'annule (`RequestTreeWindowModule.java:258-269`, `depth() <= 0`).
    NotCancellable,
    /// `isFulfillable` rend faux — toujours, pour le module de la clipboard
    /// (`RequestTreeWindowModule.java:274-277`).
    NotFulfillable,
    /// Aucun résolveur pour cette requête : Java avertit et sort d'`onOpened`
    /// (`WindowRequestDetail.java:146-158`), laissant le panneau `resolver` vide.
    ResolverMissing,
    /// L'id du bouton n'est pas une `ResourceLocation` valide
    /// (`WindowResearchTree.java:170`, `:196`).
    InvalidId { id: String },
    /// L'id est valide mais ne nomme aucune recherche de l'arbre (`:197`).
    NotAResearch { id: String },
    /// L'université est trop basse pour cette profondeur et n'est pas au maximum
    /// (`:198-199`).
    UniversityTooLow { depth: u8, level: i32 },
    /// Autant de recherches en cours que de niveaux d'université (`:203`).
    TooManyInProgress { level: i32, running: usize },
    /// `research.hasEnoughResources(player, pos)` est faux et le joueur n'est pas en
    /// créatif (`:204`). Le détail du manque est celui de `gate::affordable`, que la
    /// fenêtre n'a pas à connaître : elle ne teste qu'un booléen.
    CannotPay,
    /// La liste des coûts de réinitialisation n'est pas dans cette tranche : le
    /// bouton d'annulation d'une recherche finie ne peut pas être chiffré
    /// (`GlobalResearchTree.java:302-344`, serveur), voir `MANQUANT.md` §4.
    ResetCostUnknown,
    /// Il manque des objets pour payer l'annulation d'une recherche finie
    /// (`WindowResearchTree.java:779-810`).
    NotEnoughForReset,
    /// Un refus que `crate::research::gate` nomme déjà, repris tel quel.
    Gate(GateRefusal),
}

impl Refusal {
    /// Un nom stable, préfixé par la fenêtre à laquelle il appartient.
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::UnknownWindow => "window.refusal.unknown",
            Refusal::NotCancellable => "request.refusal.not_root",
            Refusal::NotFulfillable => "request.refusal.not_fulfillable",
            Refusal::ResolverMissing => "request.refusal.no_resolver",
            Refusal::InvalidId { .. } => "research.refusal.invalid_id",
            Refusal::NotAResearch { .. } => "research.refusal.unknown",
            Refusal::UniversityTooLow { .. } => "research.refusal.university_level",
            Refusal::TooManyInProgress { .. } => "research.refusal.too_many",
            Refusal::CannotPay => "research.refusal.cost",
            Refusal::ResetCostUnknown => "research.refusal.reset_cost_unknown",
            Refusal::NotEnoughForReset => "research.refusal.reset_items",
            Refusal::Gate(inner) => inner.name(),
        }
    }

    /// La clé de traduction que le joueur reçoit. **Aucune** : ces quatre fenêtres
    /// éteignent un bouton ou sortent d'une fonction, elles ne disent rien
    /// (`WindowRequestDetail.java:160-161`, `WindowResearchTree.java:956-967`).
    pub fn key(&self) -> Option<&'static str> {
        None
    }

    /// L'id dont le refus parle, quand il en a un.
    pub fn about(&self) -> Option<&str> {
        match self {
            Refusal::InvalidId { id } | Refusal::NotAResearch { id } => Some(id),
            Refusal::Gate(inner) => inner.about(),
            _ => None,
        }
    }

    /// Vrai quand le refus vient de `crate::research::gate` et non d'une fenêtre.
    pub fn is_gate(&self) -> bool {
        matches!(self, Refusal::Gate(_))
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())?;
        match self {
            Refusal::InvalidId { id } | Refusal::NotAResearch { id } => write!(formatter, ": {id}"),
            Refusal::UniversityTooLow { depth, level } => {
                write!(formatter, ": depth {depth} needs a level-{depth} university, this one is {level}")
            }
            Refusal::TooManyInProgress { level, running } => {
                write!(formatter, ": {running} running for a level-{level} university")
            }
            Refusal::Gate(inner) => write!(formatter, ": {inner}"),
            _ => Ok(()),
        }
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<Refusal> {
        vec![
            Refusal::UnknownWindow,
            Refusal::NotCancellable,
            Refusal::NotFulfillable,
            Refusal::ResolverMissing,
            Refusal::InvalidId { id: "nope".to_owned() },
            Refusal::NotAResearch { id: "civilian/nope".to_owned() },
            Refusal::UniversityTooLow { depth: 3, level: 1 },
            Refusal::TooManyInProgress { level: 2, running: 2 },
            Refusal::CannotPay,
            Refusal::ResetCostUnknown,
            Refusal::NotEnoughForReset,
            Refusal::Gate(GateRefusal::Immutable { id: "civilian/agile" }),
        ]
    }

    #[test]
    fn every_refusal_has_its_own_namespaced_name() {
        let all = all();
        for (index, refusal) in all.iter().enumerate() {
            let name = refusal.name();
            assert!(
                name.starts_with("window.refusal.")
                    || name.starts_with("request.refusal.")
                    || name.starts_with("research.refusal.")
                    || name.starts_with("minecolonies:research/"),
                "{name} n'est pas préfixé"
            );
            for other in &all[index + 1..] {
                assert_ne!(name, other.name(), "deux refus, un nom");
            }
        }
        assert_eq!(all.len(), 12);
    }

    #[test]
    fn none_of_these_refusals_is_a_sentence() {
        // Java éteint un bouton ou sort d'une fonction : il ne dit rien au joueur.
        for refusal in all() {
            assert_eq!(refusal.key(), None, "{refusal} parlerait au joueur");
        }
    }

    #[test]
    fn a_gate_refusal_keeps_the_research_tables_own_name() {
        let inner = GateRefusal::ParentUnfinished { parent: "civilian/agile" };
        let wrapped = Refusal::Gate(inner.clone());
        assert_eq!(wrapped.name(), inner.name());
        assert!(wrapped.is_gate());
        assert_eq!(wrapped.about(), Some("civilian/agile"));
        assert!(!Refusal::NotFulfillable.is_gate());
        // Deux refus internes différents restent deux noms différents.
        let other = GateRefusal::Immutable { id: "civilian/agile" };
        assert_ne!(Refusal::Gate(inner).name(), Refusal::Gate(other).name());
        // Et un refus de fenêtre ne prend pas le nom d'un refus de recherche.
        assert_ne!(Refusal::NotEnoughForReset.name(), Refusal::Gate(
            GateRefusal::Immutable { id: "civilian/agile" }
        ).name());
    }
}
