//! La page d'information : l'intervalle, les evenements, et les ordres de travail.
//!
//! Java reference : `core/client/gui/townhall/WindowInfoPage.java` (311 lignes),
//! `gui/townhall/layoutinfo.xml`, et `WindowStatsPage.java:37-45` pour la table
//! des intervalles, que les deux pages partagent.
//!
//! C'est la seule des quatre pages qui **priorise** quelque chose : les fleches
//! `plus` et `minus` d'un ordre de travail (`:201-210`). Elles ecrivent une
//! priorite, pas un bit de permission, et le tri qui suit est celui du
//! gestionnaire ([`crate::colony::ordre::compare`]).

use crate::colony::ordre::{self, WorkOrder};

use super::refusal::Refusal;

/// La table des intervalles, `WindowStatsPage.INTERVAL`, dans l'ordre d'un
/// `LinkedHashMap` (`WindowStatsPage.java:37-45`) : trois fenetres et « depuis
/// toujours », code par `-1`.
pub const INTERVALS: [(&str, i32); 4] = [
    ("com.minecolonies.coremod.gui.interval.yesterday", 1),
    ("com.minecolonies.coremod.gui.interval.lastweek", 7),
    ("com.minecolonies.coremod.gui.interval.100days", 100),
    ("com.minecolonies.coremod.gui.interval.alltime", -1),
];

/// L'intervalle de la page d'information : « depuis toujours »
/// (`WindowInfoPage.java:49`).
pub const DEFAULT_KEY: &str = "com.minecolonies.coremod.gui.interval.alltime";

/// L'intervalle de la page des statistiques : « hier »
/// (`WindowStatsPage.java:55`). **Deux pages, deux defauts, une table** : les
/// deux lisent le meme `static` et ne commencent pas au meme endroit.
pub const STATS_DEFAULT_KEY: &str = "com.minecolonies.coremod.gui.interval.yesterday";

/// Les jours que vaut un intervalle, `None` si la cle n'est pas dans la table.
pub fn days_of(key: &str) -> Option<i32> {
    INTERVALS.iter().find(|(name, _)| *name == key).map(|(_, days)| *days)
}

/// Un evenement est-il dans la fenetre (`WindowInfoPage.java:109-124`) ?
///
/// Un intervalle negatif ne filtre rien ; sinon Java garde `getDay() >=
/// colony.getDay() - interval`, donc le jour de la borne est **inclus**.
pub fn keeps(day: i64, colony_day: i64, interval: i32) -> bool {
    if interval < 0 {
        return true;
    }
    day >= colony_day - i64::from(interval)
}

/// Un evenement de colonie, reduit a ce que la ligne affiche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four shapes `fillEventsList` treats differently
pub enum EventKind<'a> {
    /// `CitizenDiedEvent` : l'action est la **cause** du deces
    /// (`WindowInfoPage.java:141-144`).
    CitizenDied { citizen: &'a str, cause: &'a str },
    /// Un autre evenement de citoyen : le nom du citoyen (`:150-153`).
    Citizen { name: &'a str },
    /// Un evenement de batiment : le nom et le niveau (`:154-158`).
    Building { name: &'a str, level: i32 },
    /// Ni l'un ni l'autre : Java **n'ecrit rien** dans le panneau du nom, qui
    /// garde le texte de la ligne precedente.
    Other { name: &'a str },
}

/// Un evenement de colonie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Evenement<'a> {
    pub kind: EventKind<'a>,
    pub day: i64,
    pub at: [i32; 3],
}

impl<'a> Evenement<'a> {
    /// Le texte de la ligne `action`.
    pub fn action(&self) -> &'a str {
        match self.kind {
            EventKind::CitizenDied { cause, .. } => cause,
            EventKind::Citizen { .. } | EventKind::Other { .. } => self.raw_name(),
            EventKind::Building { name, .. } => name,
        }
    }

    /// Le nom que Java lit de l'evenement (`getName()`), pour les deux branches
    /// qui l'utilisent tel quel.
    fn raw_name(&self) -> &'a str {
        match self.kind {
            EventKind::CitizenDied { citizen, .. } => citizen,
            EventKind::Citizen { name } | EventKind::Other { name } => name,
            EventKind::Building { name, .. } => name,
        }
    }

    /// Le texte de la ligne `name`, ou `None` quand Java n'ecrit rien.
    ///
    /// `None` n'est pas « vide » : c'est le panneau qui garde le texte de la
    /// ligne precedente, parce que BlockUI recycle ses lignes. La page ne
    /// l'efface pas.
    pub fn name_text(&self) -> Option<String> {
        match self.kind {
            EventKind::CitizenDied { citizen, .. } | EventKind::Citizen { name: citizen } => {
                Some(citizen.to_owned())
            }
            EventKind::Building { name, level } => Some(format!("{name} {level}")),
            EventKind::Other { .. } => None,
        }
    }

    /// Le texte de la ligne `pos` (`:160-161`).
    pub fn pos_text(&self) -> String {
        format!("{} {} {}", self.at[0], self.at[1], self.at[2])
    }
}

/// Les evenements que l'intervalle garde, dans l'ordre de la liste.
pub fn kept<'a, 'b>(
    events: &'b [Evenement<'a>],
    colony_day: i64,
    interval: i32,
) -> Vec<&'b Evenement<'a>> {
    events.iter().filter(|event| keeps(event.day, colony_day, interval)).collect()
}

/// Le `work` d'une ligne : le nom d'affichage, ramene sur une ligne
/// (`WindowInfoPage.java:292` remplace `\n` par `": "`).
pub fn one_line(display_name: &str) -> String {
    display_name.replace('\n', ": ")
}

/// La ligne ou se trouve l'ordre d'id `id`, dans la liste triee.
pub fn row_of(orders: &[WorkOrder], id: i32) -> Option<usize> {
    orders.iter().position(|order| order.id == id)
}

/// Le tri de la liste : `IWorkOrder.WORK_ORDER_COMPARATOR`
/// (`WindowInfoPage.java:183`), deja porte par [`crate::colony::ordre::compare`].
pub fn sorted(orders: &mut [WorkOrder]) {
    orders.sort_by(ordre::compare);
}

/// La fleche `plus` d'une ligne (`:201-205`) : la priorite passe **un cran
/// au-dessus** de celle de la ligne precedente.
///
/// Java n'envoie rien sur la premiere ligne ; la derniere ligne de ce port est
/// aussi refusee plutot que relue hors du tableau.
pub fn raise(orders: &mut [WorkOrder], row: usize) -> Result<i32, Refusal> {
    if row == 0 {
        return Err(Refusal::FirstRowHasNoAbove);
    }
    if row >= orders.len() {
        return Err(Refusal::LastRowHasNoBelow);
    }
    let above = orders[row - 1].priority;
    orders[row].priority = above + 1;
    Ok(orders[row].priority)
}

/// La fleche `minus` d'une ligne (`:206-210`) : la priorite passe **un cran
/// sous** celle de la ligne suivante.
///
/// **Aucun plancher.** La priorite de livraison d'une hutte est bornee a
/// `0..=10` ([`crate::ui::field::alter_priority`]) ; celle d'un ordre de travail
/// ne l'est pas, et Java descend donc sous zero sans rien dire.
pub fn lower(orders: &mut [WorkOrder], row: usize) -> Result<i32, Refusal> {
    if row + 1 >= orders.len() {
        return Err(Refusal::LastRowHasNoBelow);
    }
    let below = orders[row + 1].priority;
    orders[row].priority = below - 1;
    Ok(orders[row].priority)
}

/// La croix d'une ligne (`:224-237`) : l'ordre quitte la liste locale par son
/// `hiddenId`, puis le message part.
pub fn drop_order(orders: &mut Vec<WorkOrder>, id: i32) -> Result<WorkOrder, Refusal> {
    let row = row_of(orders, id).ok_or(Refusal::UnknownWorkOrder { id })?;
    Ok(orders.remove(row))
}

/// Les deux fleches d'une ligne sont-elles cachees ? `(plus, minus)`
/// (`:265-278`).
///
/// Une liste d'un seul ordre cache les deux ; la premiere ligne cache `plus`,
/// la derniere cache `minus`.
pub fn arrows_hidden(row: usize, count: usize) -> (bool, bool) {
    if count <= 1 {
        return (true, true);
    }
    (row == 0, row + 1 == count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::ordre::WorkType;

    fn order(id: i32, priority: i32) -> WorkOrder {
        let mut order = WorkOrder::new(id, "minecolonies:builder", [1, 2, 3], WorkType::Build, 2);
        order.priority = priority;
        order
    }

    #[test]
    fn four_intervals_and_the_two_pages_do_not_start_on_the_same_one() {
        assert_eq!(INTERVALS.len(), 4);
        assert_eq!(days_of(DEFAULT_KEY), Some(-1));
        assert_eq!(days_of(STATS_DEFAULT_KEY), Some(1));
        assert_ne!(DEFAULT_KEY, STATS_DEFAULT_KEY, "the info page starts at all-time");
        let keys: Vec<&str> = INTERVALS.iter().map(|(key, _)| *key).collect();
        for (index, key) in keys.iter().enumerate() {
            assert!(key.starts_with("com.minecolonies.coremod.gui.interval."));
            for other in &keys[index + 1..] {
                assert_ne!(key, other);
            }
        }
        assert_eq!(days_of("nope"), None);
    }

    #[test]
    fn a_negative_interval_keeps_everything_and_the_bound_is_inclusive() {
        assert!(keeps(0, 40, -1));
        assert!(keeps(39, 40, 1), "yesterday includes colony_day - 1");
        assert!(!keeps(38, 40, 1));
        assert!(keeps(33, 40, 7));
        assert!(!keeps(32, 40, 7));
        let events = [
            Evenement { kind: EventKind::Citizen { name: "Ada" }, day: 33, at: [0, 0, 0] },
            Evenement { kind: EventKind::Citizen { name: "Bea" }, day: 10, at: [0, 0, 0] },
        ];
        assert_eq!(kept(&events, 40, -1).len(), 2);
        assert_eq!(kept(&events, 40, 7).len(), 1);
        assert_eq!(kept(&events, 40, 7)[0].action(), "Ada");
    }

    #[test]
    fn a_death_shows_its_cause_a_building_shows_its_level_and_a_third_kind_shows_nothing() {
        let died = Evenement {
            kind: EventKind::CitizenDied { citizen: "Ada", cause: "fell" },
            day: 1,
            at: [1, 2, 3],
        };
        assert_eq!(died.action(), "fell", "WindowInfoPage.java:141-144");
        assert_eq!(died.name_text().as_deref(), Some("Ada"));
        assert_eq!(died.pos_text(), "1 2 3");
        let built = Evenement {
            kind: EventKind::Building { name: "Builder Hut", level: 3 },
            day: 1,
            at: [0, 0, 0],
        };
        assert_eq!(built.name_text().as_deref(), Some("Builder Hut 3"));
        // Neither a citizen nor a building: Java leaves the pane alone.
        let other = Evenement { kind: EventKind::Other { name: "raid" }, day: 1, at: [0, 0, 0] };
        assert_eq!(other.name_text(), None);
        assert_eq!(other.action(), "raid");
        assert_eq!(one_line("Builder Hut\nlevel 2"), "Builder Hut: level 2");
    }

    #[test]
    fn raising_an_order_puts_it_above_its_neighbour_and_the_list_re_sorts() {
        let mut orders = vec![order(1, 0), order(2, 5), order(3, 2)];
        assert_eq!(raise(&mut orders, 2), Ok(6));
        assert_eq!(orders[2].priority, 6);
        sorted(&mut orders);
        let ids: Vec<i32> = orders.iter().map(|order| order.id).collect();
        assert_eq!(ids, [3, 2, 1], "higher first, then the lower id");
        // No neighbour above the first row.
        assert_eq!(raise(&mut orders, 0), Err(Refusal::FirstRowHasNoAbove));
        assert_eq!(raise(&mut orders, 9), Err(Refusal::LastRowHasNoBelow));
    }

    #[test]
    fn lowering_an_order_has_no_floor_and_the_last_row_has_no_below() {
        let mut orders = vec![order(1, 4), order(2, 0)];
        assert_eq!(lower(&mut orders, 0), Ok(-1), "Java clamps nothing here");
        assert_eq!(orders[0].priority, -1);
        assert_eq!(lower(&mut orders, 1), Err(Refusal::LastRowHasNoBelow));
        assert_eq!(lower(&mut orders, 5), Err(Refusal::LastRowHasNoBelow));
        // A priority of the delivery kind *is* clamped, which is the contrast.
        assert_eq!(crate::ui::field::alter_priority(-1, -1), 0);
    }

    #[test]
    fn the_arrows_disappear_at_the_ends_and_a_cross_removes_by_hidden_id() {
        assert_eq!(arrows_hidden(0, 1), (true, true));
        assert_eq!(arrows_hidden(0, 3), (true, false));
        assert_eq!(arrows_hidden(1, 3), (false, false));
        assert_eq!(arrows_hidden(2, 3), (false, true));
        let mut orders = vec![order(7, 0), order(8, 0)];
        assert_eq!(row_of(&orders, 8), Some(1));
        assert_eq!(row_of(&orders, 9), None);
        let gone = drop_order(&mut orders, 7).expect("the order is in the list");
        assert_eq!(gone.id, 7);
        assert_eq!(orders.len(), 1);
        assert_eq!(drop_order(&mut orders, 7), Err(Refusal::UnknownWorkOrder { id: 7 }));
    }
}
