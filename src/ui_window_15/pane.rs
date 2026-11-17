//! Un panneau de l'hotel de ville : ce qu'il porte, d'ou vient sa valeur, et la
//! ligne qui declare son id.
//!
//! **Meme forme que `src/ui`** : `(id, libelle, valeur, source, portee)` est
//! [`crate::ui::field::Field`], et [`Pane::to_ui`] le rend tel quel. Deux choses
//! sont propres a ce module : [`Pane::declared`], le XML et la ligne qui
//! **creent** l'id, et [`Reader`], qui nomme l'accesseur de la colonie.
//!
//! `Reader` n'est pas un second `Source` : c'est le meme renseignement, un cran
//! plus fin. `UiSource::Colony` dit « la colonie » ; il ne dit pas si la valeur
//! sort de `getPermissions()`, de `getFreePositions()` ou de
//! `getOverallHappiness()`. [`Reader::to_ui`] replie donc les sept lecteurs de
//! la colonie sur `UiSource::Colony` et **n'invente aucune source** : l'hotel de
//! ville n'a pas eu besoin du cas qu'a demande le journal de quetes (un
//! `static` du client sans `Source`), sauf pour l'intervalle de la page
//! d'information, qui vient justement d'un `static`.

use crate::ui::field::{Field, Scope, Source as UiSource, Value};

/// Un panneau d'une des quatre pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pane {
    /// Le `(id, libelle, valeur, source, portee)` de `src/ui`.
    pub field: Field,
    /// Qui repond, dans `src/ui`'s vocabulary : le meme renseignement que
    /// [`Field::source`], un cran plus fin.
    pub reader: Reader,
    /// Le fichier XML et la ligne qui declarent l'`id`, 1-based.
    pub declared: (&'static str, u32),
}

impl Pane {
    /// Un panneau de la fenetre, sans libelle.
    pub const fn new(
        reader: Reader,
        id: &'static str,
        value: Value,
        declared: (&'static str, u32),
    ) -> Pane {
        Pane {
            field: Field::new(id, value, reader.to_ui_or_building()),
            reader,
            declared,
        }
    }

    /// Un panneau dont le XML ecrit le libelle dans un `text` **sans id** a
    /// cote : le libelle est rattache a la valeur qu'il coiffe, comme dans
    /// `src/ui`.
    pub const fn labelled(
        reader: Reader,
        id: &'static str,
        label: &'static str,
        value: Value,
        declared: (&'static str, u32),
    ) -> Pane {
        Pane {
            field: Field::labelled(id, label, value, reader.to_ui_or_building()),
            reader,
            declared,
        }
    }

    /// Un panneau d'une ligne de la liste `list`.
    ///
    /// La portee est ce qui permet a la page des permissions de nommer `name`
    /// trois fois : dans `users`, dans `eventsList` et dans `blocks`
    /// (`layoutpermissions.xml:15`, `:41`, `:105`).
    pub const fn in_row(
        reader: Reader,
        list: &'static str,
        id: &'static str,
        value: Value,
        declared: (&'static str, u32),
    ) -> Pane {
        Pane {
            field: Field::in_row(list, id, value, reader.to_ui_or_building()),
            reader,
            declared,
        }
    }

    /// L'`id` de l'XML.
    pub fn id(&self) -> &'static str {
        self.field.id
    }

    /// Ce que le panneau porte.
    pub fn value(&self) -> Value {
        self.field.value
    }

    /// La fenetre, ou une ligne de liste.
    pub fn scope(&self) -> Scope {
        self.field.scope
    }

    /// Vrai quand le panneau appartient a une ligne.
    pub fn is_row_pane(&self) -> bool {
        self.field.is_row_pane()
    }

    /// Le panneau sous la forme de `src/ui` ; `None` pour le seul lecteur que
    /// `src/ui` n'a pas, l'intervalle d'information.
    pub fn to_ui(&self) -> Option<Field> {
        self.reader.to_ui()?;
        Some(self.field)
    }
}

/// D'ou la valeur d'un panneau vient : quel accesseur de la colonie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: les dix lecteurs de ces quatre pages, et aucun autre
pub enum Reader {
    /// La vue de l'hotel de ville elle-meme :
    /// `getBuildingDisplayName()`, `getBuildingLevel()` (`WindowMainPage.java:93`).
    TownHall,
    /// La colonie : son nom (`:371`), son jour (`WindowInfoPage.java:119`), sa
    /// joie globale (`WindowCitizenPage.java:231`), son pack, ses couleurs,
    /// son horodatage de mercenaire (`WindowMainPage.java:108-112`, `:373`).
    Colony,
    /// `getColony().getPermissions()` : les rangs et les joueurs
    /// (`WindowPermissionsPage.java:117`, `:288`).
    Permissions,
    /// `getColony().getCitizens().values()` (`WindowCitizenPage.java:99`).
    Citizens,
    /// Le gestionnaire de joie **d'un** citoyen, agrege sur toute la colonie :
    /// `getHappinessHandler().getModifier(n).getFactor(null)` somme puis divise
    /// par le nombre de citoyens (`WindowCitizenPage.java:220-249`). La ligne
    /// n'est donc la joie d'aucun citoyen en particulier.
    Happiness,
    /// `getColony().getWorkOrders()` (`WindowInfoPage.java:174`).
    WorkOrders,
    /// `buildingView.getColonyEvents()` (`WindowInfoPage.java:112`).
    ColonyEvents,
    /// `buildingView.getPermissionEvents()` (`WindowPermissionsPage.java:430`).
    PermissionEvents,
    /// `getColony().getFreeBlocks()` puis `getFreePositions()` — deux listes,
    /// une seule liste a l'ecran (`WindowPermissionsPage.java:492-501`).
    FreeBlocks,
    /// `WindowStatsPage.INTERVAL`, un `static` du client
    /// (`WindowStatsPage.java:37-45`), lu par `WindowInfoPage.java:109`.
    Intervals,
    /// Le compteur de page du squelette, qui compte les enfants du `switch` :
    /// ni colonie, ni batiment (`AbstractWindowSkeleton.java:247-250`).
    PageCounter,
}

impl Reader {
    /// Les dix lecteurs, dans l'ordre de ce fichier.
    pub const ALL: [Reader; 11] = [
        Reader::TownHall,
        Reader::Colony,
        Reader::Permissions,
        Reader::Citizens,
        Reader::Happiness,
        Reader::WorkOrders,
        Reader::ColonyEvents,
        Reader::PermissionEvents,
        Reader::FreeBlocks,
        Reader::Intervals,
        Reader::PageCounter,
    ];

    /// Un nom stable, pour un log ou un test.
    pub fn name(self) -> &'static str {
        match self {
            Reader::TownHall => "townhall",
            Reader::Colony => "colony",
            Reader::Permissions => "permissions",
            Reader::Citizens => "citizens",
            Reader::Happiness => "happiness",
            Reader::WorkOrders => "work_orders",
            Reader::ColonyEvents => "colony_events",
            Reader::PermissionEvents => "permission_events",
            Reader::FreeBlocks => "free_blocks",
            Reader::Intervals => "intervals",
            Reader::PageCounter => "page_counter",
        }
    }

    /// Le lecteur, dans le vocabulaire de `src/ui`.
    ///
    /// Seul [`Reader::Intervals`] n'en a pas : la table des intervalles est un
    /// `static` du client, pas une donnee de colonie, et la ranger sous
    /// `UiSource::Colony` serait faux.
    pub fn to_ui(self) -> Option<UiSource> {
        match self {
            Reader::TownHall => Some(UiSource::Building),
            Reader::Citizens => Some(UiSource::Citizen),
            Reader::WorkOrders => Some(UiSource::WorkOrder),
            Reader::Colony
            | Reader::Permissions
            | Reader::Happiness
            | Reader::ColonyEvents
            | Reader::PermissionEvents
            | Reader::FreeBlocks => Some(UiSource::Colony),
            Reader::Intervals | Reader::PageCounter => None,
        }
    }

    /// La source a poser dans le [`Field`] partage ; l'intervalle, qui n'en a
    /// pas, est pose sous `Colony` **pour le [`Field`] seul**, et
    /// [`Pane::to_ui`] refuse de le rendre : un `Field` dit d'ou la valeur
    /// vient, et celui-la ne peut pas le dire.
    const fn to_ui_or_building(self) -> UiSource {
        match self {
            Reader::TownHall => UiSource::Building,
            Reader::Citizens => UiSource::Citizen,
            Reader::WorkOrders => UiSource::WorkOrder,
            _ => UiSource::Colony,
        }
    }

    /// Vrai quand la valeur vit dans la colonie.
    pub fn is_colony_data(self) -> bool {
        !matches!(self, Reader::TownHall | Reader::Intervals | Reader::PageCounter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_readers_fold_onto_src_ui_and_the_two_counters_do_not() {
        let without: Vec<&str> = Reader::ALL
            .iter()
            .filter(|reader| reader.to_ui().is_none())
            .map(|reader| reader.name())
            .collect();
        assert_eq!(
            without,
            ["intervals", "page_counter"],
            "a client static and the skeleton's counter are neither colony nor building"
        );
        assert_eq!(Reader::TownHall.to_ui(), Some(UiSource::Building));
        assert_eq!(Reader::WorkOrders.to_ui(), Some(UiSource::WorkOrder));
        // The seven readers of the colony keep their own name and share one source.
        let colony: Vec<&str> = Reader::ALL
            .iter()
            .filter(|reader| reader.to_ui() == Some(UiSource::Colony))
            .map(|reader| reader.name())
            .collect();
        assert_eq!(colony.len(), 6);
        for name in &colony {
            assert!(!name.is_empty());
        }
        assert!(Reader::Permissions.is_colony_data());
        assert!(!Reader::TownHall.is_colony_data());
        assert!(!Reader::Intervals.is_colony_data());
        assert!(!Reader::PageCounter.is_colony_data());
    }

    #[test]
    fn a_pane_keeps_its_scope_its_line_and_its_reader() {
        let row = Pane::in_row(
            Reader::Permissions,
            "users",
            "name",
            Value::Name,
            ("layoutpermissions.xml", 15),
        );
        assert!(row.is_row_pane());
        assert_eq!(row.declared, ("layoutpermissions.xml", 15));
        let ui = row.to_ui().expect("permissions is a src/ui source");
        assert_eq!((ui.id, ui.value, ui.source), ("name", Value::Name, UiSource::Colony));
        // The one pane src/ui cannot take.
        let interval = Pane::new(
            Reader::Intervals,
            "intervals",
            Value::Count,
            ("layoutinfo.xml", 5),
        );
        assert_eq!(interval.to_ui(), None, "a static is not a Source");
        // Its Field still exists, and it does not pretend to be another reader.
        assert_eq!(interval.field.id, "intervals");
        assert_eq!(interval.reader, Reader::Intervals);
    }
}
