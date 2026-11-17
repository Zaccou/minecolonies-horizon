//! La hutte du builder : la seule des quatre qui n'ajoute qu'une **porte d'entrée**.
//!
//! Java reference : `core/client/gui/huts/WindowHutBuilderModule.java` (67 lignes),
//! `core/client/gui/AbstractWindowWorkerModuleBuilding.java` (le parent, qui fait tout le
//! remplissage), `core/client/gui/WindowHutGuide.java:38-41` (le retour), et
//! `core/colony/buildings/workerbuildings/BuildingBuilder.java:230` (`getWindow`).
//!
//! **La classe n'ajoute rien à l'écran.** Elle étend le placeholder du worker, charge le
//! même XML (`:48`, `gui/windowhutworkerplaceholder.xml`) et ne touche **aucun** panneau :
//! pas un `findPaneOfTypeByID` dans les 67 lignes. `src/ui` le dit déjà
//! (`ui::screen::HUT_BUILDER_ID` : mêmes champs et mêmes boutons que `hut.worker`, plus
//! une `Gate`). Ce fichier porte donc la seule chose qui reste : **l'état de la porte**,
//! c'est-à-dire laquelle des deux fenêtres reste ouverte.
//!
//! # La conséquence que la porte a sur les données
//!
//! Quand l'avancement n'est pas fait, `onOpened` **sort avant `super.onOpened()`**
//! (`:60-63`) : la fenêtre se ferme, le guide s'ouvre, et **la liste des workers, la
//! priorité de livraison et le titre ne sont jamais écrits**. Ce n'est pas un détail
//! d'affichage : un appelant qui lit les champs du worker sur une ouverture de builder
//! lit des panneaux que Java n'a pas remplis. [`Opened::writes_worker_fields`] le dit.
//!
//! # Le retour ne repasse pas la porte
//!
//! `WindowHutGuide.closeGuide` rouvre la hutte avec `new WindowHutBuilderModule(building,
//! false)` (`WindowHutGuide.java:40`) : le second constructeur met `needGuide` à faux
//! (`:46-50`), donc le guide ne se rouvre pas en boucle. Le joueur peut donc fermer le
//! guide sans avoir l'avancement, et la hutte s'ouvre quand même — c'est le comportement
//! de Java, pas un contournement.

use crate::ui::{screen::HUT_GUIDE_ID, Effect};

/// La classe Java.
pub const JAVA: &str = "WindowHutBuilderModule";
/// Le XML : celui du worker, pas un fichier propre (`:48`).
pub const RESOURCE: &str = "minecolonies:gui/windowhutworkerplaceholder.xml";
/// La fenêtre que la porte ouvre à la place (`:61`).
pub const GUIDE_JAVA: &str = "WindowHutGuide";
/// L'avancement dont la porte dépend (`:23` : `MOD_ID` + `"minecolonies/check_out_guide"`).
pub const GUIDE_ADVANCEMENT: &str = "minecolonies:minecolonies/check_out_guide";

/// Les deux constructeurs de la classe, et le drapeau que chacun pose (`:35-50`).
///
/// **Le second appelant est le guide lui-même** : c'est pour lui que la porte ne se
/// referme pas.
pub const CALLERS: &[(&str, bool)] = &[
    // BuildingBuilder.java:230, par `getWindow()`
    ("BuildingBuilder.View.getWindow", true),
    // WindowHutGuide.java:40, au retour du guide
    ("WindowHutGuide.closeGuide", false),
];

/// L'état de l'avancement du guide, tel que le client le voit (`:57-58`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: Java tests `ad == null || !progress.isDone()`, which is three states
pub enum Advancement {
    /// L'avancement n'est pas dans la carte du client : `get(GUIDE_ADVANCEMENT)` rend
    /// `null` (`:57`). Une partie neuve, ou un pack qui ne le déclare pas.
    Missing,
    /// Il est là et pas fini : `progress.getOrDefault(ad, new AdvancementProgress())`
    /// n'est pas `isDone()` (`:58`).
    Started,
    /// Fini : la porte laisse passer la fenêtre de la hutte (`:65`).
    Done,
}

/// La fenêtre qui reste ouverte après `onOpened`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the two branches of WindowHutBuilderModule.onOpened
pub enum Opened {
    /// La hutte du worker, ses dix panneaux remplis par le parent (`:65`).
    Hut,
    /// Le guide, et **rien** n'a été rempli (`:60-63`).
    Guide,
}

impl Opened {
    /// La classe Java qui reste à l'écran.
    pub fn java(self) -> &'static str {
        match self {
            Opened::Hut => JAVA,
            Opened::Guide => GUIDE_JAVA,
        }
    }

    /// Les panneaux du worker ont-ils été écrits — le titre, la liste, la priorité.
    ///
    /// `false` pour [`Opened::Guide`] : `super.onOpened()` est sauté (`:62`), donc les
    /// dix panneaux de `AbstractWindowWorkerModuleBuilding.java:162-203` gardent leur
    /// valeur de XML, qui est vide.
    pub fn writes_worker_fields(self) -> bool {
        matches!(self, Opened::Hut)
    }

    /// Ce que la porte fait de la fenêtre : ouvrir le guide, ou ne rien ouvrir.
    pub fn effect(self) -> Option<Effect> {
        match self {
            Opened::Hut => None,
            Opened::Guide => Some(Effect::Opens(HUT_GUIDE_ID)),
        }
    }
}

/// La porte elle-même (`:53-66`), dans l'ordre de Java.
///
/// `need_guide` vient du constructeur, `advancement` de la carte du client. Les deux
/// conditions de Java sont `ad == null || !isDone()`, réunies par
/// [`Advancement::Missing`] et [`Advancement::Started`].
pub fn open(need_guide: bool, advancement: Advancement) -> Opened {
    if need_guide && advancement != Advancement::Done {
        Opened::Guide
    } else {
        Opened::Hut
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{screen_by_id, Gate};

    #[test]
    fn the_guide_opens_instead_of_the_hut_until_the_advancement_is_done() {
        // Les trois états de l'avancement, et les deux qui renvoient au guide.
        assert_eq!(open(true, Advancement::Missing), Opened::Guide);
        assert_eq!(open(true, Advancement::Started), Opened::Guide);
        assert_eq!(open(true, Advancement::Done), Opened::Hut);
        // La transition est un `Effect::Opens`, pas un état interne.
        assert_eq!(Opened::Guide.effect(), Some(Effect::Opens(HUT_GUIDE_ID)));
        assert_eq!(Opened::Hut.effect(), None, "la fenêtre reste, rien ne s'ouvre");
        assert_eq!(Opened::Guide.java(), "WindowHutGuide");
        assert_eq!(Opened::Hut.java(), JAVA);
        // Et le point qui compte : le guide laisse les panneaux du worker vides.
        assert!(!Opened::Guide.writes_worker_fields());
        assert!(Opened::Hut.writes_worker_fields());
    }

    #[test]
    fn the_guide_reopens_the_hut_without_checking_the_advancement_again() {
        // WindowHutGuide.java:40 passe `needGuide = false` : plus de porte du tout.
        assert_eq!(open(false, Advancement::Missing), Opened::Hut);
        assert_eq!(open(false, Advancement::Started), Opened::Hut);
        assert_eq!(open(false, Advancement::Done), Opened::Hut);
        // Les deux appelants, et le drapeau que chacun pose (:35-50).
        assert_eq!(CALLERS.len(), 2);
        assert_eq!(CALLERS[0].1, true);
        assert_eq!(CALLERS[1].1, false);
        assert_eq!(CALLERS[1].0, "WindowHutGuide.closeGuide");
        // Le second constructeur existe pour ce seul appelant : un seul nom.
        for (caller, need_guide) in CALLERS {
            assert_eq!(
                open(*need_guide, Advancement::Missing).writes_worker_fields(),
                !*need_guide,
                "{caller}"
            );
        }
    }

    #[test]
    fn the_advancement_this_window_gates_on_is_the_one_src_ui_names() {
        // La jointure : `src/ui` porte déjà la gate de cet écran.
        let screen = screen_by_id(crate::ui::screen::HUT_BUILDER_ID)
            .unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(screen.window, JAVA);
        assert_eq!(screen.resource, RESOURCE, "le XML est celui du worker");
        assert_eq!(screen.gate, Some(Gate::Advancement(GUIDE_ADVANCEMENT)));
        // Et c'est la seule ligne de `src/ui` qui ait une gate.
        let gated = crate::ui::SCREENS
            .iter()
            .filter(|candidate| candidate.gate.is_some())
            .count();
        assert_eq!(gated, 1, "la porte du builder est la seule du lot");
        // Le guide que la porte ouvre est un id de la table, pas un nom de classe :
        // `Effect::Opens` et non `Effect::OpensJava`.
        assert!(screen_by_id(HUT_GUIDE_ID).is_ok());
        assert_eq!(RESOURCE, "minecolonies:gui/windowhutworkerplaceholder.xml");
        assert!(!GUIDE_JAVA.is_empty());
    }
}
