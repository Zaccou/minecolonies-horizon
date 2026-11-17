//! `WindowHutNameEntry` : le champ de renommage d'une hutte — le pré-remplissage, la coupe, et ce
//! que le serveur en fait.
//!
//! Java reference : `core/client/gui/WindowHutNameEntry.java` (76 lignes) et
//! `gui/windowhutnameentry.xml` (9), plus `AbstractBuildingView.setCustomName`
//! (`:640-644`), `HutRenameMessage` (60) et `AbstractBuilding.setCustomBuildingName`
//! (`:747-764`).
//!
//! # Une fenêtre qui n'est pas le skeleton
//!
//! `WindowHutNameEntry extends BOWindow` (`:23`), **pas** `AbstractWindowSkeleton` : elle n'envoie
//! donc ni `OpenGuiWindowTriggerMessage` (`AbstractWindowSkeleton.java:96`, l'annonce d'ouverture au
//! serveur) ni `ClickGuiButtonTriggerMessage` (`:196`, qui est ce qui déclenche les critères
//! d'avancement « clic sur tel bouton de telle fenêtre »). Son `cancel` ne passe pas non plus par le
//! `parent` du skeleton (`:254-262`). Le renommage parle au serveur **par lui-même** :
//! `building.setCustomName(name)` → `HutRenameMessage` (`AbstractBuildingView.java:640-644`), et le
//! champ est appliqué à la vue locale avant l'envoi.
//!
//! # Le pré-remplissage, dans cet ordre
//!
//! `WindowHutNameEntry.java:49` fait trois choses en une ligne :
//! `Component.translatable(building.getCustomName().toLowerCase(Locale.US)).getString()`.
//! 1. il lit le **nom personnalisé**, pas le nom d'affichage — une hutte jamais renommée a `""`
//!    (`AbstractBuildingView.java:126`, `:275-278`) et ouvre donc un champ **vide**, jamais
//!    « Builder's Hut » ;
//! 2. il **met en minuscules** — `Phi` revient en `phi` ;
//! 3. il passe le résultat à `Component.translatable` : le nom est traité comme une **clé de
//!    langue**. Un nom qui est une clé connue s'affiche traduit ; un nom inconnu s'affiche lui-même
//!    (c'est ce que fait `getString` sur une clé absente). [`prefill`] refait les trois dans cet
//!    ordre, et l'ordre compte : `"Farm".to_lowercase()` n'est pas une clé, donc le champ dit `farm`.
//!
//! # La coupe, et le mensonge du serveur
//!
//! Quinze caractères côté client (`:28`, `:59-63`) et **aucune vérification côté serveur** :
//! `HutRenameMessage` transporte la chaîne par `writeUtf` (`:49-53`, 32 767 caractères) et
//! `AbstractBuilding.setCustomBuildingName` la stocke telle quelle (`:748-750`). La coupe est donc
//! cosmétique et contournable. [`Renamed::sent`] est ce que le message transporte réellement.
//!
//! **Et le renommage n'est pas local.** Après avoir gardé le nom, le serveur **renomme toutes les
//! commandes de travail en attente** de cette hutte (`:752-763`, `WorkOrderBuilding.setCustomName`)
//! et **relit le nom du bâtiment parent** (`WorkOrderBuilding.java:118-133`). Ni `src/ui` ni la
//! fenêtre ne le disent ; [`SERVER`] le porte.
//!
//! La règle des quinze et son avertissement ne sont **pas** recopiés ici : ils vivent dans
//! `crate::ui::action::rename` (`ui/action.rs`), qui rend déjà `(nom gardé, avertissement)`. Ce
//! module ajoute ce que cette fonction ne peut pas savoir — d'où vient le texte, où il va, et sur
//! quoi Java ne compte pas comme un caractère.

use crate::ui::screen::{Screen, HUT_NAME_ID};

use super::refusal::Refusal;

/// La classe Java (`WindowHutNameEntry.java:23`).
pub const JAVA: &str = "WindowHutNameEntry";

/// Le XML (`WindowHutNameEntry.java:42`).
pub const RESOURCE: &str = "minecolonies:gui/windowhutnameentry.xml";

/// Le champ de saisie (`windowhutnameentry.xml:6`, `WindowConstants.java:927`).
pub const INPUT: &str = "name";

/// L'avertissement du nom trop long (`TranslationConstants.java:480`, levé à
/// `WindowHutNameEntry.java:62`).
pub const WARNING_KEY: &str = "com.minecolonies.coremod.gui.name.toolong";

/// Le titre de la fenêtre : un panneau **sans id**, donc pas un [`crate::ui::field::Field`]
/// (`windowhutnameentry.xml:4-5`).
pub const TITLE_KEY: &str = "com.minecolonies.coremod.gui.building.rename.title";

/// La longueur que Java garde, reprise de `src/ui` : **une** règle, un seul endroit.
pub use crate::ui::action::MAX_NAME_LENGTH;

/// La longueur que le champ accepte, imposée par le XML et par aucune classe
/// (`windowhutnameentry.xml:6`), reprise de `src/ui` pour la même raison.
pub use crate::ui::action::NAME_FIELD_MAX_LENGTH;

/// La ligne de `SCREENS` que cette fenêtre **est**, ou [`Refusal::UnknownWindow`].
pub fn screen() -> Result<&'static Screen, Refusal> {
    crate::ui::table::screen_by_id(HUT_NAME_ID).map_err(|_| Refusal::UnknownWindow)
}

/// Le texte que le champ contient à l'ouverture (`WindowHutNameEntry.java:49`).
///
/// Minuscules, puis **traduction en tant que clé**, puis repli sur la clé elle-même : c'est
/// exactement `Component.translatable(nom.toLowerCase(Locale.US)).getString()`. Le paramètre
/// `translate` est la couche de langue (`ClientHost::translate`,
/// `crates/horizon-mod-vocab/src/host/config.rs:176-179`) ; un appelant qui n'en a pas passe
/// `|_| None` et obtient le nom en minuscules, ce qui est le cas d'un nom inconnu.
///
/// `to_lowercase` est la mise en minuscules Unicode de Rust, celle que `Locale.US` applique aussi :
/// un nom accentué descend comme en Java. La seule différence connue est un couple de caractères
/// dont le changement de casse rend deux caractères (`İ`) — le champ Java les montre tous les deux,
/// et le port aussi.
pub fn prefill(custom_name: &str, translate: impl Fn(&str) -> Option<String>) -> String {
    let key = custom_name.to_lowercase();
    match translate(&key) {
        Some(text) => text,
        // `getString()` sur une clé absente rend la clé : le nom tapé revient tel quel.
        None => key,
    }
}

/// Ce que Java compte pour la longueur : des unités UTF-16, pas des caractères.
///
/// `String.length()` (`WindowHutNameEntry.java:59`) compte les `char` Java, et un caractère hors du
/// plan de base (un emoji, un idéogramme rare) en occupe **deux**. C'est ce qui rend la règle du
/// port différente de celle de Java, et [`Renamed::diverges`] le dit au lieu de le taire.
pub fn java_length(name: &str) -> usize {
    name.encode_utf16().count()
}

/// Le résultat d'un clic sur `done`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Renamed {
    /// Le nom que la vue locale garde (`:61-65`) : au plus [`MAX_NAME_LENGTH`] caractères.
    pub kept: String,
    /// L'avertissement a-t-il été levé ? (`:59-63`)
    pub warned: bool,
    /// Le nom que `HutRenameMessage` transporte réellement (`:65` → `:49-53`).
    pub sent: String,
    /// Ce que Java aurait compté, en unités UTF-16.
    pub java_length: usize,
    /// Vrai quand les deux règles ne coupent pas au même endroit — un nom que le port garde entier
    /// et que Java aurait coupé, parce que ses caractères comptent double chez lui.
    pub diverges: bool,
}

/// Le clic sur `done` : ce que la fenêtre garde, et ce qu'elle envoie.
///
/// La coupe et l'avertissement viennent de `crate::ui::action::rename` — ce module ne les réécrit
/// pas. Il ajoute la mesure en unités UTF-16, qui est la seule chose que cette règle-là ne peut pas
/// voir.
pub fn apply(typed: &str) -> Renamed {
    let (kept, warned) = crate::ui::action::rename(typed);
    let java_length = java_length(typed);
    let diverges =
        java_length > MAX_NAME_LENGTH && typed.chars().count() <= MAX_NAME_LENGTH;
    Renamed { sent: kept.clone(), kept, warned, java_length, diverges }
}

/// Ce que le serveur fait du nom reçu, et ce qu'il ne vérifie pas.
///
/// Trois effets et une absence, chacun sa ligne. C'est le seul endroit de cette tranche où le port
/// décrit un effet **serveur** : la fenêtre ne le montre pas, et c'est précisément pour ça qu'il faut
/// l'écrire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerEffect {
    /// Le nom est stocké tel quel (`AbstractBuilding.java:748-750`).
    pub keeps_the_name: bool,
    /// Toutes les commandes de travail en attente de cette hutte sont renommées
    /// (`AbstractBuilding.java:752-763`).
    pub renames_work_orders: bool,
    /// La ligne du parent d'une commande est relue, avec sa clé de traduction
    /// (`WorkOrderBuilding.java:118-133`).
    pub rereads_the_parent: bool,
    /// La longueur n'est **pas** revérifiée : `writeUtf` accepte 32 767 caractères
    /// (`HutRenameMessage.java:49-53`) et rien ne coupe côté serveur.
    pub checks_length: bool,
}

/// L'effet serveur d'un renommage, tel que le source le montre.
pub const SERVER: ServerEffect = ServerEffect {
    keeps_the_name: true,
    renames_work_orders: true,
    rereads_the_parent: true,
    checks_length: false,
};

/// Ce qu'un clic fait à la fenêtre (`:52-76`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: three branches in Java's `if` / `else if` / `return`
pub enum Pressed {
    /// `done` : le nom est gardé puis renvoyé, **et** la fenêtre du bâtiment se rouvre (`:55-65`,
    /// `:72-75`).
    Renames,
    /// `cancel` : rien n'est écrit, et la fenêtre du bâtiment se rouvre (`:67`, `:72-75`).
    Cancels,
    /// Tout autre id : Java sort avant la réouverture (`:67-70`). Le `return` est ce qui distingue
    /// ce cas du précédent, et il est facile à perdre en portant.
    Ignored,
}

/// La branche d'un clic, par id de bouton.
pub fn click(id: &str) -> Pressed {
    match id {
        "done" => Pressed::Renames,
        "cancel" => Pressed::Cancels,
        _ => Pressed::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> &'static Screen {
        screen().unwrap_or_else(|refusal| panic!("{refusal}"))
    }

    #[test]
    fn the_field_opens_on_the_lowercased_custom_name_and_never_on_the_display_name() {
        let no_lang = |_: &str| None;
        // Une hutte jamais renommée : le champ est vide, et **pas** la clé du type.
        assert_eq!(prefill("", no_lang), "");
        // Le nom personnalisé descend en minuscules.
        assert_eq!(prefill("Farm", no_lang), "farm");
        assert_eq!(prefill("Builder's Hut", no_lang), "builder's hut");
        // Un nom accentué descend comme en Java.
        assert_eq!(prefill("Ferme Élevage", no_lang), "ferme élevage");
        // Et si le nom est une clé de langue, c'est la **traduction** qui s'affiche : le nom est
        // passé à `Component.translatable`.
        let lang = |key: &str| {
            (key == "farm").then(|| "Ferme traduite".to_owned())
        };
        assert_eq!(prefill("Farm", lang), "Ferme traduite");
        // Le pré-remplissage ne prend jamais la clé du bâtiment : ce n'est pas le nom d'affichage.
        assert_ne!(
            prefill("", no_lang),
            crate::ui_window_04::building_key(crate::building::Kind::Builder)
        );
    }

    #[test]
    fn a_name_is_cut_at_fifteen_and_the_message_carries_the_cut_one() {
        let long = apply("Builder Hut Number One");
        assert_eq!(long.kept, "Builder Hut Num");
        assert_eq!(long.kept.chars().count(), MAX_NAME_LENGTH);
        assert!(long.warned, "WARNING_NAME_TOO_LONG est levé");
        assert_eq!(long.sent, long.kept, "HutRenameMessage transporte le nom coupé");
        assert!(!long.diverges, "du texte ASCII : les deux règles coupent pareil");
        // Le champ accepte vingt-cinq : la coupe est donc atteignable sans tricher.
        assert_eq!(NAME_FIELD_MAX_LENGTH, 25);
        let typed = "abcdefghijklmnopqrstuvwxy";
        assert_eq!(typed.chars().count(), NAME_FIELD_MAX_LENGTH);
        let cut = apply(typed);
        assert_eq!(cut.kept, "abcdefghijklmno");
        assert!(cut.warned);
        // Quinze pile n'avertit pas, et seize oui.
        assert!(!apply("0123456789abcde").warned);
        assert!(apply("0123456789abcdef").warned);
        // Un nom vide vide le champ : ce n'est pas un refus, c'est un effacement.
        assert_eq!(apply("").kept, "");
        assert!(!apply("").warned);
    }

    #[test]
    fn java_counts_utf16_units_so_an_emoji_name_diverges_instead_of_lying() {
        // Huit emoji : huit caractères pour Rust, seize unités UTF-16 pour Java.
        let emoji = "🎉🎉🎉🎉🎉🎉🎉🎉";
        assert_eq!(emoji.chars().count(), 8);
        assert_eq!(java_length(emoji), 16);
        let renamed = apply(emoji);
        // Le port garde les huit entiers et n'avertit pas.
        assert_eq!(renamed.kept, emoji);
        assert!(!renamed.warned);
        // Java aurait coupé à quinze unités, soit sept emoji et **un demi-caractère** : un
        // substitut orphelin, impossible à representer dans une `String` Rust. La divergence est
        // donc dans la donnée, pas dans un choix de rendu.
        assert_eq!(renamed.java_length, 16);
        assert!(renamed.diverges);
        assert_eq!(java_length("abc"), 3, "l'ASCII compte un pour un");
        assert!(!apply("🏠").diverges);
    }

    #[test]
    fn the_server_takes_the_name_as_sent_and_renames_the_huts_work_orders() {
        assert!(!SERVER.checks_length, "HutRenameMessage.java:49-53 ne vérifie rien");
        assert!(SERVER.keeps_the_name);
        assert!(SERVER.renames_work_orders, "AbstractBuilding.java:752-763");
        assert!(SERVER.rereads_the_parent, "WorkOrderBuilding.java:118-133");
        // Conséquence : un nom de trois cents caractères passe la fenêtre sans que rien côté
        // serveur ne le refuse — la coupe est cliente, donc cosmétique.
        let huge = "x".repeat(300);
        let renamed = apply(&huge);
        assert_eq!(renamed.kept.chars().count(), MAX_NAME_LENGTH);
        assert!(renamed.warned);
        assert!(!SERVER.checks_length);
        // Les trois branches du clic, et la réouverture des deux boutons.
        assert_eq!(click("done"), Pressed::Renames);
        assert_eq!(click("cancel"), Pressed::Cancels);
        assert_eq!(click("build"), Pressed::Ignored);
        assert_eq!(WARNING_KEY, "com.minecolonies.coremod.gui.name.toolong");
        // Ce que `src/ui` annonce : `done` renomme, `cancel` rouvre.
        let screen = row();
        assert_eq!(
            screen.action("done").map(|action| action.effect),
            Some(crate::ui::action::Effect::Renames)
        );
        assert_eq!(
            screen.action("cancel").map(|action| action.effect),
            Some(crate::ui::action::Effect::Reopens)
        );
        // Le champ est le seul panneau de la ligne, et c'est une saisie.
        assert_eq!(screen.fields.len(), 1);
        assert_eq!(screen.field(INPUT).map(|field| field.value), Some(crate::ui::field::Value::Input));
        assert_eq!(screen.resource, RESOURCE);
        assert_eq!(screen.window, JAVA);
        // Le titre existe dans le XML mais n'a pas d'id, donc ce n'est pas un champ : un test le
        // dit au lieu de l'oublier.
        assert!(screen.field("title").is_none());
    }
}
