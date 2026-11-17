//! `WindowTownHallNameEntry` : le champ de renommage de la **colonie** — le pré-remplissage
//! en minuscules, la frappe à vingt-cinq, et la coupe de vingt-quatre du message.
//!
//! Java référence : `core/client/gui/townhall/WindowTownHallNameEntry.java` (63 lignes),
//! `gui/townhall/windowtownhallnameentry.xml` (9), `ColonyView.setName`
//! (`ColonyView.java:995-998`) et `TownHallRenameMessage.java:17-39`.
//!
//! # Le pré-remplissage, et ce qu'il fait au nom
//!
//! `onOpened` (`:39`) fait trois choses en une ligne :
//! `Component.translatable(colony.getName().toLowerCase(Locale.US)).getString()`.
//! 1. il lit le nom de la **colonie** (`getName`) ;
//! 2. il le met **en minuscules** — `Halte` revient en `halte` ;
//! 3. il passe le résultat à `Component.translatable` : le nom est traité comme une **clé
//!    de langue**, et une clé inconnue s'affiche elle-même.
//!
//! C'est exactement la transformation de `WindowHutNameEntry.java:49`, et
//! `crate::ui_window_04::rename::prefill` l'implémente déjà : ce module **l'appelle** au
//! lieu de la réécrire ([`prefill`] est un `pub use`, pas une copie). Conséquence à
//! connaître : ouvrir ce champ puis cliquer `done` sans rien taper **renomme la colonie en
//! minuscules**.
//!
//! # Deux limites de vingt-cinq, et une coupe à vingt-quatre
//!
//! - le champ accepte **vingt-cinq** unités UTF-16 (`maxlength="25"`,
//!   `windowtownhallnameentry.xml:6`) ; au-delà, BlockUI avale la frappe et la classe ne
//!   voit rien ([`Edit::type_text`] le nomme [`Refusal::FieldFull`]) ;
//! - `TownHallRenameMessage` garde **vingt-cinq** et coupe à **vingt-quatre** au-delà
//!   (`TownHallRenameMessage.java:17-18`, `:38`) — c'est `colony_settings::MAX_COLONY_NAME`
//!   et son `capped`, que ce module appelle ;
//! - la classe, elle, ne vérifie **que le vide** (`:48`). Aucune longueur n'est testée ici,
//!   contrairement au renommage d'une hutte qui coupe à quinze avec un avertissement.
//!
//! La coupe est donc **inatteignable à la frappe** (25 ≤ 25) mais atteignable autrement :
//! `setText` (`:39`) n'est pas une frappe, et une colonie fondée sans coupe
//! (`CreateColonyMessage.java:53` écrit le nom en `writeUtf`) peut porter plus de
//! vingt-cinq caractères. Le champ l'affiche entier, et `done` le fait couper par le
//! message : [`Rename`] garde les deux longueurs, parce qu'elles diffèrent.
//!
//! # Ce que le clic laisse
//!
//! Ni `done` ni `cancel` ne ferment la fenêtre : la classe **rouvre l'hôtel de ville**
//! par-dessus quand elle en trouve un (`:58-61`, `openGui(false)`) et ne fait rien quand
//! il n'y en a pas. Tout autre id sort **avant** la réouverture (`:53-56`) — c'est le seul
//! chemin où la fenêtre reste telle quelle.

use crate::base::Dimension;
use crate::network::colony_settings::TownHallRename;
use crate::ui::field::Value;

use super::{Button, Header, Leaves, Opener, Pane, Refusal, Window, COLONY_NAME_ID};

/// La classe (`WindowTownHallNameEntry.java:21`).
pub const JAVA: &str = "WindowTownHallNameEntry";
/// Le XML (`:32`).
pub const RESOURCE: &str = "minecolonies:gui/townhall/windowtownhallnameentry.xml";
/// Le champ de saisie (`windowtownhallnameentry.xml:6`, `WindowConstants.java:927`).
pub const INPUT: &str = "name";
/// Le bouton qui applique (`:7`, testé `:45`).
pub const DONE: &str = "done";
/// Le bouton qui renonce (`:8`, testé `:53`).
pub const CANCEL: &str = "cancel";

/// Le titre de la fenêtre : un panneau **sans id**, donc pas un [`Pane`] — il vit dans
/// `Window::header` (`windowtownhallnameentry.xml:4-5`).
pub const TITLE_KEY: &str = "com.minecolonies.coremod.gui.townhall.rename.title";
/// Le libellé de `done` (`:7`), vanilla.
pub const DONE_KEY: &str = "gui.done";
/// Le libellé de `cancel` (`:8`), vanilla.
pub const CANCEL_KEY: &str = "gui.cancel";
/// L'écran qui l'ouvre : la page principale de l'hôtel de ville
/// (`WindowMainPage.java:383-386`).
pub const OPENER_CLASS: &str = "WindowMainPage";

/// Ce que le champ accepte : `maxlength="25"` (`windowtownhallnameentry.xml:6`).
///
/// **Le même nombre que la coupe du message**, et ce n'est pas un hasard : les deux valent
/// `colony_settings::MAX_COLONY_NAME` (`TownHallRenameMessage.java:17`). Une seule source,
/// pour que les deux ne puissent pas diverger.
pub const MAX_FIELD: usize = crate::network::colony_settings::MAX_COLONY_NAME;

/// Le pré-remplissage d'une hutte, **réutilisé** : même transformation, autre source.
pub use crate::ui_window_04::rename::prefill;

const XML: &str = "minecolonies/gui/townhall/windowtownhallnameentry.xml";
const JAVA_FILE: &str = "WindowTownHallNameEntry.java";

/// Le champ, tel qu'il est ouvert puis tapé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    /// Ce que le champ contient. Peut dépasser [`MAX_FIELD`] par `setText` (voir le
    /// pré-remplissage) ; la frappe, elle, ne le peut pas.
    pub typed: String,
}

impl Edit {
    /// Le champ à l'ouverture (`:39`) : le nom de la colonie, en minuscules, puis traduit
    /// s'il se trouve que c'est une clé de langue.
    pub fn opened(colony_name: &str, translate: impl Fn(&str) -> Option<String>) -> Edit {
        Edit { typed: prefill(colony_name, translate) }
    }

    /// Un champ déjà rempli, pour un appelant qui n'a pas de couche de langue.
    pub fn with(typed: &str) -> Edit {
        Edit { typed: typed.to_owned() }
    }

    /// Ajoute de la frappe, en s'arrêtant à [`MAX_FIELD`] unités UTF-16
    /// (`windowtownhallnameentry.xml:6`).
    ///
    /// `Err(Refusal::FieldFull)` quand une partie au moins des caractères est avalée :
    /// BlockUI les perd en silence, le port dit lesquels n'ont pas passé. La coupe tombe
    /// sur une **unité** UTF-16, comme `maxlength`, ce qui peut séparer une paire de
    /// substitution — c'est ce que fait un champ Java, pas une décision du port.
    pub fn type_text(&mut self, more: &str) -> Result<(), Refusal> {
        let mut units: Vec<u16> = self.typed.encode_utf16().collect();
        let mut overflowed = false;
        for unit in more.encode_utf16() {
            if units.len() >= MAX_FIELD {
                overflowed = true;
                break;
            }
            units.push(unit);
        }
        self.typed = String::from_utf16_lossy(&units);
        if overflowed {
            Err(Refusal::FieldFull)
        } else {
            Ok(())
        }
    }

    /// Vrai quand le champ tient dans ce que la frappe autorise.
    pub fn fits(&self) -> bool {
        self.typed.encode_utf16().count() <= MAX_FIELD
    }

    /// Vrai quand le champ est vide au sens de Java : `isEmpty()` (`:48`), qui ne
    /// regarde **que** la longueur — `" "` n'est donc pas vide, et devient un nom.
    pub fn is_empty(&self) -> bool {
        self.typed.is_empty()
    }

    /// Longueur mesurée comme Java : des unités UTF-16 (`String.length()`).
    pub fn length(&self) -> usize {
        self.typed.encode_utf16().count()
    }
}

/// Ce que la fenêtre garde en local et ce que le message emporte.
///
/// `colony.setName(name)` écrit le nom **entier** dans la vue (`ColonyView.java:995-997`)
/// puis construit le message, qui coupe (`TownHallRenameMessage.java:38`). Les deux
/// longueurs divergent donc dès la vingt-sixième unité.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rename {
    /// Le nom gardé par la vue de colonie, sans coupe.
    pub local_name: String,
    /// Le message envoyé, dont le nom est coupé à vingt-quatre au-delà de vingt-cinq.
    pub message: TownHallRename,
}

/// Ce qu'un clic laisse à l'appelant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    /// `done` avec un nom : la vue prend le nom, le message part (`:47-52`).
    Rename(Rename),
    /// `done` avec un champ vide : **rien** n'est envoyé (`:48`), et l'hôtel de ville se
    /// rouvre quand même (`:58-61`).
    Empty,
    /// `cancel` : rien n'est envoyé, l'hôtel de ville se rouvre (`:53`, `:58-61`).
    Cancel,
    /// Un autre id : Java sort **avant** la réouverture (`:53-56`).
    Ignored,
}

impl Out {
    /// Les types de message construits : un seul cas en envoie un.
    pub fn sends(&self) -> &'static [&'static str] {
        match self {
            Out::Rename(_) => &["TownHallRename"],
            Out::Empty | Out::Cancel | Out::Ignored => &[],
        }
    }

    /// Le refus nommé que ce clic porte, quand il en porte un.
    ///
    /// [`Refusal::EmptyName`] n'est pas une erreur d'appel : Java continue, rouvre l'hôtel
    /// de ville, et n'écrit rien. C'est pourquoi le clic rend [`Out::Empty`] **et** le
    /// refus, au lieu de rendre un `Err` qui ferait croire que rien ne se passe.
    pub fn refusal(&self) -> Option<Refusal> {
        match self {
            Out::Empty => Some(Refusal::EmptyName),
            Out::Rename(_) | Out::Cancel | Out::Ignored => None,
        }
    }

    /// Où le clic laisse la fenêtre, pour un hôtel de ville présent ou non (`:58-61`).
    pub fn leaves(&self, town_hall_present: bool) -> Result<Leaves, Refusal> {
        match self {
            Out::Ignored => Ok(Leaves::Stays),
            Out::Rename(_) | Out::Empty | Out::Cancel => {
                if town_hall_present {
                    Ok(Leaves::Reopens)
                } else {
                    Err(Refusal::NoTownHall)
                }
            }
        }
    }
}

/// Le clic d'un bouton.
pub fn press(edit: &Edit, id: &str, colony_id: i32, dimension: &Dimension) -> Out {
    match id {
        DONE => {
            if edit.is_empty() {
                return Out::Empty;
            }
            Out::Rename(Rename {
                local_name: edit.typed.clone(),
                message: TownHallRename::new(dimension, colony_id, &edit.typed),
            })
        }
        CANCEL => Out::Cancel,
        _ => Out::Ignored,
    }
}

/// Le seul panneau, écrit par `onOpened` (`WindowTownHallNameEntry.java:39`).
const PANES: &[Pane] = &[Pane::written(
    INPUT,
    Value::Input,
    super::Source::ColonyView,
    (XML, 6),
    (JAVA_FILE, 39),
)];

/// Les deux boutons, dans l'ordre de l'XML.
const BUTTONS: &[Button] = &[
    Button::sending(DONE, DONE_KEY, &["TownHallRename"], Leaves::Reopens),
    Button::local(CANCEL, CANCEL_KEY, Leaves::Reopens),
];

/// L'écran.
pub static WINDOW: Window = Window {
    id: COLONY_NAME_ID,
    java: JAVA,
    resource: RESOURCE,
    header: Header::Xml(TITLE_KEY),
    opener: Opener::Java(OPENER_CLASS),
    panes: PANES,
    buttons: BUTTONS,
    skeleton: false,
};

/// Le message qu'un clic enverrait pour un champ donné, sans passer par la fenêtre.
pub fn rename_message(typed: &str, colony_id: i32, dimension: &Dimension) -> TownHallRename {
    TownHallRename::new(dimension, colony_id, typed)
}

/// Le nombre de caractères que le message garde pour un nom trop long
/// (`TownHallRenameMessage.java:18`, `SUBSTRING_LENGTH = MAX_NAME_LENGTH - 1`).
pub fn message_keeps(name: &str) -> usize {
    rename_message(name, 1, &Dimension::new("minecraft:overworld"))
        .name
        .encode_utf16()
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overworld() -> Dimension {
        Dimension::new("minecraft:overworld")
    }

    #[test]
    fn the_field_opens_on_the_lowercased_name_with_the_hut_s_prefill() {
        let no_lang = |_: &str| None;
        assert_eq!(Edit::opened("Halte", no_lang).typed, "halte");
        assert_eq!(Edit::opened("Ferme Élevage", no_lang).typed, "ferme élevage");
        // Une seule règle, un seul endroit : c'est la fonction du renommage de hutte.
        for name in ["Halte", "Ferme Élevage", "", "Simon's Colony"] {
            assert_eq!(
                Edit::opened(name, no_lang).typed,
                crate::ui_window_04::rename::prefill(name, no_lang),
                "{name}"
            );
        }
        // Et un nom qui est une clé de langue s'affiche traduit, comme en Java.
        let lang = |key: &str| (key == "halte").then(|| "Halte traduite".to_owned());
        assert_eq!(Edit::opened("Halte", lang).typed, "Halte traduite");
        // Le champ d'ici lit le nom de la **colonie**, pas un nom personnalisé de hutte.
        let pane = match WINDOW.pane(INPUT) {
            Some(pane) => pane,
            None => panic!("le champ a disparu"),
        };
        assert_eq!(pane.source, super::super::Source::ColonyView);
        assert_eq!(pane.value, Value::Input);
        assert_eq!(WINDOW.header, Header::Xml(TITLE_KEY));
    }

    #[test]
    fn the_frappe_stops_at_twenty_five_which_is_also_the_message_s_limit() {
        assert_eq!(MAX_FIELD, 25);
        assert_eq!(MAX_FIELD, crate::network::colony_settings::MAX_COLONY_NAME);
        // Ce n'est pas la règle du renommage de hutte : quinze.
        assert_ne!(MAX_FIELD, crate::ui::action::MAX_NAME_LENGTH);
        let mut edit = Edit::with("");
        assert_eq!(edit.type_text("abcdefghijklmnopqrstuvwxy"), Ok(()), "vingt-cinq pile");
        assert_eq!(edit.length(), 25);
        assert!(edit.fits());
        // Le vingt-sixième caractère est avalé, et le refus le dit.
        assert_eq!(edit.type_text("z"), Err(Refusal::FieldFull));
        assert_eq!(edit.typed, "abcdefghijklmnopqrstuvwxy");
        assert!(edit.fits());
        // Une frappe partiellement refusée garde ce qui est passé.
        let mut short = Edit::with("abcdefghijklmnopqrstuvwx");
        assert_eq!(short.type_text("yz"), Err(Refusal::FieldFull));
        assert_eq!(short.length(), 25);
        // Et un champ déjà trop long par `setText` ne redevient pas valide en tapant.
        let mut long = Edit::with(&"a".repeat(40));
        assert!(!long.fits());
        assert_eq!(long.type_text("b"), Err(Refusal::FieldFull));
    }

    #[test]
    fn done_keeps_the_name_whole_and_lets_the_message_cut_it() {
        let dimension = overworld();
        let edit = Edit::with("Halte");
        match press(&edit, DONE, 3, &dimension) {
            Out::Rename(rename) => {
                assert_eq!(rename.local_name, "Halte", "la vue garde le nom entier");
                assert_eq!(rename.message.name, "Halte");
                assert_eq!(rename.message.colony_id, 3);
                assert_eq!(rename.message.dimension, dimension);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(press(&edit, DONE, 3, &dimension).sends(), &["TownHallRename"][..]);
        // Vingt-cinq : gardé tel quel. Vingt-six : le message coupe à vingt-quatre.
        assert_eq!(message_keeps(&"a".repeat(25)), 25);
        assert_eq!(message_keeps(&"a".repeat(26)), 24);
        // Le cas atteignable : un nom pré-rempli plus long que le champ.
        let long = Edit::with(&"a".repeat(40));
        match press(&long, DONE, 3, &dimension) {
            Out::Rename(rename) => {
                assert_eq!(rename.local_name.len(), 40, "setText ne coupe pas");
                assert_eq!(rename.message.name.len(), 24, "TownHallRenameMessage.java:38");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_empty_field_renames_nothing_and_every_click_still_leaves() {
        let dimension = overworld();
        let empty = Edit::with("");
        assert_eq!(press(&empty, DONE, 3, &dimension), Out::Empty);
        assert_eq!(press(&empty, DONE, 3, &dimension).sends(), &[] as &[&str]);
        assert_eq!(press(&empty, DONE, 3, &dimension).refusal(), Some(Refusal::EmptyName));
        // Un blanc est un nom : Java ne teste que `isEmpty` (:48).
        assert_eq!(
            press(&Edit::with(" "), DONE, 3, &dimension).sends(),
            &["TownHallRename"][..],
            "« » n'est pas vide pour Java"
        );
        // `cancel` n'envoie rien et rouvre l'hôtel de ville, comme `done`.
        assert_eq!(press(&empty, CANCEL, 3, &dimension), Out::Cancel);
        assert_eq!(press(&empty, CANCEL, 3, &dimension).refusal(), None);
        assert_eq!(press(&empty, CANCEL, 3, &dimension).leaves(true), Ok(Leaves::Reopens));
        assert_eq!(press(&empty, DONE, 3, &dimension).leaves(true), Ok(Leaves::Reopens));
        // Sans hôtel de ville à rouvrir, rien ne se rouvre — et le refus le nomme.
        assert_eq!(press(&empty, CANCEL, 3, &dimension).leaves(false), Err(Refusal::NoTownHall));
        // Tout autre id sort avant la réouverture (:53-56) : la fenêtre ne bouge pas.
        assert_eq!(press(&empty, "rename", 3, &dimension), Out::Ignored);
        assert_eq!(press(&empty, "rename", 3, &dimension).leaves(true), Ok(Leaves::Stays));
        // La table de la fenêtre dit la même chose que le comportement.
        assert_eq!(WINDOW.button(DONE).map(|b| b.leaves), Some(Leaves::Reopens));
        assert_eq!(WINDOW.button(CANCEL).map(|b| b.is_local()), Some(true));
    }
}
