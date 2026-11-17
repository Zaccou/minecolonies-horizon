//! Le vocabulaire de ces quatre fenetres : un panneau, un bouton, un clic.
//!
//! **Meme quadruple que `src/ui/field.rs`** — `(id, valeur, provenance, portee)` — et
//! `Value` / `Scope` sont **les siens**, importes et non redits (`crate::ui::field`).
//! Seule la [`Source`] est propre a ce module, et c'est mesure : **aucun** des douze
//! panneaux de ce lot ne lit une des neuf sources de `src/ui` (`ui/field.rs`, `Source`).
//! Pas un ne vient d'un batiment, d'une colonie, d'un module, d'un citoyen, d'une requete,
//! d'un avancement, d'un ordre de travail ni de l'inventaire du joueur : ces quatre
//! fenetres sont des **outils**, elles ne parlent d'aucune hutte.
//!
//! **Le pack de structures, la carte du monde et l'item tenu en main ne sont pas des
//! sources ici**, et c'est mesure aussi : aucune des quatre ne les ecrit dans un panneau.
//! Ce sont des **regles de clic** — [`super::supplies::place`] lit le pack et le monde,
//! [`super::suggest::press`] lit l'inventaire. Un panneau qui les citerait mentirait.
//!
//! **Un clic n'est pas un message, c'est une suite.** Trois boutons de ce lot font deux
//! choses dans l'ordre : envoyer **puis** ouvrir ou fermer
//! (`WindowSupplyStory.java:113-114` envoie `MarkStoryReadOnItemMessage` puis ouvre
//! `WindowSupplies` ; `WindowSuggestBuildTool.java:80-81` envoie
//! `SwitchBuildingWithToolMessage` puis ouvre l'outil de construction ;
//! `WindowSupplies.java:150-157` envoie `BuildToolPlacementMessage` puis appelle
//! `cancelClicked()`). D'ou [`Press`] : des messages, puis une sortie.
//!
//! **Un bouton peut etre cable sans exister.** `registerButton` ne fait qu'un `put` dans
//! une `Map` (`AbstractWindowSkeleton.java:130-133`) ; c'est `onButtonClicked` qui ne
//! repond qu'aux panneaux reellement cliques (`:191-199`). Un id cable sans panneau ne
//! fait donc **jamais** rien, en silence : c'est le [`Button::declared`] a `None`.

use crate::ui::field::{Scope, Value};

/// D'ou vient la valeur d'un panneau de ces quatre fenetres.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: les quatre sources de ces fenetres, et aucune des neuf de `src/ui`
pub enum Source {
    /// Le cache d'analyses de la fenetre : `analyzationResults` et sa liste triee, deux
    /// `static` de la classe (`WindowSchematicAnalyzer.java:56`, `:61`).
    Cache,
    /// L'objet `Blueprint` lui-meme : son nom, son chemin, sa taille
    /// (`WindowSchematicAnalyzer.java:351-377`).
    Blueprint,
    /// Une phrase ou une visibilite que le **XML** pose et qu'aucune ligne de la classe
    /// n'ecrit (`windowsuggestbuildtool.xml:9`, `layoutmanipulation.xml:4`).
    Xml,
    /// Le corpus de recits, filtre par biome (`WindowSupplyStory.java:83`, `:87`,
    /// `ColonyStoryListener.java:88-98`).
    Story,
}

impl Source {
    /// Les quatre sources, dans l'ordre de ce fichier.
    pub const ALL: [Source; 4] = [Source::Cache, Source::Blueprint, Source::Xml, Source::Story];

    /// Un nom stable, pour un journal ou un test.
    pub fn name(self) -> &'static str {
        match self {
            Source::Cache => "analyzer.cache",
            Source::Blueprint => "blueprint",
            Source::Xml => "xml",
            Source::Story => "story",
        }
    }
}

/// Un panneau : ce que Java remplit par son `id`, et rien d'autre.
///
/// L'`id` est la seule prise qu'une classe a sur un panneau : `findPaneOfTypeByID("score",
/// Text.class)` cherche la meme chaine (`WindowSchematicAnalyzer.java:360`). Le
/// `label="list"` de `analyzedisplay.xml:9` n'est **pas** porte : c'est un litteral, pas
/// une cle de langue, et un champ `label` n'aurait servi qu'a le faire passer pour une.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pane {
    /// L'`id` de l'XML.
    pub id: &'static str,
    /// Ce que le panneau tient.
    pub value: Value,
    /// D'ou la valeur vient.
    pub source: Source,
    /// Le panneau de la fenetre, ou d'une ligne d'une de ses listes.
    pub scope: Scope,
}

impl Pane {
    /// Un panneau de la fenetre.
    pub const fn new(id: &'static str, value: Value, source: Source) -> Pane {
        Pane { id, value, source, scope: Scope::Window }
    }

    /// Un panneau **dans** un sous-arbre : une ligne de la liste `list`, ou — pour
    /// l'analyseur — une des deux boites `left` / `right`, dont la resolution est la meme
    /// (`switchSelectionTo` cherche depuis la boite, `WindowSchematicAnalyzer.java:334`
    /// et `:342`). `Scope` de `src/ui` porte exactement cette idee de sous-arbre.
    pub const fn in_row(
        list: &'static str,
        id: &'static str,
        value: Value,
        source: Source,
    ) -> Pane {
        Pane { id, value, source, scope: Scope::Row(list) }
    }

    /// Vrai quand le panneau appartient a un sous-arbre.
    pub fn is_row_pane(&self) -> bool {
        matches!(self.scope, Scope::Row(_))
    }
}

/// Ce que porte le libelle d'un bouton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: quatre facons pour un bouton de n'avoir pas de cle, et pas cinq
pub enum Label {
    /// Une cle ecrite par le XML, que la classe ne reecrit pas
    /// (`analyzedisplay.xml:7`).
    Xml(&'static str),
    /// Une cle que la classe ecrit apres l'ouverture, le XML en montrant une autre
    /// entre-temps (`WindowSchematicAnalyzer.java:384`, `WindowSupplyStory.java:105`).
    Written(&'static str),
    /// Un texte brut, **sans cle** : les deux fleches (`analyzedisplay.xml:4-5`) et le
    /// mot `none` que Java ecrit en dur (`:343`).
    Literal(&'static str),
    /// Aucun libelle : le bouton n'est qu'une image (`analyzedisplay.xml:25`).
    Image,
}

impl Label {
    /// La cle de langue, quand il y en a une.
    ///
    /// `None` n'est pas « pas de libelle » : c'est « le libelle n'est pas une cle », ce qui
    /// interdit a un appelant de le passer a un traducteur.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Label::Xml(key) | Label::Written(key) => Some(key),
            Label::Literal(_) | Label::Image => None,
        }
    }

    /// Le texte affiche, quand ce n'est pas une cle.
    pub fn text(self) -> Option<&'static str> {
        match self {
            Label::Literal(text) => Some(text),
            _ => None,
        }
    }

    /// Vrai quand Java remplace le libelle du XML.
    pub fn is_written(self) -> bool {
        matches!(self, Label::Written(_))
    }
}

/// Un message de `core/network/messages/`, **nomme et non envoye** — meme constat que
/// `ui/action.rs`, `Effect::Sends` : ce port n'a ni client ni paquet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message {
    /// Le nom simple de la classe Java.
    pub class: &'static str,
    /// La charge utile quand elle decide du sens : la main qui tient l'item
    /// (`MarkStoryReadOnItemMessage`, `WindowSupplyStory.java:113`), le type de placement
    /// (`BuildToolPlacementMessage.HandlerType.Survival`, `WindowSupplies.java:131`).
    pub payload: Option<&'static str>,
}

impl Message {
    /// Un message dont seule la classe compte.
    pub const fn plain(class: &'static str) -> Message {
        Message { class, payload: None }
    }

    /// Un message dont la charge utile est un mot du source.
    pub const fn with(class: &'static str, payload: &'static str) -> Message {
        Message { class, payload: Some(payload) }
    }
}

/// Ou un clic laisse la fenetre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: quatre sorties, et un `close()` de fenetre d'outil est la premiere
pub enum Exit {
    /// Elle reste ouverte (`WindowSchematicAnalyzer.java:85-91`).
    Stays,
    /// Elle se ferme — `close()`, sans fenetre parente a rouvrir
    /// (`WindowSuggestBuildTool.java:70`, `WindowSupplies.java:125`).
    Closes,
    /// Une fenetre **de ce lot** s'ouvre, par son id ([`super::WINDOWS`]).
    OpensHere(&'static str),
    /// Une fenetre Java que ce lot ne decrit pas : l'outil de construction etendu
    /// (`WindowSuggestBuildTool.java:81`), le choix de pack
    /// (`WindowSupplies.java:90`, `WindowSupplyStory.java:122`).
    OpensJava(&'static str),
}

/// Ce qu'un clic envoie, dans l'ordre, puis ou il laisse la fenetre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Press {
    pub sends: &'static [Message],
    pub exit: Exit,
}

impl Press {
    /// Rien, et la fenetre reste ouverte.
    pub const STAYS: Press = Press { sends: &[], exit: Exit::Stays };
    /// Rien, et elle se ferme.
    pub const CLOSES: Press = Press { sends: &[], exit: Exit::Closes };

    pub const fn new(sends: &'static [Message], exit: Exit) -> Press {
        Press { sends, exit }
    }

    /// Le message de cette classe, s'il y en a un.
    pub fn message(&self, class: &str) -> Option<&'static Message> {
        self.sends.iter().find(|message| message.class == class)
    }

    /// Vrai quand le clic ne fait rien du tout.
    pub fn is_noop(&self) -> bool {
        self.sends.is_empty() && self.exit == Exit::Stays
    }
}

/// Un bouton de la fenetre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Button {
    /// L'`id` que `registerButton` lie (`AbstractWindowSkeleton.java:130`).
    pub id: &'static str,
    pub label: Label,
    pub press: Press,
    /// Chaque facon dont Java le refuse, dans l'ordre ou il teste.
    pub refusals: &'static [super::refusal::Refusal],
    /// Le fichier qui **declare le panneau** et sa ligne, 1-based.
    ///
    /// `None` n'est pas une donnee manquante : c'est un id cable qu'aucun XML de la
    /// fenetre ne porte, donc un clic **inatteignable** — le cas de `direct`
    /// (`WindowSuggestBuildTool.java:60` cable, `windowsuggestbuildtool.xml:1-14` ne le
    /// declare pas). C'est la distinction qui a produit le bouton mort du guide dans
    /// `src/ui_window_04`.
    pub declared: Option<(&'static str, u32)>,
}

impl Button {
    /// Vrai quand un panneau de la fenetre porte cet id : le clic est atteignable.
    pub fn is_reachable(&self) -> bool {
        self.declared.is_some()
    }

    /// Vrai quand Java cache ou eteint le bouton dans au moins un etat.
    pub fn is_guarded(&self) -> bool {
        !self.refusals.is_empty()
    }

    /// Les messages du clic, dans l'ordre.
    pub fn sends(&self) -> &'static [Message] {
        self.press.sends
    }
}

/// Une fenetre : sa classe, son XML, ses panneaux, ses boutons.
///
/// Meme forme que `crate::ui::screen::Screen`, a une difference pres et elle est mesuree :
/// **`title` est un `Option`**, parce qu'aucune de ces quatre fenetres n'a de panneau de
/// titre (`windowanalyze.xml:1-13`, `windowsuggestbuildtool.xml:1-14`,
/// `windowsupplies.xml:1-8`, `windowsupplystory.xml:1-18`). Un `Title::Fixed("")` aurait
/// fait passer une absence pour une cle vide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// Le nom de cette table. Il n'est **pas** de Java : Java n'a pas d'id de fenetre.
    pub id: &'static str,
    /// La classe Java qui la construit.
    pub java: &'static str,
    /// Le XML, sous `assets/minecolonies/`.
    pub resource: &'static str,
    /// La cle du titre, quand la fenetre en a un.
    pub title: Option<&'static str>,
    pub panes: &'static [Pane],
    pub buttons: &'static [Button],
}

impl Window {
    /// Le panneau de cet id, copie ; le **premier**, puisqu'un id peut revenir.
    pub fn pane(&self, id: &str) -> Option<Pane> {
        self.panes.iter().find(|pane| pane.id == id).copied()
    }

    /// Le bouton de cet id, copie.
    pub fn button(&self, id: &str) -> Option<Button> {
        self.buttons.iter().find(|button| button.id == id).copied()
    }

    /// Si la fenetre montre au moins un panneau de ce genre.
    pub fn shows(&self, value: Value) -> bool {
        self.panes.iter().any(|pane| pane.value == value)
    }

    /// Les genres de valeur montres, sans repetition, dans l'ordre des panneaux.
    pub fn values(&self) -> Vec<Value> {
        let mut seen = Vec::new();
        for pane in self.panes {
            if !seen.contains(&pane.value) {
                seen.push(pane.value);
            }
        }
        seen
    }

    /// Les boutons reellement atteignables — les autres sont des cablages morts.
    pub fn reachable(&self) -> Vec<&'static str> {
        self.buttons
            .iter()
            .filter(|button| button.is_reachable())
            .map(|button| button.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_keeps_its_id_its_value_and_its_source() {
        let score = Pane::new("score", Value::Count, Source::Cache);
        assert_eq!(score.id, "score");
        assert_eq!(score.value, Value::Count);
        assert_eq!(score.source, Source::Cache);
        assert_eq!(score.scope, Scope::Window, "un panneau de la fenetre");
        assert!(!score.is_row_pane());
        // Le meme id dans un sous-arbre est une autre portee : c'est ce qui permet a
        // `left` et `right` de porter les neuf memes panneaux.
        let in_box =
            Pane::in_row(crate::ui_window_07::analyzer::SIDE_LEFT, "score", Value::Count, Source::Cache);
        assert_ne!(in_box.scope, score.scope);
        assert!(in_box.is_row_pane());
    }

    #[test]
    fn the_four_sources_are_four_names_and_none_is_a_hut_source() {
        let names: Vec<&str> = Source::ALL.iter().map(|source| source.name()).collect();
        assert_eq!(names.len(), 4);
        for (index, name) in names.iter().enumerate() {
            assert!(!name.is_empty());
            for other in &names[index + 1..] {
                assert_ne!(name, other, "deux sources, un nom");
            }
        }
        // Aucune ne se fait passer pour une source de `src/ui`, dont les noms sont
        // `ui.building`, `ui.colony`... (`ui_window_02/pane.rs`, `Source::name`).
        for name in names {
            assert!(!name.starts_with("ui."), "{name} ");
        }
    }

    #[test]
    fn a_label_says_whether_it_is_a_key_a_bare_word_or_nothing() {
        assert_eq!(Label::Xml("a.b").key(), Some("a.b"));
        assert_eq!(Label::Written("a.b").key(), Some("a.b"));
        assert!(Label::Written("a.b").is_written());
        assert!(!Label::Xml("a.b").is_written());
        // Les fleches et le mot `none` ne sont pas des cles : `Component.literal`.
        assert_eq!(Label::Literal("<").key(), None);
        assert_eq!(Label::Literal("<").text(), Some("<"));
        assert_eq!(Label::Image.key(), None);
        assert_eq!(Label::Image.text(), None);
    }

    #[test]
    fn a_press_keeps_its_messages_in_order_and_knows_its_exit() {
        const SENT: &[Message] = &[
            Message::with("MarkStoryReadOnItemMessage", "main_hand"),
            Message::plain("HutRenameMessage"),
        ];
        let press = Press::new(SENT, Exit::OpensHere("supplies"));
        assert_eq!(press.sends.len(), 2);
        assert_eq!(press.sends[0].class, "MarkStoryReadOnItemMessage");
        assert_eq!(press.sends[0].payload, Some("main_hand"));
        assert_eq!(
            press.message("HutRenameMessage").and_then(|message| message.payload),
            None
        );
        assert_eq!(press.message("ForcePickupMessage"), None);
        assert_eq!(press.exit, Exit::OpensHere("supplies"));
        assert!(!press.is_noop());
        assert!(Press::STAYS.is_noop());
        assert!(!Press::CLOSES.is_noop(), "se fermer n'est pas ne rien faire");
    }
}
