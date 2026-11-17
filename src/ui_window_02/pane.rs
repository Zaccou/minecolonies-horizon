//! Le vocabulaire d'une fenêtre BlockUI : un panneau, une liste déroulante, un bouton.
//!
//! **Même quadruple que `src/ui/field.rs`** — `(id, libellé, valeur, portée)` — et `Value` /
//! `Scope` sont **les siens** : [`Pane::from_ui`] convertit un [`crate::ui::Field`], ce qui
//! prouve que les deux formes s'emboîtent. Seule la [`Source`] est propre à ce module, parce
//! que ces quatre fenêtres lisent trois choses qu'aucun écran de hutte ne lit : le fichier
//! `.blueprint`, le catalogue de packs et le compound de l'item tenu en main.
//!
//! **Une liste déroulante n'est pas un panneau à valeur** : Java lui donne un fournisseur
//! (`getElementCount`, `getLabel`) et un index (`WindowBuildBuilding.java:391-410`).
//!
//! **Un clic est une suite, pas un message** : `build` envoie **deux** messages dans
//! l'ordre — le style puis la demande (`:221-229`) — d'où [`Press`].

use crate::ui::field::{Scope, Value};

use super::refusal::Refusal;

/// D'où vient la valeur d'un panneau de ces quatre fenêtres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Une source que `src/ui::field::Source` nomme déjà.
    Ui(crate::ui::Source),
    /// Le `.blueprint` : ses ressources (`WindowBuildBuilding.java:298-355`), sa taille, ses
    /// niveaux (`WindowBuildingBrowser.java:375`, `:368`).
    Blueprint,
    /// Le catalogue de packs (`WindowBuildingBrowser.java:247`, `:303`).
    Packs,
    /// Le compound de l'item tenu en main (`ItemClipboard.java:37`). La clipboard en lit
    /// son drapeau, qui n'est pas un panneau mais l'état de son bouton
    /// ([`super::clipboard::Filter`]).
    Item,
    /// Une constante de la fenêtre.
    Window,
    /// Le bloc de hutte (`AbstractBlockHut.getBlueprintDisplayName()`, `:92`).
    Block,
}

impl Source {
    /// La vue du bâtiment : niveau, pack, chemin, parent, déconstruit.
    pub const BUILDING: Source = Source::Ui(crate::ui::Source::Building);
    /// La colonie et son registre client de bâtiments (`WindowBuildBuilding.java:250-254`).
    pub const COLONY: Source = Source::Ui(crate::ui::Source::Colony);
    /// Une requête du gestionnaire (`WindowClipBoard.java:143`).
    pub const REQUEST: Source = Source::Ui(crate::ui::Source::Request);

    /// Un nom stable, pour un journal ou un test.
    pub fn name(self) -> &'static str {
        match self {
            Source::Ui(crate::ui::Source::Building) => "ui.building",
            Source::Ui(crate::ui::Source::Colony) => "ui.colony",
            Source::Ui(crate::ui::Source::Request) => "ui.request",
            Source::Ui(_) => "ui.other",
            Source::Blueprint => "blueprint",
            Source::Packs => "packs",
            Source::Item => "item",
            Source::Window => "window",
            Source::Block => "block",
        }
    }

    /// Faux pour ce qui vient d'un fichier ou d'un bloc : le port n'a alors aucune valeur
    /// vivante à lire, seulement un document.
    pub fn is_runtime(self) -> bool {
        !matches!(self, Source::Blueprint | Source::Packs | Source::Block)
    }
}

/// Un panneau : ce que Java remplit par son `id`, et rien d'autre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pane {
    /// L'`id` de l'XML, seule prise de la classe (`WindowBuildBuilding.java:290`).
    pub id: &'static str,
    /// La clé du libellé écrit à côté, quand il y en a un.
    pub label: Option<&'static str>,
    pub value: Value,
    pub source: Source,
    /// Le panneau de la fenêtre, ou d'une ligne d'une de ses listes.
    pub scope: Scope,
}

impl Pane {
    /// Un panneau de la fenêtre, sans libellé propre.
    pub const fn new(id: &'static str, value: Value, source: Source) -> Pane {
        Pane { id, label: None, value, source, scope: Scope::Window }
    }

    /// Un panneau **dans une ligne** de la liste `list` : BlockUI y cherche `name`, donc le
    /// même id peut vivre dans deux portées.
    pub const fn in_row(list: &'static str, id: &'static str, value: Value, source: Source) -> Pane {
        Pane { id, label: None, value, source, scope: Scope::Row(list) }
    }

    /// Le même panneau, décrit par `src/ui` : sa source y est forcément une des neuf.
    pub const fn from_ui(field: crate::ui::Field) -> Pane {
        Pane {
            id: field.id,
            label: field.label,
            value: field.value,
            source: Source::Ui(field.source),
            scope: field.scope,
        }
    }

    /// Vrai quand le panneau appartient à une ligne.
    pub fn is_row_pane(&self) -> bool {
        matches!(self.scope, Scope::Row(_))
    }
}

/// Une liste déroulante : un fournisseur de libellés et un index sélectionné.
///
/// Java ne stocke pas la liste dans le panneau : elle vit dans la fenêtre (`styles`,
/// `builders`) et le panneau la lit (`WindowBuildBuilding.java:393-410`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dropdown {
    pub id: &'static str,
    pub source: Source,
    entries: Vec<String>,
    selected: usize,
}

impl Dropdown {
    /// Un fournisseur de `entries` libellés et l'index sélectionné.
    ///
    /// Une liste vide n'a pas d'index, et un index hors bornes est ce que Java laisse
    /// passer jusqu'à `builders.get(index)` (`WindowBuildBuilding.java:187`, qui lève).
    pub fn new(
        id: &'static str,
        source: Source,
        entries: Vec<String>,
        selected: usize,
    ) -> Result<Dropdown, Refusal> {
        if entries.is_empty() {
            return Err(Refusal::EmptyProvider);
        }
        if selected >= entries.len() {
            return Err(Refusal::IndexOutOfRange);
        }
        Ok(Dropdown { id, source, entries, selected })
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Le libellé sélectionné (`getLabel`, `:402-409`).
    pub fn label(&self) -> Option<&str> {
        self.entries.get(self.selected).map(String::as_str)
    }

    /// L'index suivant, en bouclant. La règle de bord appartient à
    /// `DropDownList.selectNext()`, qui vit dans BlockUI et **n'est pas dans notre arbre**
    /// (`MANQUANT.md` §2) ; le seul cas atteignable est une liste d'un élément.
    pub fn next(&mut self) -> usize {
        self.selected = (self.selected + 1) % self.count();
        self.selected
    }

    /// L'index précédent, en bouclant — même réserve que [`Dropdown::next`].
    pub fn previous(&mut self) -> usize {
        self.selected = (self.selected + self.count() - 1) % self.count();
        self.selected
    }
}

/// Un message de `core/network/messages/`, **nommé et non envoyé** (même constat que
/// `src/ui/action.rs`, `Effect::Sends`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message {
    /// Le nom simple de la classe Java — ou, pour la décoration, celui de son paramètre.
    pub class: &'static str,
    /// La charge utile quand elle décide du sens : le mode de `BuildRequestMessage`
    /// (`BuildRequestMessage.java:23-28`), l'état de `UpdateRequestStateMessage`.
    pub payload: Option<&'static str>,
}

impl Message {
    /// Un message dont seule la classe compte.
    pub const fn plain(class: &'static str) -> Message {
        Message { class, payload: None }
    }

    /// Un message dont la charge utile est un mot du source (`BUILD`, `CANCELLED`…).
    pub const fn with(class: &'static str, payload: &'static str) -> Message {
        Message { class, payload: Some(payload) }
    }
}

/// Où un clic laisse la fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// Elle reste ouverte (`WindowClipBoard.java:67-78`).
    Stays,
    /// Elle se ferme (`WindowBuildBuilding.java:179`).
    Closes,
    /// Celle qui l'a ouverte se rouvre : `building.openGui(false)` (`:197`) — le mot de
    /// `ui::action::Effect::Reopens`.
    Reopens,
    /// Une fenêtre Java que cette table ne décrit pas (`WindowConfirm`, `:209`).
    OpensJava(&'static str),
}

/// Ce qu'un clic envoie, dans l'ordre, puis où il laisse la fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Press {
    pub sends: &'static [Message],
    pub exit: Exit,
}

impl Press {
    /// Aucun message, et la fenêtre reste ouverte.
    pub const STAYS: Press = Press { sends: &[], exit: Exit::Stays };

    pub const fn new(sends: &'static [Message], exit: Exit) -> Press {
        Press { sends, exit }
    }

    /// Le message de cette classe, s'il y en a un.
    pub fn message(&self, class: &str) -> Option<&'static Message> {
        self.sends.iter().find(|message| message.class == class)
    }
}

/// Ce que porte le libellé d'un bouton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    /// Le libellé de l'XML, que la classe ne réécrit pas (`windowbuildbuilding.xml:47-58`).
    Xml(&'static str),
    /// Un libellé que la classe écrit après coup (`setText`, `WindowBuildBuilding.java:142`,
    /// `:153`) : le XML en montre un autre entre-temps.
    Written(&'static str),
}

impl Label {
    /// La clé affichée, `""` quand le bouton n'est qu'une image.
    pub fn key(self) -> &'static str {
        match self {
            Label::Xml(key) | Label::Written(key) => key,
        }
    }

    /// Vrai quand Java remplace le libellé du XML.
    pub fn is_written(self) -> bool {
        matches!(self, Label::Written(_))
    }
}

/// Un bouton de la fenêtre, ou d'une ligne de sa liste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    /// L'`id` que `registerButton` lie (`AbstractWindowSkeleton.java:130`).
    pub id: &'static str,
    pub label: Label,
    pub press: Press,
    /// Chaque façon dont Java le refuse, dans l'ordre où il teste.
    pub refusals: &'static [Refusal],
}

impl Action {
    pub const fn new(
        id: &'static str,
        label: Label,
        press: Press,
        refusals: &'static [Refusal],
    ) -> Action {
        Action { id, label, press, refusals }
    }

    /// Vrai quand Java cache le bouton dans au moins un état.
    pub fn is_guarded(&self) -> bool {
        !self.refusals.is_empty()
    }

    /// Les messages du clic, dans l'ordre.
    pub fn sends(&self) -> &'static [Message] {
        self.press.sends
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_from_src_ui_keeps_its_four_parts_and_wraps_the_source() {
        let title = crate::ui::Field::new("name", Value::Name, crate::ui::Source::Building);
        let pane = Pane::from_ui(title);
        assert_eq!(pane.id, "name");
        assert_eq!(pane.value, title.value, "les deux formes portent la même valeur");
        assert_eq!(pane.label, None);
        assert_eq!(pane.scope, Scope::Window);
        assert_eq!(pane.source, Source::BUILDING);
        assert_eq!(pane.source.name(), "ui.building");
        assert!(pane.source.is_runtime(), "un bâtiment vit dans la colonie, pas dans un fichier");
        assert!(!Source::Blueprint.is_runtime());
        assert!(!pane.is_row_pane());
        // Chaque source a son nom, et aucun ne double : c'est ce qu'un journal lira.
        let all = [
            Source::BUILDING,
            Source::COLONY,
            Source::REQUEST,
            Source::Blueprint,
            Source::Packs,
            Source::Item,
            Source::Window,
            Source::Block,
        ];
        for (index, source) in all.iter().enumerate() {
            assert!(!source.name().is_empty(), "une source sans nom");
            for other in &all[index + 1..] {
                assert_ne!(source.name(), other.name(), "deux sources, un nom");
            }
        }
    }

    #[test]
    fn a_dropdown_refuses_an_empty_provider_and_an_index_out_of_range() {
        let one = match Dropdown::new("style", Source::BUILDING, vec!["caledonia".to_owned()], 0) {
            Ok(dropdown) => dropdown,
            Err(refusal) => panic!("un style valide est refusé : {refusal}"),
        };
        assert_eq!(one.count(), 1);
        assert_eq!(one.selected(), 0);
        assert_eq!(one.label(), Some("caledonia"));
        assert_eq!(
            Dropdown::new("style", Source::BUILDING, Vec::new(), 0),
            Err(Refusal::EmptyProvider)
        );
        assert_eq!(
            Dropdown::new("style", Source::BUILDING, vec!["a".to_owned()], 1),
            Err(Refusal::IndexOutOfRange),
            "WindowBuildBuilding.java:187 lèverait ici"
        );
    }

    #[test]
    fn a_press_keeps_its_messages_in_order() {
        const SENT: &[Message] = &[
            Message::plain("BuildingSetStyleMessage"),
            Message::with("BuildRequestMessage", "BUILD"),
        ];
        let press = Press::new(SENT, Exit::Reopens);
        assert_eq!(press.sends.len(), 2);
        assert_eq!(press.sends[0].class, "BuildingSetStyleMessage");
        assert_eq!(press.sends[0].payload, None);
        assert_eq!(
            press.message("BuildRequestMessage").and_then(|message| message.payload),
            Some("BUILD")
        );
        assert_eq!(press.message("BuildPickUpMessage"), None);
        assert_eq!(Press::STAYS.sends.len(), 0);
        assert_eq!(Press::STAYS.exit, Exit::Stays);
        assert_eq!(Label::Written("a").key(), Label::Xml("a").key());
        assert!(Label::Written("a").is_written());
        assert!(!Label::Xml("a").is_written());
    }
}
