//! `WindowHutMinPlaceholder` : la fenêtre que tout bâtiment ouvre quand il n'en a pas d'autre.
//!
//! Java reference : `core/client/gui/WindowHutMinPlaceholder.java` (22 lignes, dont **quatre** de
//! code utile) et `gui/layouthuts/layouthutpageactionsmin.xml`, qui **inclut**
//! `layouthutpageactionsminwoinv.xml` et ajoute deux boutons.
//!
//! **La classe ne fait rien, et c'est la trouvaille.** Son constructeur passe un XML au parent
//! (`:19-22`) : le titre, les cinq boutons et la règle du bouton `info` sont **hérités** de
//! `AbstractBuildingMainWindow` (`:43-59`, `:166-179`) ; la barre latérale vient de
//! `AbstractBuildingWindow` (`:55-84`) ; l'envoi d'ouverture vient de `AbstractWindowSkeleton`
//! (`:96`). Ce module décrit donc surtout du Java d'héritage, et c'est exactement ce que
//! `src/ui::screen::Screen` ne peut pas porter : une ligne de `SCREENS` liste cinq boutons, elle ne
//! dit pas que **trois** viennent d'un fichier et **deux** d'un autre.
//!
//! # Les deux fichiers, dans l'ordre de l'imbrication
//!
//! `layouthutpageactionsmin.xml:4` monte `layouthutpageactionsminwoinv.xml` en premier, puis pose
//! `inventory` (`:5-7`) et `allinventory` (`:8-10`). Le fichier inclus porte `name` (`:9`),
//! `editName` (`:10-11`), `build` (`:13-15`) et `info` (`:16-17`). Total : **un panneau de titre et
//! cinq boutons**, et le nom « min » ne veut pas dire « moins de boutons » : c'est la fenêtre
//! **complète** d'un bâtiment qui n'a rien de plus.
//!
//! # Ce que le titre écrit
//!
//! Deux écritures dans **un seul** panneau : le nom d'affichage, une espace, le niveau
//! (`AbstractBuildingMainWindow.java:171-176`). Le nom d'affichage est le nom personnalisé s'il n'est
//! pas vide, la clé du type sinon — c'est `IBuildingView.getBuildingDisplayName` (`:83-86`), et
//! [`crate::ui_window_04::building_key`] est la clé en question. [`title_text`] garde les deux
//! morceaux séparés au lieu de rendre une chaîne : un rendu qui traduit la clé a besoin de savoir
//! laquelle des deux il a.

use crate::building::Kind;
use crate::ui::screen::{HUT_MIN_ID, Screen};

use super::info;
use super::refusal::Refusal;
use super::Owner;

/// Le XML de la fenêtre (`WindowHutMinPlaceholder.java:21`).
pub const RESOURCE: &str = "minecolonies:gui/layouthuts/layouthutpageactionsmin.xml";

/// Le fichier qu'il monte (`layouthutpageactionsmin.xml:4`).
pub const INCLUDED: &str = "minecolonies:gui/layouthuts/layouthutpageactionsminwoinv.xml";

/// Le panneau du titre (`WindowConstants.java:559`, écrit à `AbstractBuildingMainWindow.java:53`).
pub const TITLE: &str = "name";

/// La chaîne d'imbrication, du plus externe au plus interne, avec la ligne de l'include.
pub const LAYOUTS: &[(&str, u32)] = &[("layouthutpageactionsmin.xml", 4), ("layouthutpageactionsminwoinv.xml", 0)];

/// Les cinq boutons et **le fichier qui pose chacun**.
///
/// Trois viennent du fichier inclus, deux du fichier principal : c'est la seule table de cette
/// tranche qui puisse le dire, une ligne de `SCREENS` n'ayant qu'une liste plate.
pub const BUTTON_OWNERS: &[Owner] = &[
    // `layouthutpageactionsminwoinv.xml:10-11` — le crayon, `WindowConstants.BUTTON_EDIT_NAME`.
    Owner { id: "editName", source: "layouthutpageactionsminwoinv.xml", line: 10 },
    // `:13-15` — le bouton de construction, dont le libellé change
    // (`AbstractBuildingMainWindow.java:133-163`).
    Owner { id: "build", source: "layouthutpageactionsminwoinv.xml", line: 13 },
    // `:16-17` — la pastille d'information, cachée quand il n'y a pas de manuel (`:56-59`).
    Owner { id: "info", source: "layouthutpageactionsminwoinv.xml", line: 16 },
    // `layouthutpageactionsmin.xml:5-7` — le conteneur de la hutte.
    Owner { id: "inventory", source: "layouthutpageactionsmin.xml", line: 5 },
    // `:8-10` — tout l'inventaire, une autre fenêtre.
    Owner { id: "allinventory", source: "layouthutpageactionsmin.xml", line: 8 },
];

/// La ligne de `SCREENS` que cette fenêtre **est**, ou [`Refusal::UnknownWindow`].
///
/// Le refus n'est jamais atteint sur un id constant (`HUT_MIN_ID`) : il couvre le cas d'une
/// constante cassée, et un test parcourt les trois fenêtres de ce module pour montrer que le chemin
/// d'erreur reste fermé.
pub fn screen() -> Result<&'static Screen, Refusal> {
    crate::ui::table::screen_by_id(HUT_MIN_ID).map_err(|_| Refusal::UnknownWindow)
}

/// Le titre d'une fenêtre de bâtiment : ce qu'il faut pour l'écrire.
///
/// Deux morceaux, dans l'ordre où Java les écrit (`AbstractBuildingMainWindow.java:173-174`) : le
/// nom d'affichage et le niveau. Le niveau **n'est pas** recompté ici : `crate::hut::level_of` l'a
/// déjà ramené dans `1..=5` à la lecture, et Java écrit `getBuildingLevel()` tel quel — un niveau
/// zéro s'écrit donc `0`, ce qui est le cas de toute hutte posée et pas encore construite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleText {
    /// Le nom personnalisé, ou la clé du type quand il est vide.
    pub label: String,
    /// Le niveau du bâtiment, tel que la vue le rend.
    pub level: i32,
}

impl TitleText {
    /// Le texte, une espace entre les deux — `component.append(" ").append(String.valueOf(level))`.
    pub fn text(&self) -> String {
        format!("{} {}", self.label, self.level)
    }
}

/// Ce que le titre d'une hutte de type `hut` écrit, sachant son nom personnalisé et son niveau.
///
/// **Le nom vide n'est pas un nom vide** : `getBuildingDisplayName` retombe sur la clé du type
/// (`IBuildingView.java:83-86`), que le client traduit. Un appelant qui veut du texte doit donc
/// passer la clé à la couche de langue, et c'est pour cela que les deux morceaux sont rendus
/// séparément.
pub fn title_text(custom_name: &str, hut: Kind, level: i32) -> TitleText {
    let label = if custom_name.is_empty() {
        crate::ui_window_04::building_key(hut)
    } else {
        custom_name.to_owned()
    };
    TitleText { label, level }
}

/// Le bouton `info` est-il visible pour ce bâtiment ?
///
/// La question est posée **deux fois** dans Java : `AbstractBuildingMainWindow.java:58` teste
/// `I18n.exists(prefix + ".0")` pour cacher le bouton, et `WindowInfo.java:42-47` teste la même
/// famille de clés pour compter les pages. Ce module n'en pose qu'une, dans [`info`], et cette
/// fonction est l'alias qui dit pourquoi.
pub fn info_visible(hut: Kind, translate: impl Fn(&str) -> Option<String>) -> bool {
    info::has_manual(hut, translate)
}

/// Ce que le clic sur `info` ouvre, ou [`Refusal::NoInfoPage`].
///
/// C'est la jointure entre les deux fenêtres de cette tranche : le bouton est **dans** le XML du
/// minimum (`layouthutpageactionsminwoinv.xml:16-17`), son gestionnaire est dans
/// `AbstractBuildingMainWindow` (`:92-95`, `Effect::OpensJava("WindowInfo")` dans `src/ui`), et la
/// fenêtre qu'il ouvre n'est pas dans `SCREENS` du tout. Pour trente-six des cinquante-quatre
/// types, il n'y a pas de page zéro et le bouton n'est pas là : le refus est nommé.
pub fn open_info(
    hut: Kind,
    translate: impl Fn(&str) -> Option<String>,
) -> Result<&'static info::Window, Refusal> {
    if info::has_manual(hut, translate) {
        Ok(&info::WINDOW)
    } else {
        Err(Refusal::NoInfoPage { hut })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::Value;

    fn row() -> &'static Screen {
        screen().unwrap_or_else(|refusal| panic!("{refusal}"))
    }

    /// La couche de langue d'un pack réel, reconstruite depuis [`info::MANUALS`].
    fn manual() -> impl Fn(&str) -> Option<String> {
        move |key: &str| {
            let rest = key.strip_prefix(info::PARTIAL)?;
            let (slug, page) = rest.rsplit_once('.')?;
            let page: usize = page.parse().ok()?;
            let hut = Kind::from_name(slug)?;
            (page < info::manual_of(hut)?.pages).then(|| "texte".to_owned())
        }
    }

    #[test]
    fn the_minimum_screen_is_a_row_of_src_ui_and_its_five_buttons_come_from_two_files() {
        let screen = row();
        assert_eq!(screen.id, HUT_MIN_ID);
        assert_eq!(screen.window, "WindowHutMinPlaceholder");
        assert_eq!(screen.resource, RESOURCE);
        assert_eq!(screen.actions.len(), 5);
        // L'ordre de la ligne de `src/ui` est l'ordre de cette table, id par id.
        let table: Vec<&str> = screen.actions.iter().map(|action| action.id).collect();
        let owners: Vec<&str> = BUTTON_OWNERS.iter().map(|owner| owner.id).collect();
        assert_eq!(table, owners);
        // Trois boutons du fichier inclus, deux du fichier principal.
        let included = BUTTON_OWNERS
            .iter()
            .filter(|owner| owner.source == "layouthutpageactionsminwoinv.xml")
            .count();
        assert_eq!(included, 3);
        assert_eq!(BUTTON_OWNERS.len() - included, 2);
        assert_eq!(LAYOUTS.len(), 2);
        assert_eq!(LAYOUTS[0].0, "layouthutpageactionsmin.xml");
        // Le titre est un panneau de cette fenêtre, et c'est le même id que la ligne de `src/ui`.
        assert_eq!(screen.field(TITLE).map(|field| field.value), Some(Value::Name));
    }

    #[test]
    fn the_title_is_the_display_name_then_the_level_even_at_level_zero() {
        // Nom vide : la clé du type, telle que `getBuildingDisplayName` la rend.
        let untouched = title_text("", Kind::Builder, 0);
        assert_eq!(untouched.label, "com.minecolonies.building.builder");
        assert_eq!(untouched.level, 0);
        assert_eq!(untouched.text(), "com.minecolonies.building.builder 0");
        // Nom personnalisé : gardé tel quel, sans minuscule (c'est la fenêtre de renommage qui
        // met en minuscules, et c'est une autre affaire).
        let named = title_text("Farm", Kind::Builder, 3);
        assert_eq!(named.label, "Farm");
        assert_eq!(named.text(), "Farm 3");
        // Le niveau passe tel quel : Java écrit `getBuildingLevel()` sans le clamper ici.
        assert_eq!(title_text("Farm", Kind::Builder, 5).text(), "Farm 5");
    }

    #[test]
    fn the_info_button_is_hidden_for_thirty_six_kinds_and_refuses_by_name() {
        let probe = manual();
        let visible = Kind::ALL.iter().filter(|hut| info_visible(**hut, &probe)).count();
        assert_eq!(visible, 18);
        assert_eq!(Kind::ALL.len() - visible, 36);
        // Le bouton d'un bâtiment sans manuel : nommé, silencieux, et le type est dans la donnée.
        assert_eq!(
            open_info(Kind::Alchemist, &probe),
            Err(Refusal::NoInfoPage { hut: Kind::Alchemist })
        );
        assert!(open_info(Kind::Miner, &probe).is_ok());
        // La règle `visible` et la règle `open` sont la même : aucune hutte ne peut être visible
        // sans que la fenêtre s'ouvre.
        for hut in Kind::ALL {
            assert_eq!(
                info_visible(*hut, &probe),
                open_info(*hut, &probe).is_ok(),
                "{}",
                hut.name()
            );
        }
    }

    #[test]
    fn the_minimum_window_shows_a_name_and_a_level_and_nothing_else() {
        let screen = row();
        assert!(screen.shows(Value::Name));
        assert!(screen.shows(Value::Level));
        // Ni progression, ni stock, ni priorité, ni occupation : ce sont d'autres fenêtres.
        for absent in [
            Value::Progress,
            Value::Stock,
            Value::Step,
            Value::Priority,
            Value::Occupancy,
            Value::Input,
            Value::Row,
        ] {
            assert!(!screen.shows(absent), "le minimum montre {:?}", absent);
        }
        // Et aucune porte : le guide est la seule fenêtre gardée de cette tranche.
        assert_eq!(screen.gate, None);
    }

    #[test]
    fn five_kinds_open_the_minimum_window_and_the_warehouse_is_one_of_them() {
        let mut kinds = Vec::new();
        for hut in Kind::ALL {
            let opens = match crate::ui::table::main_screen(*hut) {
                Ok(screen) => screen.id == HUT_MIN_ID,
                Err(_) => false,
            };
            if opens {
                kinds.push(*hut);
            }
        }
        // `AbstractBuildingView.java:392-397` délègue au défaut tout bâtiment sans
        // `WorkerBuildingModuleView`, et `BuildingWareHouse.java:219` /
        // `BuildingMysticalSite.java:65` le disent explicitement.
        assert_eq!(
            kinds,
            [
                Kind::MediumQuarry,
                Kind::MysticalSite,
                Kind::SimpleQuarry,
                Kind::Stash,
                Kind::Warehouse
            ],
            "les cinq huttes du défaut"
        );
        // Le builder n'en est pas : il a sa propre fenêtre (`BuildingBuilder.java:226-232`).
        assert!(!kinds.contains(&Kind::Builder));
    }
}
