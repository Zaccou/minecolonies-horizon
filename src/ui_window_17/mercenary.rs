//! `WindowTownHallMercenary` : la vague de mercenaires — combien, combien de portraits,
//! et les quarante minutes qui séparent deux vagues.
//!
//! Java référence : `core/client/gui/townhall/WindowTownHallMercenary.java` (73 lignes) et
//! `gui/townhall/windowtownhallmercenary.xml` (16), plus les deux bouts serveur :
//! `EntityMercenary.spawnMercenariesInColony` (`EntityMercenary.java:485-520`) et
//! `HireMercenaryMessage.java:43`.
//!
//! **La fenêtre ne calcule rien et n'affiche qu'un texte.** Le nombre de mercenaires est
//! lu dans la vue de colonie (`:41-43`), neuf portraits au plus sont ajoutés comme enfants
//! `Image` (`:48-58`), et le seul panneau de l'XML est un récit figé
//! (`windowtownhallmercenary.xml:15`). **Les portraits n'ont pas d'id** : ce ne sont pas
//! des panneaux, et leur image n'est pas reproductible — `ColonyConstants.rand` est un
//! `Random` **non graine** (`ColonyConstants.java:25`), donc `CitizenData.SUFFIXES`
//! (`CitizenData.java:98`) donne quatre têtes différentes à chaque ouverture.
//!
//! **Le compte ne s'arrête pas où le dessin s'arrête.** `amountOfMercenaries` vaut
//! `citoyens / 10 + 3` sans borne, et le serveur en fait apparaître autant
//! (`EntityMercenary.java:496-500`) ; la fenêtre, elle, s'arrête à neuf portraits (`:48`).
//! Une colonie de deux cents habitants paie donc vingt-trois mercenaires et en dessine
//! neuf : [`Offer`] garde les deux nombres, parce que les confondre serait faux dans les
//! deux sens.
//!
//! **Un clic muet, et quarante minutes d'attente.** Le clic sur `done`
//! envoie `HireMercenaryMessage` et joue un coup de tonnerre **au client**, puis marque
//! l'heure locale (`:66-68`) ; le serveur, lui, refuse **sans rien dire** si la vague
//! précédente date de moins de `TICKS_FOURTY_MIN` (`EntityMercenary.java:489-492`), et
//! c'est l'hôtel de ville qui désactive son bouton d'entrée pour éviter d'ouvrir cet écran
//! (`WindowMainPage.java:373-377`). [`ready`] est ce refus, nommé.
//!
//! **Aucun bouton ne refuse.** `this.close()` est **hors** du `if` (`:71`) : `done` envoie
//! et ferme, `cancel` ferme sans rien envoyer, et n'importe quel autre id ferme aussi.

use crate::base::Dimension;
use crate::network::colony_settings::HireMercenary;
use crate::ui::field::Value;

use super::{Button, Header, Leaves, Opener, Pane, Refusal, Window, MERCENARY_ID, TICKS_FORTY_MIN};

/// La classe (`WindowTownHallMercenary.java:24`).
pub const JAVA: &str = "WindowTownHallMercenary";
/// Le XML (`:38`).
pub const RESOURCE: &str = "minecolonies:gui/townhall/windowtownhallmercenary.xml";
/// Le seul panneau : un récit figé dans l'XML, qu'aucune ligne de la classe n'écrit
/// (`windowtownhallmercenary.xml:15`).
pub const TEXT: &str = "text";
/// Le bouton qui paie (`:12`, testé `:64`).
pub const DONE: &str = "done";
/// Le bouton qui ferme sans payer (`:13`) — la classe ne le nomme jamais.
pub const CANCEL: &str = "cancel";

/// Le récit de l'XML (`windowtownhallmercenary.xml:15`).
pub const STORY_KEY: &str = "com.minecolonies.coremod.gui.townhall.mercenarystory";
/// Le libellé de `done` (`:12`).
pub const HIRE_KEY: &str = "com.minecolonies.coremod.gui.townhall.mercenaryhire";
/// Le libellé de `cancel` (`:13`) : la clé vanilla de la Croix.
pub const CANCEL_KEY: &str = "gui.cancel";
/// L'écran qui l'ouvre : le bouton `mercenaries` de la page principale de l'hôtel de ville
/// (`WindowMainPage.java:390-394`, `WindowConstants.java:87`).
pub const OPENER_CLASS: &str = "WindowMainPage";

/// Le nombre de portraits que la fenêtre dessine, quel que soit le nombre de mercenaires
/// (`WindowTownHallMercenary.java:48`).
pub const MAX_PORTRAITS: i32 = 9;

/// Les quatre têtes possibles (`CitizenData.java:98`) — quatre valeurs, pas un tirage
/// reproductible : la graine est `ColonyConstants.java:25`.
pub const PORTRAIT_SUFFIXES: [&str; 4] = ["_a", "_b", "_d", "_w"];

/// Le son que le client joue au clic, par son nom (`:68`) : le port n'a pas de son.
pub const HIRE_SOUND: &str = "LIGHTNING_BOLT_THUNDER";

const XML: &str = "minecolonies/gui/townhall/windowtownhallmercenary.xml";

/// Combien de mercenaires une colonie de `citizens` habitants achète (`:41-43`, relu côté
/// serveur à `EntityMercenary.java:496-498`) : `citoyens / 10 + 3`, division entière.
pub fn mercenaries(citizens: i32) -> i32 {
    citizens / 10 + 3
}

/// Combien de portraits la fenêtre dessine pour `mercenaries` mercenaires (`:48`).
///
/// `Math.min(amount, 9)`, avec un plancher à zéro : Java boucle alors zéro fois, ce qui est
/// la même chose vue du dessin, et le port ne rend pas un compte négatif.
pub fn portraits(mercenaries: i32) -> i32 {
    mercenaries.min(MAX_PORTRAITS).max(0)
}

/// Ce qu'une vague coûte en hommes, et ce qu'elle montre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offer {
    /// Ce que le serveur fera apparaître, plus le chef (`EntityMercenary.java:513-518`).
    pub mercenaries: i32,
    /// Ce que la fenêtre dessine (`:48`) : au plus [`MAX_PORTRAITS`].
    pub portraits: i32,
}

impl Offer {
    /// L'offre pour une colonie de `citizens` habitants.
    pub fn of(citizens: i32) -> Offer {
        let mercenaries = mercenaries(citizens);
        Offer { mercenaries, portraits: portraits(mercenaries) }
    }

    /// Vrai quand la fenêtre dessine moins de portraits qu'il n'y aura de mercenaires —
    /// c'est le cas de toute colonie de vingt habitants ou plus.
    pub fn portraits_are_short(&self) -> bool {
        self.portraits < self.mercenaries
    }
}

/// La vague est-elle disponible ? Sinon, combien de ticks restent.
///
/// `EntityMercenary.java:489-492` : `useTime != 0 && now - useTime < TICKS_FOURTY_MIN`
/// refuse. Un `use_time` de zéro veut dire « jamais utilisé » et n'a pas de date — c'est le
/// cas d'une colonie neuve, et Java le traite à part exprès.
pub fn ready(use_time: i64, now: i64) -> Result<(), Refusal> {
    if use_time == 0 {
        return Ok(());
    }
    let elapsed = now - use_time;
    if elapsed >= TICKS_FORTY_MIN {
        return Ok(());
    }
    Err(Refusal::MercenaryCooldown { remaining: TICKS_FORTY_MIN - elapsed })
}

/// Ce qu'un clic laisse à l'appelant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    /// `done` : la vague est payée.
    Hire {
        /// `HireMercenaryMessage` ne porte que l'enveloppe de la colonie (`:43`).
        message: HireMercenary,
        /// La fenêtre marque l'heure **au client** avant d'envoyer (`:66`) : le serveur
        /// garde la vraie, et c'est la sienne qui décide du refus suivant.
        marks_used: bool,
    },
    /// La fenêtre se ferme sans rien envoyer.
    Close,
}

impl Out {
    /// Le son que le client joue pour ce clic, s'il y en a un (`:68`).
    pub fn sound(&self) -> Option<&'static str> {
        match self {
            Out::Hire { .. } => Some(HIRE_SOUND),
            Out::Close => None,
        }
    }
}

/// Le clic d'un bouton.
///
/// Pas de `Result` : **aucun** bouton de cet écran ne peut être refusé côté fenêtre — le
/// `close()` de `:71` est hors du `if`, donc tout id ferme, et c'est le serveur qui décide
/// ensuite. Un test le constate plutôt que de le supposer.
pub fn press(id: &str, colony_id: i32, dimension: &Dimension) -> Out {
    if id == DONE {
        return Out::Hire {
            message: HireMercenary { colony_id, dimension: dimension.clone() },
            marks_used: true,
        };
    }
    Out::Close
}

/// Le seul panneau : un texte que personne n'écrit.
const PANES: &[Pane] = &[Pane::fixed(TEXT, Value::Text, (XML, 15))];

/// Les deux boutons, dans l'ordre de l'XML.
const BUTTONS: &[Button] = &[
    Button::sending(DONE, HIRE_KEY, &["HireMercenary"], Leaves::Closes),
    Button::local(CANCEL, CANCEL_KEY, Leaves::Closes),
];

/// L'écran.
pub static WINDOW: Window = Window {
    id: MERCENARY_ID,
    java: JAVA,
    resource: RESOURCE,
    header: Header::None,
    opener: Opener::Java(OPENER_CLASS),
    panes: PANES,
    buttons: BUTTONS,
    skeleton: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn overworld() -> Dimension {
        Dimension::new("minecraft:overworld")
    }

    #[test]
    fn the_wave_is_citizens_div_ten_plus_three_and_the_drawing_stops_at_nine() {
        // La division est entière : dix habitants valent un mercenaire de plus.
        assert_eq!(mercenaries(0), 3);
        assert_eq!(mercenaries(9), 3);
        assert_eq!(mercenaries(10), 4);
        assert_eq!(mercenaries(61), 9);
        assert_eq!(mercenaries(200), 23);
        // Le dessin, lui, s'arrête à neuf (`:48`).
        assert_eq!(portraits(3), 3);
        assert_eq!(portraits(9), 9);
        assert_eq!(portraits(23), MAX_PORTRAITS);
        // Et une valeur impossible ne rend pas un compte négatif.
        assert_eq!(portraits(-4), 0);
        // Les deux nombres ensemble : au-delà de soixante habitants, le dessin ment.
        assert!(!Offer::of(61).portraits_are_short(), "neuf pour neuf");
        let big = Offer::of(200);
        assert_eq!((big.mercenaries, big.portraits), (23, 9));
        assert!(big.portraits_are_short(), "vingt-trois payés, neuf dessinés");
        // Quatre têtes possibles, et aucune graine : rien à reproduire.
        assert_eq!(PORTRAIT_SUFFIXES.len(), 4);
    }

    #[test]
    fn the_cooldown_is_forty_minutes_and_zero_means_never_used() {
        assert_eq!(ready(0, 999_999), Ok(()), "useTime == 0 : jamais utilisé");
        assert_eq!(TICKS_FORTY_MIN, 48_000);
        // Une seconde avant la fin : refusé, et on sait combien il reste.
        assert_eq!(ready(1_000, 1_000 + 47_999), Err(Refusal::MercenaryCooldown { remaining: 1 }));
        // Pile à la fin : accepté — Java teste `<`, pas `<=`.
        assert_eq!(ready(1_000, 1_000 + 48_000), Ok(()));
        // Une horloge qui recule (un monde rechargé) refuse encore plus longtemps.
        assert_eq!(
            ready(5_000, 1_000),
            Err(Refusal::MercenaryCooldown { remaining: 48_000 + 4_000 })
        );
    }

    #[test]
    fn done_hires_and_every_other_button_closes() {
        let dimension = overworld();
        match press(DONE, 7, &dimension) {
            Out::Hire { message, marks_used } => {
                assert_eq!(message.colony_id, 7);
                assert_eq!(message.dimension, dimension);
                assert!(marks_used, ":66 marque l'heure au client avant d'envoyer");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(press(DONE, 7, &dimension).sound(), Some(HIRE_SOUND));
        // `cancel` ferme sans envoyer, et il ne refuse rien.
        assert_eq!(press(CANCEL, 7, &dimension), Out::Close);
        assert_eq!(press(CANCEL, 7, &dimension).sound(), None);
        // Et **tout** autre id ferme aussi : le `close()` est hors du `if` (`:71`).
        assert_eq!(press("mercenaries", 7, &dimension), Out::Close);
        assert_eq!(press("", 7, &dimension), Out::Close);
        // La table de la fenêtre dit la même chose que le comportement.
        let done = match WINDOW.button(DONE) {
            Some(button) => button,
            None => panic!("le bouton done a disparu"),
        };
        assert_eq!(done.sends, &["HireMercenary"][..]);
        assert_eq!(done.leaves, Leaves::Closes);
        assert!(super::super::message_is_known(done.sends[0]));
        // Le seul panneau est celui que l'XML fige : la classe ne l'écrit jamais.
        let text = match WINDOW.pane(TEXT) {
            Some(pane) => pane,
            None => panic!("le récit a disparu"),
        };
        assert_eq!(text.written, None);
        assert_eq!(text.source, super::super::Source::Key);
    }
}
