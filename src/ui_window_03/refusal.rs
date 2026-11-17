//! Pourquoi un clic de ces quatre fenêtres ne fait rien — nommé.
//!
//! Java dit non de quatre manières et les quatre sont ici : une **exception** qui tombe
//! (`WindowDecorationController.java:118-121` attrape et journalise), un bouton **caché**
//! (`:64-65`, `:96`, `:112`), un bouton **éteint** (`WindowHireWorker.java:407-447`), et
//! un `return` muet (`:112-115`, `:145`). [`Refusal::key`] ne répond `Some` que quand
//! Java parle au joueur ; partout ailleurs l'appelant ne doit **pas** imprimer
//! [`Refusal::name`] comme une phrase.
//!
//! [`Refusal::Port`] reprend un refus de `src/ui` **tel quel** : les deux slices ne
//! peuvent pas diverger sur ce que « il n'y a plus de place » veut dire, exactement comme
//! `ui::action::Refusal::HutRefuses` reprend ceux de `src/jobs`.

use std::fmt;

/// Un refus de ces quatre fenêtres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this slice's own closed set, plus the `src/ui` forwards
pub enum Refusal {
    /// Un id de fenêtre que [`crate::ui_window_03::WINDOWS`] ne porte pas.
    UnknownWindow,
    /// Un id de bouton qu'aucune des quatre fenêtres n'enregistre. Java ne lève rien :
    /// `registerButton` est une `Map`, donc un id absent ne fait simplement rien
    /// (`AbstractWindowSkeleton.java:130-133`).
    UnknownButton,
    /// Un métier qui n'est pas dans les modules de cette hutte
    /// (`WindowHireWorker.java:291-306` cherche par `getKey().toString()` et sort sans
    /// rien dire si aucun module ne correspond).
    UnknownJob,
    /// `cancel` avec une colonie sans hôtel de ville : Java teste
    /// `getTownHall() != null` et **n'appelle rien** (`:145`). Silencieux.
    NoTownHall,
    /// Aucune colonie à la position du contrôleur : le bloc `if (view != null)` entier
    /// est sauté et les deux boutons restent cachés (`WindowDecorationController.java:67`).
    NoColony,
    /// Le chemin de blueprint ne finit pas par un chiffre, donc `Utils.getBlueprintLevel`
    /// rend `-1` (`api/util/Utils.java:199-219`) : pas de niveau suivant, donc pas de
    /// bouton de construction (`:101-116`). Silencieux.
    NoLevel,
    /// Le bloc primaire du blueprint n'est pas le placeholder de décoration : Java ne
    /// montre pas le bouton correspondant (`:94-97`, `:110-113`). Silencieux.
    NotPlaceholder,
    /// La lecture du pack a levé : Java écrit `Unable to retrieve blueprint` dans le log
    /// et laisse les deux boutons cachés (`:118-121`). **Le joueur ne voit rien.**
    BlueprintUnreadable,
    /// Le bouton existe dans l'XML mais Java l'a éteint pour cette ligne — le clic ne
    /// peut pas arriver. Silencieux (`WindowHireWorker.java:407-447`).
    ButtonOff,
    /// Un refus que `src/ui::action::Refusal` nomme déjà, repris tel quel.
    Port(crate::ui::Refusal),
}

impl Refusal {
    /// Un nom stable, préfixé pour ne jamais pouvoir être pris pour une phrase.
    pub fn name(self) -> &'static str {
        match self {
            Refusal::UnknownWindow => "ui03.refusal.unknown_window",
            Refusal::UnknownButton => "ui03.refusal.unknown_button",
            Refusal::UnknownJob => "ui03.refusal.unknown_job",
            Refusal::NoTownHall => "ui03.refusal.no_town_hall",
            Refusal::NoColony => "ui03.refusal.no_colony",
            Refusal::NoLevel => "ui03.refusal.no_level",
            Refusal::NotPlaceholder => "ui03.refusal.not_placeholder",
            Refusal::BlueprintUnreadable => "ui03.refusal.blueprint_unreadable",
            Refusal::ButtonOff => "ui03.refusal.button_off",
            Refusal::Port(inner) => inner.name(),
        }
    }

    /// La clé de traduction que Java montre au joueur, quand il en montre une.
    ///
    /// `None` n'est pas « pas de raison » : c'est « Java n'écrit rien », et un appelant
    /// qui afficherait [`Self::name`] à la place d'une phrase se tromperait de public.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Refusal::Port(inner) => inner.key(),
            Refusal::UnknownWindow
            | Refusal::UnknownButton
            | Refusal::UnknownJob
            | Refusal::NoTownHall
            | Refusal::NoColony
            | Refusal::NoLevel
            | Refusal::NotPlaceholder
            | Refusal::BlueprintUnreadable
            | Refusal::ButtonOff => None,
        }
    }
}

impl From<crate::ui::Refusal> for Refusal {
    fn from(inner: crate::ui::Refusal) -> Refusal {
        Refusal::Port(inner)
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Port(inner) => write!(f, "{inner}"),
            other => f.write_str(other.name()),
        }
    }
}

impl std::error::Error for Refusal {}
