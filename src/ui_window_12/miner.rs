//! L'onglet « niveaux » de la mine : une ligne par puits creusé, deux boutons par ligne.
//!
//! Java reference : `core/client/gui/modules/building/WindowHutMinerModule.java` (le
//! dessin, `onOpened` à `:80-111`, les deux gestionnaires `:58-78`) et
//! `core/colony/buildings/moduleviews/MinerLevelManagementModuleView.java` (la donnée,
//! désérialisée en `:42-64`), sur `gui/layouthuts/layoutminermodule.xml` (le squelette
//! des lignes, `:7-22`).
//!
//! **Ce que cette fenêtre demande** : une liste de couples `(nœuds, profondeur)` — un par
//! niveau —, l'index du niveau courant, et les ordres de travail de la mine. **Ce qu'elle
//! rend** : trois textes par ligne, deux boutons par ligne, et un message par clic utile.
//!
//! # Les deux textes ne sont pas des nombres
//!
//! `depth` et `nodes` sont des panneaux `Text` remplis avec une **étiquette traduite**
//! puis `": "` puis la valeur (`WindowHutMinerModule.java:103-108`). `src/ui` les déclare
//! `Value::Count` — un entier nu —, ce qui est vrai du *contenu* et faux du *panneau* :
//! ce module garde la forme de `src/ui` et rend le texte composé
//! ([`depth_text`], [`nodes_text`]), ce que le panneau affiche vraiment.
//!
//! # La profondeur affichée est décalée de un, et c'est écrit dans le code
//!
//! `getB() + 1`, avec le commentaire « Extra 1 is for Y depth fix »
//! (`WindowHutMinerModule.java:102-106`) : le couple stocké porte la coordonnée du
//! **plafond** du niveau, la fenêtre affiche celle du sol. La **recherche** d'ordre de
//! travail, elle, compare le Y brut (`MinerLevelManagementModuleView.java:106-112`) — les
//! deux nombres ne sont donc pas interchangeables, et [`Level`] garde `depth` tel quel.
//!
//! # L'onglet disparaît quand un carrier y travaille
//!
//! `isPageVisible()` répond faux tant qu'un `WorkerBuildingModuleView` de la hutte a le
//! métier `quarrier` **et** au moins un citoyen assigné
//! (`MinerLevelManagementModuleView.java:86-96`). Ce n'est pas une permission : c'est la
//! même hutte qui sert de carrière, et gérer des niveaux qu'elle ne creusera pas n'a pas
//! de sens. [`visible`] rend ce test.

use super::refusal::Refusal;

/// La clé de module de la vue (`BuildingModules.java:497-498`), la même que
/// `crate::building::Module::key` sauvegarde et que `src/ui::MODULE_SCREENS` traduit.
pub const MODULE: &str = "miner_levels";

/// Le panneau de la liste (`WindowHutMinerModule.java:29`).
pub const LIST: &str = "levels";
/// Le numéro de la ligne (`:30`) — « 01 », « 02 », … .
pub const LEVEL_PANE: &str = "level";
/// La profondeur (`:33`).
pub const DEPTH_PANE: &str = "depth";
/// Le nombre de nœuds (`:34`).
pub const NODES_PANE: &str = "nodes";
/// Le bouton de réparation, un par ligne (`:32`).
pub const REPAIR: &str = "repair";
/// Le bouton « creuser ce niveau », un par ligne (`:31`).
pub const MINE: &str = "mine";

/// Les trois panneaux que la ligne remplit, dans l'ordre du XML
/// (`layoutminermodule.xml:9-11`). Les deux boutons sont des [`super::Button`], pas des
/// panneaux : ils ne portent aucune valeur.
pub const ROW_PANES: &[&str] = &[LEVEL_PANE, DEPTH_PANE, NODES_PANE];

/// Le titre de la fenêtre (`layoutminermodule.xml:5-6`).
pub const TITLE_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.minernodelist";
/// Le texte de la liste vide (`layoutminermodule.xml:7`) : « No levels have been mined
/// yet » (`manual_en_us.json:503`).
///
/// **Ce panneau-là n'existe pas** : c'est l'attribut `emptytext` de la liste, que BlockUI
/// dessine à la place des lignes quand le fournisseur de données en rend zéro.
pub const EMPTY_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.minernolevels";
/// L'étiquette de la profondeur (`TranslationConstants.java:542`).
pub const DEPTH_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.minerleveldepth";
/// L'étiquette des nœuds (`TranslationConstants.java:538`).
pub const NODES_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.minernode";
/// Le libellé du bouton `repair` (`manual_en_us.json:501`, et le XML l'écrit aussi :
/// `layoutminermodule.xml:13-17`). Il n'a pas de constante dans `TranslationConstants`.
pub const REPAIR_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.minerrepairnode";
/// Ce que Java dit au joueur après un `repair` accepté (`TranslationConstants.java:536`,
/// dit à `WindowHutMinerModule.java:62`).
pub const REPAIR_ENQUEUED_KEY: &str = "com.minecolonies.coremod.gui.workerhuts.miner.repair.enqueued";

/// Un niveau creusé, tel que la vue le désérialise : `levelsInfo.get(index)` est un
/// `Tuple<Integer, Integer>` = `(getA, getB)` = `(nœuds, profondeur)`
/// (`MinerLevelManagementModuleView.java:30`, rempli à `:51`).
///
/// Les deux champs sont des `i32` et non des `u32` : ils viennent de `buf.readInt()`
/// (`:51`) et Java n'a rien contre une valeur négative — `String.valueOf` l'imprime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    /// `getA()` : combien de nœuds ce niveau contient (`WindowHutMinerModule.java:108`).
    pub nodes: i32,
    /// `getB()` : la coordonnée Y du plafond du niveau (`:106`, affichée `+ 1`).
    pub depth: i32,
}

impl Level {
    pub fn new(nodes: i32, depth: i32) -> Level {
        Level { nodes, depth }
    }
}

/// Un ordre de travail de mine, réduit aux **deux** faits que `doesWorkOrderExist` lit :
/// sa position et le fait qu'il concerne le puits principal.
///
/// Java teste `wo.getDisplayName().getString().contains("main") &&
/// wo.getLocation().getY() == depth` (`MinerLevelManagementModuleView.java:104-115`).
/// Le nom affiché est la **clé de traduction**, qui est le nom du schéma passé à l'ordre
/// (`AbstractWorkOrder.java:338-352`) : `infrastructure/mineshafts/minermainshaft` pour
/// le puits (`MineNode.java:390-391`), donc « main » apparaît dans la chaîne.
///
/// `main_shaft` est donc un fait que l'appelant fournit : `crate::colony::WorkOrder` ne
/// porte ni nom de schéma ni clé de traduction (`src/colony/ordre.rs:109-126`), et
/// `MANQUANT.md` §3 dit ce qu'il faudrait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MineOrder {
    /// `getLocation()` : la position de l'ordre.
    pub at: [i32; 3],
    /// `getDisplayName().getString().contains("main")`.
    pub main_shaft: bool,
}

impl MineOrder {
    pub fn new(at: [i32; 3], main_shaft: bool) -> MineOrder {
        MineOrder { at, main_shaft }
    }

    /// Le Y comparé par Java : **brut**, pas `+ 1`.
    pub fn depth(&self) -> i32 {
        self.at[1]
    }
}

/// Y a-t-il déjà un ordre de travail pour ce niveau ?
///
/// Java compare le Y **brut** du couple (`MinerLevelManagementModuleView.java:106`) à la
/// position de l'ordre : la ligne affiche `depth + 1` et la recherche utilise `depth`,
/// ce qui est cohérent tant que l'on ne mélange pas les deux.
pub fn repair_blocked(orders: &[MineOrder], depth: i32) -> bool {
    orders
        .iter()
        .any(|order| order.main_shaft && order.depth() == depth)
}

/// Le numéro d'une ligne : `String.format("%02d", index + 1)`
/// (`WindowHutMinerModule.java:97`).
///
/// Deux chiffres, complétés par un zéro : la première ligne s'appelle « 01 » et non « 1 ».
/// Au-delà de 99 le format laisse trois chiffres (`%02d` est un minimum).
pub fn level_text(index: usize) -> String {
    format!("{:02}", index + 1)
}

/// L'étiquette de la profondeur, telle que le panneau l'affiche
/// (`WindowHutMinerModule.java:103-106`) : la clé traduite, `": "`, puis `depth + 1`.
///
/// `wrapping_add` plutôt que `+` : un `i32::MAX` écrit à la main dans une sauvegarde
/// ferait paniquer un build de test, là où Java déborde en silence et affiche
/// `-2147483648` (un `int` additionné à `1`, `WindowHutMinerModule.java:106`).
pub fn depth_text(depth: i32) -> String {
    format!("{}: {}", DEPTH_KEY, depth.wrapping_add(1))
}

/// L'étiquette des nœuds (`WindowHutMinerModule.java:107-108`) : la clé, `": "`, le
/// compte.
pub fn nodes_text(nodes: i32) -> String {
    format!("{}: {}", NODES_KEY, nodes)
}

/// Une ligne rendue : trois textes et deux états de bouton.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// L'index dans `levelsInfo`, qui est aussi l'argument du message
    /// (`WindowHutMinerModule.java:60-61`, `:72-76`).
    pub index: usize,
    /// Le panneau `level`.
    pub level: String,
    /// Le panneau `depth`.
    pub depth: String,
    /// Le panneau `nodes`.
    pub nodes: String,
    /// `repair` est actif : `!doesWorkOrderExist(index)` (`:99`).
    pub may_repair: bool,
    /// `mine` est actif : `!isCurrentLevel` (`:100`).
    pub may_mine: bool,
}

/// Ce qu'un `repair` accepté annonce au joueur (`WindowHutMinerModule.java:61-62`) :
/// `MinerRepairLevelMessage` **puis** `MINER_REPAIR_ENQUEUED`.
///
/// Un seul variant : la fenêtre n'a qu'un succès. Il est un type et pas un `bool` pour
/// que l'appelant ait la clé sous la main, et pour qu'ajouter un succès soit un
/// changement visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: un seul succès dans cette fenêtre
pub enum Notice {
    /// L'ordre de réparation est dans la file du mineur.
    RepairEnqueued,
}

impl Notice {
    pub fn key(self) -> &'static str {
        match self {
            Notice::RepairEnqueued => REPAIR_ENQUEUED_KEY,
        }
    }
}

/// L'état de l'onglet : les niveaux et le niveau courant.
///
/// `current` est un index **du client** (`MinerLevelManagementModuleView.java:34-35`) :
/// il est écrit par `mineLevelClicked` **avant** que le serveur ne réponde
/// (`WindowHutMinerModule.java:75-76`), et la prochaine désérialisation le réécrit. Un
/// `current` hors de la liste est possible (vue reçue avant que le serveur ne renvoie ses
/// niveaux) et n'est pas corrigé ici : aucune ligne n'est alors « courante », et `mine`
/// reste actif partout, ce que Java fait aussi parce que `index == current` est faux
/// pour tout index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Levels {
    levels: Vec<Level>,
    current: usize,
}

impl Levels {
    pub fn new(levels: Vec<Level>, current: usize) -> Levels {
        Levels { levels, current }
    }

    pub fn len(&self) -> usize {
        self.levels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    pub fn levels(&self) -> &[Level] {
        &self.levels
    }

    /// L'index du niveau courant, tel que le client le croit.
    pub fn current(&self) -> usize {
        self.current
    }

    /// La ligne `index`, ou [`Refusal::LevelOutOfRange`].
    pub fn row(&self, index: usize, orders: &[MineOrder]) -> Result<Row, Refusal> {
        let level = self.levels.get(index).ok_or(Refusal::LevelOutOfRange {
            row: index,
            levels: self.levels.len(),
        })?;
        Ok(Row {
            index,
            level: level_text(index),
            depth: depth_text(level.depth),
            nodes: nodes_text(level.nodes),
            may_repair: !repair_blocked(orders, level.depth),
            may_mine: index != self.current,
        })
    }

    /// Toutes les lignes que la liste dessine, une par niveau, dans l'ordre de
    /// `levelsInfo` (`WindowHutMinerModule.java:89-90`).
    pub fn rows(&self, orders: &[MineOrder]) -> Vec<Row> {
        (0..self.levels.len())
            .filter_map(|index| self.row(index, orders).ok())
            .collect()
    }

    /// Le clic sur `mine` : le niveau courant devient `index`, **localement**, et le
    /// message part (`WindowHutMinerModule.java:70-78`).
    ///
    /// Les trois conditions de Java, dans son ordre : `row != current`, `row >= 0`,
    /// `row < levelsInfo.size()`. La première est un refus ([`Refusal::CurrentLevel`]),
    /// les deux autres aussi, réunies en [`Refusal::LevelOutOfRange`].
    pub fn mine(&mut self, index: usize) -> Result<usize, Refusal> {
        if index >= self.levels.len() {
            return Err(Refusal::LevelOutOfRange {
                row: index,
                levels: self.levels.len(),
            });
        }
        if index == self.current {
            return Err(Refusal::CurrentLevel { row: index });
        }
        self.current = index;
        Ok(index)
    }

    /// Le clic sur `repair` : l'ordre de réparation est mis dans la file du mineur
    /// (`WindowHutMinerModule.java:58-63`).
    ///
    /// Java ne teste rien d'autre que la ligne : le bouton est grisé quand un ordre
    /// existe (`:99`), et `repairClicked` envoie sans vérifier. Le port refuse
    /// explicitement le cas, parce qu'un clic peut arriver sur un bouton grisé par un
    /// rendu retardé.
    pub fn repair(&self, index: usize, orders: &[MineOrder]) -> Result<Notice, Refusal> {
        let level = self.levels.get(index).ok_or(Refusal::LevelOutOfRange {
            row: index,
            levels: self.levels.len(),
        })?;
        if repair_blocked(orders, level.depth) {
            return Err(Refusal::WorkOrderExists {
                level: level.depth.wrapping_add(1),
            });
        }
        Ok(Notice::RepairEnqueued)
    }

    /// Le niveau que le mineur doit creuser, si le client en connaît un.
    pub fn current_level(&self) -> Option<Level> {
        self.levels.get(self.current).copied()
    }
}

/// L'onglet est-il dessiné ? `isPageVisible()`
/// (`MinerLevelManagementModuleView.java:86-96`).
///
/// Faux dès qu'un module de travail de la hutte a le métier `quarrier` **et** au moins un
/// citoyen : la hutte est alors une carrière, et ses « niveaux » n'ont pas de sens. Java
/// parcourt **tous** les modules de travail, pas seulement le premier.
pub fn visible(quarriers_assigned: usize) -> bool {
    quarriers_assigned == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::field::Value;
    use crate::ui::table::screen_by_id;
    use crate::ui::screen::TAB_MINER_LEVELS_ID;

    fn three_levels() -> Levels {
        Levels::new(
            vec![Level::new(4, 30), Level::new(9, 29), Level::new(1, 28)],
            1,
        )
    }

    #[test]
    fn the_row_panes_are_the_ones_src_ui_declares_with_their_kinds() {
        let screen = screen_by_id(TAB_MINER_LEVELS_ID).expect("src/ui carries the miner tab");
        for id in ROW_PANES {
            let field = screen
                .field(id)
                .unwrap_or_else(|| panic!("{id} is not a pane of the miner tab"));
            assert_eq!(
                field.value,
                Value::Count,
                "{id}: src/ui declares the corner's panes as counts"
            );
            assert_eq!(field.source, crate::ui::field::Source::Module(MODULE));
        }
        assert_eq!(screen.fields[0].id, LIST, "the list comes first");
        assert_eq!(screen.fields[0].value, Value::Row);
    }

    #[test]
    fn a_level_prints_two_digits_and_the_depth_is_shifted_by_one() {
        assert_eq!(level_text(0), "01");
        assert_eq!(level_text(8), "09");
        assert_eq!(level_text(9), "10");
        assert_eq!(level_text(99), "100", "%02d is a minimum, not a clamp");
        // depth 30 is stored for the ceiling and shown as the floor.
        assert_eq!(depth_text(30), format!("{DEPTH_KEY}: 31"));
        assert_eq!(depth_text(-60), format!("{DEPTH_KEY}: -59"));
        // A hand-edited i32::MAX prints as Java does, instead of panicking.
        assert_eq!(depth_text(i32::MAX), format!("{DEPTH_KEY}: {}", i32::MIN));
        assert_eq!(nodes_text(0), format!("{NODES_KEY}: 0"));
        assert_ne!(DEPTH_KEY, NODES_KEY, "two labels, two keys");
    }

    #[test]
    fn the_current_level_is_the_one_that_cannot_be_mined() {
        let levels = three_levels();
        let rows = levels.rows(&[]);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows.iter().map(|row| row.may_mine).collect::<Vec<_>>(),
            [true, false, true]
        );
        assert!(rows.iter().all(|row| row.may_repair), "no work order exists");
    }

    #[test]
    fn mining_moves_the_current_index_and_refuses_the_two_bad_rows() {
        let mut levels = three_levels();
        assert_eq!(levels.mine(2), Ok(2));
        assert_eq!(levels.current(), 2);
        assert_eq!(levels.current_level(), Some(Level::new(1, 28)));
        // The row we are on, and a row that does not exist: Java tests both at :73.
        assert_eq!(levels.mine(2), Err(Refusal::CurrentLevel { row: 2 }));
        assert_eq!(
            levels.mine(7),
            Err(Refusal::LevelOutOfRange { row: 7, levels: 3 })
        );
        assert_eq!(levels.current(), 2, "a refused click changes nothing");
    }

    #[test]
    fn a_work_order_on_the_main_shaft_blocks_repair_at_its_own_depth_only() {
        let orders = [
            MineOrder::new([10, 29, 10], true),
            MineOrder::new([10, 28, 10], false),
        ];
        assert!(repair_blocked(&orders, 29), "main shaft at Y=29");
        assert!(
            !repair_blocked(&orders, 28),
            "a side tunnel does not block the level"
        );
        assert!(!repair_blocked(&orders, 30));
        let levels = three_levels();
        assert_eq!(
            levels.repair(1, &orders),
            Err(Refusal::WorkOrderExists { level: 30 }),
            "the refusal names the depth the player sees, not the stored one"
        );
        assert_eq!(levels.repair(0, &orders), Ok(Notice::RepairEnqueued));
        assert_eq!(
            levels.row(1, &orders).expect("row").may_repair,
            false,
            "the same test decides the button"
        );
        assert_eq!(Notice::RepairEnqueued.key(), REPAIR_ENQUEUED_KEY);
    }

    #[test]
    fn the_tab_hides_itself_when_a_quarrier_works_here() {
        assert!(visible(0));
        assert!(!visible(1));
        assert!(!visible(4));
        // The empty list is the XML's `emptytext`, not a pane (layoutminermodule.xml:7).
        assert!(Levels::new(Vec::new(), 0).is_empty());
        assert_eq!(Levels::new(Vec::new(), 0).rows(&[]), Vec::new());
        assert!(EMPTY_KEY.ends_with("minernolevels"));
    }
}
