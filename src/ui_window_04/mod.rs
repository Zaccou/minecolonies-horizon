//! Les quatre fenêtres que `src/ui` laisse au bord : le guide du builder, la fenêtre
//! minimale, le renommage, et la page d'aide.
//!
//! Java reference : `core/client/gui/WindowHutGuide.java` (41 lignes),
//! `WindowHutMinPlaceholder.java` (22), `WindowHutNameEntry.java` (76), `WindowInfo.java` (70) ;
//! leurs XML sous `assets/minecolonies/gui/` ; et les trois classes mères qui font le travail,
//! `AbstractWindowSkeleton` (302), `AbstractBuildingWindow` (103), `AbstractBuildingMainWindow`
//! (180).
//!
//! **Trois des quatre sont déjà des lignes de `src/ui`, et ce module ne les redécrit pas.**
//! `SCREENS` porte `hut.min` (`WindowHutMinPlaceholder`), `hut.name` (`WindowHutNameEntry`) et
//! `hut.guide` (`WindowHutGuide`) avec leurs panneaux et leurs boutons. Ce qui manquait à cette
//! table est de trois ordres, et c'est tout ce que ce module ajoute : **qui déclare** chaque
//! bouton (deux fichiers de mise en page s'imbriquent), **ce que la classe fait** que le XML ne
//! dit pas (le titre, la visibilité du bouton `info`, le pré-remplissage du champ) et **l'état**
//! (le guide s'ouvre ou ne s'ouvre pas, une page avance ou ne bouge pas).
//!
//! La quatrième, `WindowInfo`, n'est qu'un **nom** dans `src/ui` : `Effect::OpensJava("WindowInfo")`
//! (`ui/action.rs:436-441`). C'est le seul écran que ce module décrit en entier — [`info::WINDOW`].
//!
//! # Les comptes, mesurés dans le source
//!
//! | Quoi | Combien | Où |
//! | --- | --- | --- |
//! | fenêtres décrites | 4 | [`WINDOWS`] |
//! | déjà une ligne de `src/ui` | 3 | [`describe`] |
//! | décrite en entier ici | **1** — `WindowInfo` | [`info::WINDOW`] |
//! | boutons, tous fichiers confondus | 12 | [`minimum::BUTTON_OWNERS`], [`guide`], [`info`] |
//! | boutons qui existent vraiment dans un XML | **11** | le `close` du guide manque |
//! | panneaux à valeur sans id XML | 2 | les pages de `WindowInfo` |
//! | fenêtres sans titre | 2 | le guide et `WindowInfo` |
//! | types de bâtiment avec un manuel | 18 sur 54 | [`info::MANUALS`] |
//! | pages de manuel dans la langue | 19 manuels, 72 pages | `manual_en_us.json` |
//! | pages que ce port peut rendre | 69 — `citizen` n'est pas un bâtiment | [`info::MANUALS`] |
//!
//! # Six prémisses que le source contredit
//!
//! 1. **« Ces écrans montrent nom, niveau, progression et stock. »** Deux des quatre ne montrent
//!    **rien** de tout ça : le guide est une image et trois phrases figées
//!    (`windowhutguide.xml:5-9`) et `WindowInfo` est un texte paginé. Le minimum montre un nom et
//!    un niveau (`AbstractBuildingMainWindow.java:171-176`), le renommage un champ de saisie. La
//!    seule valeur *comptée* de ces quatre fenêtres est **le numéro de page** — `3/5`, écrit par
//!    le skeleton (`AbstractWindowSkeleton.java:249`), et ce n'est pas une progression de
//!    construction.
//! 2. **« Les panneaux du guide viennent de l'avancement. »** C'est `src/ui` qui le dit
//!    (`ui/screen.rs`, `Source::Advancement` sur les quatre) et c'est faux : le XML écrit le texte
//!    en dur (`$(…)`, résolu par BlockUI au chargement) et **aucune ligne de la classe ne touche un
//!    panneau** — `WindowHutGuide` n'a que deux `registerButton` (`:33-34`). L'avancement décide
//!    de l'**ouverture**, pas du contenu ; et il est accordé **par le clic sur `confirm`**
//!    (`DefaultAdvancementsProvider.java:176-181`), donc s'en servir comme source serait circulaire.
//!    [`guide::PANE_ORIGINS`] donne l'origine réelle, panneau par panneau.
//! 3. **« Les quatre étendent `AbstractWindowSkeleton`. »** `WindowHutNameEntry` étend `BOWindow`
//!    directement (`:23`). Elle n'envoie donc ni `OpenGuiWindowTriggerMessage`
//!    (`AbstractWindowSkeleton.java:96`) ni `ClickGuiButtonTriggerMessage` (`:196`) : un clic dans
//!    la fenêtre de renommage ne déclenche aucun critère d'avancement, et son `cancel` ne passe pas
//!    par le `parent` du skeleton. Le renommage parle au serveur par lui-même
//!    (`AbstractBuildingView.java:640-644` → `HutRenameMessage`).
//! 4. **« Le guide a deux boutons. »** Java en enregistre deux (`:33-34`) et le XML n'en porte
//!    **qu'un** (`windowhutguide.xml:6`) : `GUIDE_CLOSE` (`"close"`, `WindowConstants.java:523`)
//!    ne correspond à aucun panneau. [`guide::BUTTONS_IN_XML`] est la liste du fichier,
//!    [`guide::DEAD_BINDING`] le nom mort, et un test garde les deux.
//! 5. **« Le champ de renommage s'ouvre sur le nom du bâtiment. »** Il s'ouvre sur le **nom
//!    personnalisé**, mis en minuscules, et surtout **passé à `Component.translatable`**
//!    (`WindowHutNameEntry.java:49`) : le nom saisi est traité comme une **clé de langue**. Une
//!    hutte jamais renommée ouvre donc un champ **vide**, et un nom qui est une clé connue s'affiche
//!    traduit. [`rename::prefill`] refait ce calcul, dans cet ordre.
//! 6. **« Le serveur coupe à quinze. »** Non : `HutRenameMessage` transporte la chaîne telle quelle
//!    (`HutRenameMessage.java:44-53`, `writeUtf` accepte 32 767) et
//!    `AbstractBuilding.setCustomBuildingName` la garde sans la regarder (`:747-750`). La coupe à
//!    quinze est **côté client seulement**, donc contournable ; et un renommage **renomme aussi
//!    toutes les commandes de travail en attente** de cette hutte (`:752-763`) — un effet que ni
//!    `src/ui` ni la fenêtre ne mentionnent. [`rename::SERVER`] le dit.
//!
//! # Ce qui n'est pas ici
//!
//! Le **dessin** (positions, tailles, sprites, l'image de fond du guide) et les **clés de langue
//! elles-mêmes** : `src/ui/MANQUANT.md` §1 et `MANQUANT.md` §1 de ce module disent ce qui manque
//! côté SDK. Et **l'envoi** : les deux classes de message de ces fenêtres (`HutRenameMessage`,
//! `ClickGuiButtonTriggerMessage`) sont nommées, aucune n'est envoyée — le port n'a pas de tranche
//! réseau.

pub mod guide;
pub mod info;
pub mod minimum;
pub mod refusal;
pub mod rename;

use crate::building::Kind;
use crate::ui::Screen;

use refusal::Refusal;

/// La clé de langue d'un type de bâtiment, telle que Java la fabrique.
///
/// `BuildingEntry.getTranslationKey` concatène `"com." + namespace + ".building." + path`
/// (`api/colony/buildings/registry/BuildingEntry.java:124-127`), et le chemin du registre est
/// exactement [`Kind::name`]. Les deux fenêtres qui lisent cette clé la **démontent** aussitôt :
/// `WindowInfo.java:33` et `AbstractBuildingMainWindow.java:58` font tous deux
/// `.replace("com.minecolonies.building.", "")` pour retrouver le suffixe. Cette fonction est
/// l'aller, `Kind::name` est le retour, et un test tient l'aller-retour.
pub fn building_key(hut: Kind) -> String {
    format!("com.minecolonies.building.{}", hut.name())
}

/// Un bouton de ces quatre fenêtres, et **qui le déclare**.
///
/// Le champ `source` est un nom de fichier et `line` la ligne, 1-based : un XML de mise en page
/// quand c'est le fichier qui pose le panneau, une classe Java quand c'est `registerButton` qui le
/// câble. Les deux cas existent et se ressemblent dangereusement — c'est la distinction qui a produit
/// le bouton mort du guide ([`guide::DEAD_BINDING`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    /// L'id du panneau, celui que `registerButton` cherche (`AbstractWindowSkeleton.java:130`).
    pub id: &'static str,
    /// Le fichier qui déclare le bouton, sans chemin : `layouthutpageactionsmin.xml` ou
    /// `WindowHutGuide.java`.
    pub source: &'static str,
    /// La ligne, 1-based, dans ce fichier.
    pub line: u32,
}

impl Owner {
    /// Le bouton et sa provenance, pour un message d'erreur ou une ligne de journal.
    pub fn pair(self) -> (&'static str, &'static str, u32) {
        (self.id, self.source, self.line)
    }
}

/// Une des quatre fenêtres, telle que cette table la répond.
///
/// Deux formes, parce qu'il y a deux cas et qu'un seul type mentirait : trois fenêtres **sont**
/// des lignes de `src/ui` (on rend la ligne, pas une copie), et la quatrième n'en est pas une.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: two shapes, and a third would be another module's screen
pub enum Entry {
    /// Une ligne de [`crate::ui::screen::SCREENS`], rendue telle quelle.
    Table(&'static Screen),
    /// `WindowInfo`, que `src/ui` nomme sans la décrire.
    Info(&'static info::Window),
}

impl Entry {
    /// L'id de la fenêtre : celui de `src/ui` pour les trois lignes, celui de [`info::ID`] sinon.
    pub fn id(self) -> &'static str {
        match self {
            Entry::Table(screen) => screen.id,
            Entry::Info(window) => window.id,
        }
    }

    /// La classe Java qui la construit.
    pub fn java(self) -> &'static str {
        match self {
            Entry::Table(screen) => screen.window,
            Entry::Info(window) => window.java,
        }
    }

    /// Le XML, sous `assets/minecolonies/`.
    pub fn resource(self) -> &'static str {
        match self {
            Entry::Table(screen) => screen.resource,
            Entry::Info(window) => window.resource,
        }
    }

    /// La ligne de `src/ui`, quand il y en a une — `None` pour `WindowInfo`, et ce `None` est la
    /// raison d'être de cette table.
    pub fn screen(self) -> Option<&'static Screen> {
        match self {
            Entry::Table(screen) => Some(screen),
            Entry::Info(_) => None,
        }
    }
}

/// Les quatre classes Java que ce module décrit, dans l'ordre de la mission.
pub const WINDOWS: &[&str] = &[
    "WindowHutGuide",
    "WindowHutMinPlaceholder",
    "WindowHutNameEntry",
    "WindowInfo",
];

/// La fenêtre de cette classe Java, ou [`Refusal::UnknownWindow`].
///
/// C'est la jointure avec `src/ui` : `Effect::OpensJava("WindowInfo")` et `Effect::Opens("hut.guide")`
/// tombent tous deux ici.
pub fn describe(java: &str) -> Result<Entry, Refusal> {
    match java {
        "WindowHutGuide" => Ok(Entry::Table(guide::screen()?)),
        "WindowHutMinPlaceholder" => Ok(Entry::Table(minimum::screen()?)),
        "WindowHutNameEntry" => Ok(Entry::Table(rename::screen()?)),
        info::JAVA => Ok(Entry::Info(&info::WINDOW)),
        _ => Err(Refusal::UnknownWindow),
    }
}

/// La fenêtre de cet id, ou [`Refusal::UnknownWindow`].
///
/// Les trois ids sont ceux de `src/ui` (`hut.guide`, `hut.min`, `hut.name`) et le quatrième est
/// celui de ce module. Un id d'un **autre** écran de `src/ui` — `hire`, `tab.tasks` — est refusé :
/// cette table ne répond que de ses quatre fenêtres.
pub fn describe_id(id: &str) -> Result<Entry, Refusal> {
    if id == info::ID {
        return Ok(Entry::Info(&info::WINDOW));
    }
    for java in WINDOWS {
        let Ok(entry) = describe(java) else { continue };
        if entry.id() == id {
            return Ok(entry);
        }
    }
    Err(Refusal::UnknownWindow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::action::Effect;

    #[test]
    fn the_four_windows_are_answered_by_class_and_three_of_them_are_src_ui_rows() {
        assert_eq!(WINDOWS.len(), 4);
        let mut from_table = 0;
        let mut described_here = 0;
        for java in WINDOWS {
            let entry = describe(java).unwrap_or_else(|refusal| panic!("{java}: {refusal}"));
            assert_eq!(entry.java(), *java);
            assert!(entry.resource().starts_with("minecolonies:gui/"), "{}", entry.id());
            assert!(entry.resource().ends_with(".xml"));
            // L'id répond la même fenêtre que la classe : la table est une bijection.
            assert_eq!(describe_id(entry.id()).map(Entry::java), Ok(*java));
            match entry.screen() {
                Some(screen) => {
                    from_table += 1;
                    assert_eq!(screen.window, *java);
                }
                None => described_here += 1,
            }
        }
        assert_eq!(from_table, 3, "hut.guide, hut.min, hut.name");
        assert_eq!(described_here, 1, "WindowInfo");
    }

    #[test]
    fn window_info_is_the_one_src_ui_names_without_describing() {
        // Ce que `src/ui` publie : un nom dans `Effect::OpensJava`, aucune ligne de `SCREENS`.
        assert_eq!(
            Effect::OpensJava("WindowInfo"),
            Effect::OpensJava(info::JAVA),
            "ui/screen.rs ouvre cette classe par son nom"
        );
        assert!(
            crate::ui::screen::SCREENS.iter().all(|screen| screen.window != info::JAVA),
            "une ligne de SCREENS décrirait cette fenêtre deux fois"
        );
        assert!(crate::ui::table::screen_by_id(info::ID).is_err());
        // Et c'est bien la seule des quatre dans ce cas.
        for java in WINDOWS {
            let in_table = crate::ui::screen::SCREENS.iter().any(|screen| screen.window == *java);
            assert_eq!(in_table, *java != info::JAVA, "{java}");
        }
        // Le bouton `info` du minimum est le seul chemin vers elle : `OpensJava`, pas `Opens` —
        // une fenêtre hors table ne peut pas être un id.
        let screen = minimum::screen().unwrap_or_else(|refusal| panic!("{refusal}"));
        let info_button = screen
            .action("info")
            .unwrap_or_else(|| panic!("le minimum n'a pas de bouton info"));
        assert_eq!(info_button.effect, Effect::OpensJava(info::JAVA));
        // Le guide, lui, est un id : il est dans la table.
        let guide_screen = guide::screen().unwrap_or_else(|refusal| panic!("{refusal}"));
        assert_eq!(
            guide_screen.action("confirm").map(|action| action.effect),
            Some(Effect::Opens(crate::ui::screen::HUT_BUILDER_ID))
        );
    }

    #[test]
    fn a_hut_screen_of_src_ui_that_is_not_one_of_the_four_is_refused_by_name() {
        // `WindowHutAllInventory` est une ligne de SCREENS et **pas** une fenêtre de ce module.
        assert_eq!(describe("WindowHutAllInventory"), Err(Refusal::UnknownWindow));
        assert_eq!(describe("WindowHireWorker"), Err(Refusal::UnknownWindow));
        assert_eq!(describe_id("hire"), Err(Refusal::UnknownWindow));
        assert_eq!(describe_id("tab.tasks"), Err(Refusal::UnknownWindow));
        assert_eq!(describe_id(""), Err(Refusal::UnknownWindow));
        // Le refus est nommé et silencieux, comme tous ceux de ce module.
        assert_eq!(Refusal::UnknownWindow.key(), None);
    }

    #[test]
    fn the_building_key_java_builds_is_the_one_kind_names_and_java_takes_apart_again() {
        assert_eq!(building_key(Kind::Builder), "com.minecolonies.building.builder");
        assert_eq!(building_key(Kind::TownHall), "com.minecolonies.building.townhall");
        for hut in Kind::ALL {
            let key = building_key(*hut);
            // L'aller-retour que `WindowInfo.java:33` fait à la main.
            let suffix = key.replace("com.minecolonies.building.", "");
            assert_eq!(suffix, hut.name(), "{key}");
            assert_eq!(Kind::from_name(&suffix), Some(*hut));
        }
    }
}
