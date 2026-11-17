//! `WindowBuildingInventory` et `WindowCitizenInventory` : deux écrans de conteneur, une
//! seule arithmétique de rangées, et un `static` que personne n'attend là.
//!
//! Java reference : `core/client/gui/containers/WindowBuildingInventory.java` (81 lignes),
//! `core/client/gui/containers/WindowCitizenInventory.java` (208 lignes),
//! `api/inventory/container/ContainerBuildingInventory.java:63-133`,
//! `api/inventory/container/ContainerCitizenInventory.java:84-220` et
//! `api/util/constant/InventoryConstants.java:11`, `:36`, `:41`.
//!
//! # Ce que ces deux fenêtres demandent, et ce qu'elles n'ont pas
//!
//! Ni l'une ni l'autre n'a de XML : ce sont des `AbstractContainerScreen`, dessinés par
//! `blit` (`WindowBuildingInventory.java:73-80`, `WindowCitizenInventory.java:112-135`).
//! Aucun panneau n'a donc d'id, et [`BUILDING_FIELDS`] comme [`CITIZEN_FIELDS`] sont
//! **vides** : ce que l'écran montre est une grille d'emplacements et deux titres dessinés
//! par `renderLabels`, pas des panneaux nommés. Un test le vérifie plutôt que de le
//! laisser au lecteur.
//!
//! # La rangée du citoyen compte quatre emplacements qu'elle ne montre pas
//!
//! `inventoryRows = (container.getItems().size() - 36) / 9` (`WindowCitizenInventory.java:82`).
//! `getItems()` rend **tous** les emplacements du menu : ceux du citoyen, les **quatre**
//! d'armure et les 36 du joueur. La division entière par neuf fait disparaître l'armure
//! (4 < 9), donc la rangée affichée est bien `slots / 9` — mais elle est calculée sur un
//! total qui la contient, et c'est ce total qui décide de la hauteur de la fenêtre et de
//! la place du titre de l'inventaire du joueur (`:105`).
//!
//! # Huit rangées de haut, mais tous les emplacements sont là
//!
//! Le dessin s'arrête à `min(inventorySize, INVENTORY_BAR_SIZE)` rangées, avec
//! `INVENTORY_BAR_SIZE = 8` (`InventoryConstants.java:41`) ; au-delà, le conteneur élargit
//! la grille en colonnes (`size / 8 + 1`, `ContainerCitizenInventory.java:148`) au lieu de
//! l'allonger, et la boucle s'arrête sur `index < size` (`:168`). Résultat : **tous** les
//! emplacements d'un handler sont ajoutés, quelle que soit sa taille — c'est
//! [`slots_shown`], et c'est la seule chose que ce module retient d'une formule de dessin.
//!
//! # L'armure d'un citoyen est fermée à clé tant qu'il n'a pas de bâtiment de travail
//!
//! `mayPlace` exige une armure de la bonne partie du corps **et** un `GuardGear` de la
//! liste construite pour le niveau du bâtiment de travail
//! (`ContainerCitizenInventory.java:256-270`). Cette liste est **vide** quand le citoyen
//! n'a pas de bâtiment ou que celui-ci est de niveau 0 (`:154-162`, le `default`), donc
//! les quatre emplacements refusent **toute** armure : ils sont là, dessinés, et
//! inutilisables.

use super::pane::Pane;
use super::refusal::Refusal;

/// La fenêtre de l'inventaire d'un bâtiment.
pub const BUILDING_JAVA: &str = "WindowBuildingInventory";
/// La fenêtre de l'inventaire d'un citoyen.
pub const CITIZEN_JAVA: &str = "WindowCitizenInventory";

/// Neuf colonnes (`InventoryConstants.java:11`).
pub const COLUMNS: usize = 9;
/// Les 36 emplacements du joueur : trois rangées de neuf plus la barre
/// (`InventoryConstants.java:36` + `:11`).
pub const PLAYER_SLOTS: usize = 36;
/// Les quatre emplacements d'armure d'un citoyen
/// (`ContainerCitizenInventory.java:193-196`).
pub const ARMOUR_SLOTS: usize = 4;
/// Les rangées que le dessin montre au plus (`InventoryConstants.java:41`,
/// `INVENTORY_BAR_SIZE`).
pub const ROWS_VISIBLE: usize = 8;
/// L'inventaire propre d'un bâtiment : un rack de taille par défaut
/// (`Constants.java:143`, `AbstractTileEntityRack.java:50`).
pub const HUT_SLOTS: usize = 27;

/// La hauteur et la largeur ne sont **pas** ici : c'est du dessin, et `TRANCHE.md` §3 M2
/// dit déjà que cette tranche n'en porte aucun. Les rangées, elles, sont une donnée : elles
/// sont `container.getSize()` côté Java.

/// Le nombre de rangées de l'inventaire d'un bâtiment : `slots / 9`
/// (`ContainerBuildingInventory.java:69-70`, lu par `WindowBuildingInventory.java:47`).
///
/// Java lèverait une exception sur un bloc disparu
/// (`(TileEntityColonyBuilding) getBlockEntity(pos)`, `:67`) ; le port refuse par un nom.
pub fn rows(slots: usize) -> Result<i32, Refusal> {
    if slots == 0 {
        return Err(Refusal::BlockGone);
    }
    Ok((slots / COLUMNS) as i32)
}

/// Le nombre de rangées de l'inventaire d'un citoyen
/// (`WindowCitizenInventory.java:82`) : `(slots + 4) / 9`, les quatre emplacements
/// d'armure étant dans le total et disparaissant dans la division.
///
/// Java, quand la colonie ou le citoyen a disparu, sort du constructeur **avant d'ajouter
/// le moindre emplacement** (`ContainerCitizenInventory.java:99-103`) : le total vaut zéro
/// et Java calcule `(0 - 36) / 9 = -4`, soit une fenêtre de hauteur négative. Le port
/// refuse par [`Refusal::ColonyUnloaded`] plutôt que de reproduire un dessin cassé — c'est
/// une divergence assumée, et elle est dite ici.
pub fn citizen_rows(slots: usize) -> Result<i32, Refusal> {
    if slots == 0 {
        return Err(Refusal::ColonyUnloaded);
    }
    Ok(((slots + ARMOUR_SLOTS) / COLUMNS) as i32)
}

/// Les rangées que le dessin montre : `Math.min(8, rangées)`
/// (`ContainerBuildingInventory.java:76` pour la boucle d'emplacements,
/// `WindowCitizenInventory.java:84` pour la hauteur).
pub fn visible_rows(rows: i32) -> i32 {
    rows.clamp(0, ROWS_VISIBLE as i32)
}

/// Combien d'emplacements le conteneur ajoute vraiment, pour un handler de `slots`.
///
/// **Tous** : les rangées sont plafonnées à huit, mais les colonnes prennent le relais
/// (`size / 8 + 1`) et la boucle s'arrête sur `index < size`
/// (`ContainerCitizenInventory.java:148`, `:164-191` ; même formule à
/// `ContainerBuildingInventory.java:72-105`).
pub fn slots_shown(slots: usize) -> usize {
    let full_rows = slots / COLUMNS;
    let shown_rows = full_rows.min(ROWS_VISIBLE);
    let columns = if full_rows <= ROWS_VISIBLE {
        COLUMNS
    } else {
        slots / ROWS_VISIBLE + 1
    };
    (shown_rows * columns).min(slots)
}

/// Les quatre emplacements d'armure, dans l'ordre où le conteneur les crée
/// (`ContainerCitizenInventory.java:193-196` : `HEAD`, `CHEST`, `LEGS`, `FEET`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustive: quatre parties du corps, celles que Java nomme
pub enum Armour {
    Head,
    Chest,
    Legs,
    Feet,
}

impl Armour {
    /// Les quatre, dans l'ordre des index du menu.
    pub const ALL: [Armour; 4] = [Armour::Head, Armour::Chest, Armour::Legs, Armour::Feet];

    /// Le rang de l'emplacement dans le menu : celui que `createArmorSlot` reçoit
    /// (`:193-196`).
    pub fn index(self) -> usize {
        match self {
            Armour::Head => 0,
            Armour::Chest => 1,
            Armour::Legs => 2,
            Armour::Feet => 3,
        }
    }

    /// Le nom de la partie du corps (`EquipmentSlot`).
    pub fn name(self) -> &'static str {
        match self {
            Armour::Head => "head",
            Armour::Chest => "chest",
            Armour::Legs => "legs",
            Armour::Feet => "feet",
        }
    }
}

/// Les niveaux d'armure de `EquipmentLevelConstants.java:32-62`.
pub const ARMOUR_LEATHER: i32 = 0;
pub const ARMOUR_GOLD: i32 = 1;
pub const ARMOUR_CHAIN: i32 = 2;
pub const ARMOUR_IRON: i32 = 3;
pub const ARMOUR_DIAMOND: i32 = 4;
/// `ARMOR_LEVEL_MAX` : `Integer.MAX_VALUE`, le plafond ouvert du niveau 5.
pub const ARMOUR_MAX: i32 = i32::MAX;

/// La plage d'armures qu'un bâtiment de travail de ce niveau accepte
/// (`WindowCitizenInventory.java:154-162`), en niveaux d'armure.
///
/// `None` est le `default` de Java — pas de bâtiment, ou niveau 0 — et c'est une
/// **liste vide**, donc un refus pour tout objet.
pub fn gear_range(work_level: i32) -> Option<(i32, i32)> {
    match work_level {
        5 => Some((ARMOUR_IRON, ARMOUR_MAX)),
        4 => Some((ARMOUR_CHAIN, ARMOUR_DIAMOND)),
        3 => Some((ARMOUR_LEATHER, ARMOUR_IRON)),
        2 => Some((ARMOUR_LEATHER, ARMOUR_CHAIN)),
        1 => Some((ARMOUR_LEATHER, ARMOUR_GOLD)),
        _ => None,
    }
}

/// Ce qu'un emplacement d'armure accepte (`ContainerCitizenInventory.java:256-270`).
///
/// `level` est le niveau d'armure de l'objet ; la partie du corps est déjà celle de
/// l'emplacement, puisque l'appelant a choisi son [`Armour`].
pub fn may_place(work_level: i32, level: i32) -> Result<(), Refusal> {
    let Some((min, max)) = gear_range(work_level) else {
        return Err(Refusal::ArmourSlotLocked);
    };
    if level < min || level > max {
        return Err(Refusal::SlotRefusesStack);
    }
    Ok(())
}

/// Le `static` de la fenêtre du citoyen : **une seule** fenêtre d'inventaire de citoyen
/// peut être « active » à la fois (`WindowCitizenInventory.java:67`).
///
/// Java l'écrit dans le constructeur (`:86`) et le remet à `null` dans `onClose` (`:195`).
/// Le trou est réel : une deuxième fenêtre écrase la première sans la fermer, et la
/// fermeture de la **première** remet le champ à `null` alors que la seconde est encore
/// ouverte. Ce type reproduit le trou au lieu de le corriger, parce que le reste du client
/// lit ce champ (`getCitizenData`, `:204-207`) et verrait la différence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Active {
    /// L'id du citoyen dont l'inventaire est le dernier ouvert, `None` au repos.
    citizen: Option<i32>,
}

impl Active {
    /// Aucune fenêtre ouverte : le `= null` de Java (`:67`).
    pub const fn none() -> Active {
        Active { citizen: None }
    }

    /// Le constructeur (`:86`) : rend l'id de la fenêtre qu'elle remplace, s'il y en avait
    /// une — que Java ne ferme pas.
    pub fn open(&mut self, citizen: i32) -> Option<i32> {
        self.citizen.replace(citizen)
    }

    /// `onClose` (`:195`) : remet à `None`, sans regarder de quelle fenêtre il s'agit.
    pub fn close(&mut self) {
        self.citizen = None;
    }

    /// Le citoyen de la fenêtre active, tel que `getCitizenData` le lirait.
    pub fn citizen(&self) -> Option<i32> {
        self.citizen
    }
}

/// Aucun panneau nommé : le titre est dessiné par `renderLabels`
/// (`WindowBuildingInventory.java:63-67`) et le reste est une grille du menu.
pub const BUILDING_FIELDS: &[Pane] = &[];
/// Aucun bouton : cet écran n'en enregistre aucun (les 81 lignes de la classe n'ont pas un
/// seul `Button`).
pub const BUILDING_BUTTONS: &[super::Button] = &[];
/// Aucun panneau nommé, pour la même raison (`WindowCitizenInventory.java:101-106`).
pub const CITIZEN_FIELDS: &[Pane] = &[];
/// Aucun bouton : la fenêtre du citoyen n'en crée aucun non plus.
pub const CITIZEN_BUTTONS: &[super::Button] = &[];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hut_container_is_three_rows_of_nine() {
        // Constants.java:143 DEFAULT_SIZE = 27, et AbstractTileEntityRack.java:50.
        assert_eq!(rows(HUT_SLOTS), Ok(3));
        assert_eq!(HUT_SLOTS, COLUMNS * 3);
        assert_eq!(rows(9), Ok(1));
        assert_eq!(rows(72), Ok(8), "un rack de magasin : 27 + 5 × 9");
        // Un bloc disparu : Java lèverait, le port refuse.
        assert_eq!(rows(0), Err(Refusal::BlockGone));
        assert_eq!(Refusal::BlockGone.key(), None);
    }

    #[test]
    fn the_citizen_row_count_includes_four_armour_slots_that_the_division_hides() {
        // (items.size() - 36) / 9 avec items = slots + 4 + 36.
        assert_eq!(citizen_rows(27), Ok(3));
        assert_eq!(citizen_rows(9), Ok(1));
        assert_eq!(citizen_rows(54), Ok(6));
        // Le total compte bien l'armure : sans les quatre, 4 slots donneraient 0 rangée.
        assert_eq!((4 + ARMOUR_SLOTS) / COLUMNS, 0);
        assert_eq!(citizen_rows(4), Ok(0));
        // La colonie disparue : Java calcule -4 et peint une fenêtre cassée.
        assert_eq!(citizen_rows(0), Err(Refusal::ColonyUnloaded));
        assert_eq!((0 - PLAYER_SLOTS as i32) / COLUMNS as i32, -4, "le calcul de Java");
    }

    #[test]
    fn every_slot_is_added_even_past_the_eight_row_cap() {
        assert_eq!(visible_rows(3), 3);
        assert_eq!(visible_rows(8), 8);
        assert_eq!(visible_rows(9), 8, "le dessin s'arrête à huit");
        assert_eq!(visible_rows(40), 8);
        for slots in (9..=180).step_by(9) {
            assert_eq!(slots_shown(slots), slots, "{slots} emplacements, pas un de moins");
        }
        // Les huit rangées suffisent, et c'est tout : la neuvième passe en colonnes.
        assert_eq!(slots_shown(72), 72);
        assert_eq!(slots_shown(81), 81);
        assert_eq!(slots_shown(0), 0);
    }

    #[test]
    fn armour_opens_from_level_one_and_the_five_ranges_grow_with_the_building() {
        // WindowCitizenInventory.java:154-162.
        assert_eq!(gear_range(0), None, "le default de Java : liste vide");
        assert_eq!(gear_range(-1), None);
        assert_eq!(gear_range(1), Some((ARMOUR_LEATHER, ARMOUR_GOLD)));
        assert_eq!(gear_range(2), Some((ARMOUR_LEATHER, ARMOUR_CHAIN)));
        assert_eq!(gear_range(3), Some((ARMOUR_LEATHER, ARMOUR_IRON)));
        assert_eq!(gear_range(4), Some((ARMOUR_CHAIN, ARMOUR_DIAMOND)));
        assert_eq!(gear_range(5), Some((ARMOUR_IRON, ARMOUR_MAX)));
        // Chaque plage est ordonnée, et celle du niveau 5 est ouverte en haut.
        for level in 1..=5 {
            let (min, max) = match gear_range(level) {
                Some(range) => range,
                None => panic!("le niveau {level} a une plage"),
            };
            assert!(min <= max, "niveau {level} : {min} > {max}");
        }
        for level in 1..=4 {
            let (min, max) = match gear_range(level) {
                Some(range) => range,
                None => continue,
            };
            assert!(max < ARMOUR_MAX, "seul le niveau 5 est ouvert");
            assert!(min >= ARMOUR_LEATHER && max <= ARMOUR_DIAMOND);
        }
    }

    #[test]
    fn an_armour_slot_refuses_everything_without_a_work_building() {
        // La liste d'armures est vide : mayPlace répond faux à tout, même à du diamant.
        assert_eq!(may_place(0, ARMOUR_DIAMOND), Err(Refusal::ArmourSlotLocked));
        assert_eq!(may_place(0, ARMOUR_LEATHER), Err(Refusal::ArmourSlotLocked));
        // À partir du niveau 1, seuls les niveaux de la plage passent.
        assert_eq!(may_place(1, ARMOUR_LEATHER), Ok(()));
        assert_eq!(may_place(1, ARMOUR_GOLD), Ok(()));
        assert_eq!(may_place(1, ARMOUR_IRON), Err(Refusal::SlotRefusesStack));
        assert_eq!(may_place(5, ARMOUR_DIAMOND), Ok(()));
        assert_eq!(may_place(5, ARMOUR_MAX), Ok(()));
        // Les quatre parties du corps ont chacune leur rang, dans l'ordre de Java.
        assert_eq!(
            Armour::ALL.iter().map(|armour| armour.index()).collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        assert_eq!(Armour::Head.name(), "head");
        assert_eq!(ARMOUR_SLOTS, Armour::ALL.len());
    }

    #[test]
    fn the_static_holds_one_citizen_window_and_the_first_close_erases_the_second() {
        let mut active = Active::none();
        assert_eq!(active.citizen(), None);
        assert_eq!(active.open(7), None, "la première fenêtre ne remplace personne");
        assert_eq!(active.citizen(), Some(7));
        // La deuxième écrase la première sans la fermer.
        assert_eq!(active.open(9), Some(7));
        assert_eq!(active.citizen(), Some(9));
        // Fermer la première (celle du citoyen 7) efface la seconde : le trou de Java.
        active.close();
        assert_eq!(
            active.citizen(),
            None,
            "onClose remet à null sans regarder de quelle fenêtre il s'agit"
        );
        // Aucun panneau, aucun bouton : les deux écrans n'en déclarent pas un seul.
        assert!(BUILDING_FIELDS.is_empty());
        assert!(CITIZEN_FIELDS.is_empty());
        assert!(BUILDING_BUTTONS.is_empty());
        assert!(CITIZEN_BUTTONS.is_empty());
    }
}
