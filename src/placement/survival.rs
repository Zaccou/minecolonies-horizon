//! Les deux handlers de `placementhandlers/main/` : qui a le droit de coller un blueprint.
//!
//! Les 21 autres fichiers disent comment poser **un** bloc. Ceux-ci décident, avant le
//! premier bloc, si le collage a lieu : c'est le clic droit avec un blueprint en main.
//! `SurvivalHandler` (`:81-287`) répond par des messages de chat, des sons et des `return`
//! — huit refus nommés — et `SuppliesHandler` (`:59-126`) y ajoute la règle de la caisse de
//! ravitaillement, une seule par joueur.
//!
//! **Pourquoi ces refus sont ici et pas dans `colony_world`.** Les six questions de
//! `crate::colony_world::guard` portent sur *une cellule* : libre, chargée, à qui. Ces huit-ci
//! portent sur *le joueur et la structure entière* : permission, distance à la colonie la
//! plus proche, boîte du blueprint entièrement dans la colonie, item en main. Deux sujets,
//! deux jeux de noms.
//!
//! **Ce que le port ne refait pas.** `isBlueprintInColony` (`:296-325`) parcourt les chunks
//! de la boîte du blueprint ; la boîte est
//! [`Placing::boxed`](crate::structures::Placing::boxed) et le propriétaire d'un chunk est
//! [`Manager::owner_at`](crate::manager::Manager::owner_at). Le port demande donc **un
//! booléen** (`Gate::inside`) au lieu de recalculer la boîte et de redemander les claims :
//! c'est la même règle, avec une seule source.
//!
//! De même, le chemin du blueprint s'appelle ici un id de
//! [`Placing`](crate::structures::Placing) (`crate::structures::Placing::id`) : c'est le même
//! texte, et c'est déjà lui que `crate::structures::Index` résout.

/// Ce que la caisse de ravitaillement est : une caisse de vaisseau ou un camp.
///
/// Le chemin du blueprint décide, et c'est Java qui le fait par un `String.contains`
/// (`SuppliesHandler.java:90-97`) : `supplyship` veut une caisse de ravitaillement, `supplycamp`
/// un camp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les deux blueprints de ravitaillement que Java nomme
pub enum Supply {
    /// `supplyship` : l'item cherché est `ModItems.supplyChest`.
    Chest,
    /// `supplycamp` : l'item cherché est `ModItems.supplyCamp`.
    Camp,
}

/// Ce que le blueprint a posé, quand il a le droit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les quatre issues d'un collage autorisé
pub enum Admission {
    /// Une hutte : le bloc d'ancrage est posé, un item consommé, et le dossier du bâtiment
    /// suit (`SurvivalHandler.java:140-265`).
    Hut,
    /// Une décoration : la fenêtre de construction s'ouvre sur le fichier de niveau 1 quand
    /// le nom en porte un (`:266-284`, `:270-278`).
    Decoration,
    /// Un champ de plantation : la fenêtre du champ s'ouvre (`:134-138`).
    Field,
    /// Une caisse de ravitaillement : le collage a lieu et un des deux items est consommé.
    Supplies(Supply),
}

/// Pourquoi le collage n'a pas eu lieu.
///
/// Chaque variante porte la clé de traduction que Java envoie au joueur, parce que c'est la
/// seule chose que le joueur voit : un refus muet serait un refus que personne ne comprend.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustif : les dix refus des deux handlers de main/
pub enum Refusal {
    /// `NO_CUSTOM_BUILDINGS` (`:92-98`) : le blueprint n'a pas pu être lu.
    UnknownBlueprint,
    /// `BP_NO_PERM` (`:104-110`) : la cellule est dans une colonie et le joueur n'a pas
    /// `MANAGE_HUTS`.
    NoPermission,
    /// `TOWNHALL_TOO_CLOSE` (`:112-125`) : un hôtel de ville hors colonie, trop près d'une
    /// autre.
    TownHallTooClose,
    /// `BP_OUTSIDE_COLONY` (`:127-132`) : la boîte du blueprint sort de la colonie.
    OutsideColony,
    /// `BUILDING_MISSING` (`:142-147`) : pack inconnu côté serveur, ou blueprint scanné
    /// (`scans/`), qui n'est pas un bâtiment.
    MissingPack,
    /// `WRONG_COLONY` (`:161-166`) : l'item en main appartient à une autre colonie.
    WrongColony { carried: i32, here: i32 },
    /// `onBlockHutPlaced` a dit non (`:150`), et Java se tait : pas de message, pas de son.
    EventRefused,
    /// Aucun item à consommer (`:152-157`) : un son d'erreur et un `return`.
    NoItem,
    /// `WARNING_SUPPLY_CHEST_ALREADY_PLACED` (`SuppliesHandler.java:81-87`) : une caisse par
    /// joueur, sauf réglage, sauf placement instantané, sauf créatif.
    AlreadyPlaced,
    /// `WARNING_REMOVING_SUPPLY_CHEST` (`:122-125`) : aucune caisse correspondante en
    /// inventaire.
    NoSupplyItem,
}

impl Refusal {
    /// Le nom du refus, et la clé de traduction que Java lui associe.
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::UnknownBlueprint => "placement.survival.unknown_blueprint",
            Refusal::NoPermission => "placement.survival.no_permission",
            Refusal::TownHallTooClose => "placement.survival.town_hall_too_close",
            Refusal::OutsideColony => "placement.survival.outside_colony",
            Refusal::MissingPack => "placement.survival.missing_pack",
            Refusal::WrongColony { .. } => "placement.survival.wrong_colony",
            Refusal::EventRefused => "placement.survival.event_refused",
            Refusal::NoItem => "placement.survival.no_item",
            Refusal::AlreadyPlaced => "placement.survival.already_placed",
            Refusal::NoSupplyItem => "placement.survival.no_supply_item",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::error::Error for Refusal {}

/// Ce que le bloc d'ancrage est, pour choisir la branche de `SurvivalHandler`.
///
/// `blueprint.getBlockState(blueprint.getPrimaryBlockOffset())` (`:101`) donne le bloc, et
/// trois `instanceof` plus un test d'identité le classent. Le port reçoit la réponse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les quatre branches que Java distingue
pub enum Anchor {
    /// `BlockHutTownHall` (`:113`).
    TownHall,
    /// `AbstractBlockHut` (`:140`).
    Hut,
    /// `ModBlocks.blockPlantationField` (`:134`).
    Field,
    /// Tout le reste : une décoration à ancre nivelée ou non (`:266-284`).
    Decoration,
}

/// Les faits que `SurvivalHandler` lit, et rien d'autre.
#[derive(Clone, Copy, Debug)]
pub struct Gate<'a> {
    /// Ce qu'est le bloc d'ancrage.
    pub anchor: Anchor,
    /// Le blueprint a-t-il été lu ? `blueprint == null` est le premier test (`:92`).
    pub known: bool,
    /// `isCoordInColony(world, blockPos)` (`:104`).
    pub in_colony: bool,
    /// `hasPermission(player, Action.MANAGE_HUTS)` (`:105`).
    pub manage_huts: bool,
    /// `isFarEnoughFromColonies(world, blockPos)` (`:115`).
    pub far_enough: bool,
    /// `isBlueprintInColony(blueprint, colony, blockPos)` (`:127`), boîte entière.
    pub inside: bool,
    /// `StructurePacks.hasPack(packName)` et le pack n'est pas seulement côté client
    /// (`:142`).
    pub pack_known: bool,
    /// `blueprintPath`.
    pub path: &'a str,
    /// `EventHandler.onBlockHutPlaced(world, player, block, pos)` (`:150`).
    pub event_allows: bool,
    /// L'identifiant de colonie que l'item en main porte (`TAG_COLONY_ID`, `:161`), ou
    /// `None` quand l'item n'en porte pas.
    pub carried: Option<i32>,
    /// La colonie de la cellule (`tempColony`, `:161`), quand il y en a une.
    pub colony: Option<i32>,
    /// Le joueur est-il en créatif ? (`:153` : un créatif n'a pas besoin de l'item.)
    pub creative: bool,
}

impl<'a> Gate<'a> {
    /// Un clic droit sur une hutte, dans une colonie, avec l'item en main.
    pub fn hut(path: &'a str) -> Gate<'a> {
        Gate {
            anchor: Anchor::Hut,
            known: true,
            in_colony: true,
            manage_huts: true,
            far_enough: true,
            inside: true,
            pack_known: true,
            path,
            event_allows: true,
            carried: None,
            colony: Some(1),
            creative: false,
        }
    }
}

/// Le clic droit a-t-il le droit de coller, et qu'est-ce qu'il colle ?
///
/// L'ordre est celui de `SurvivalHandler.handle`, et **il compte** : un joueur sans
/// permission dans une colonie apprend qu'il n'a pas la permission avant d'apprendre que son
/// pack est absent, parce que c'est l'ordre dans lequel Java répond.
pub fn admit(gate: &Gate<'_>) -> Result<Admission, Refusal> {
    // `:92-98` — pas de blueprint lu, pas de collage.
    if !gate.known {
        return Err(Refusal::UnknownBlueprint);
    }
    // `:104-110` — dans une colonie, il faut `MANAGE_HUTS`.
    if gate.in_colony && !gate.manage_huts {
        return Err(Refusal::NoPermission);
    }
    // `:112-125` — un hôtel de ville ne se pose que dans sa colonie, ou loin de toutes.
    let town_hall_ok = gate.anchor == Anchor::TownHall && (gate.in_colony || gate.far_enough);
    if gate.anchor == Anchor::TownHall && !town_hall_ok {
        return Err(Refusal::TownHallTooClose);
    }
    // `:127-132` — et tout le reste doit tenir dans la colonie.
    if (!gate.in_colony || !gate.inside) && !town_hall_ok {
        return Err(Refusal::OutsideColony);
    }
    match gate.anchor {
        // `:134-138` — la fenêtre du champ de plantation.
        Anchor::Field => Ok(Admission::Field),
        // `:140-265`
        Anchor::Hut | Anchor::TownHall => {
            if !gate.pack_known || gate.path.starts_with("scans/") {
                return Err(Refusal::MissingPack);
            }
            // `:150` — l'événement peut refuser, et Java se tait.
            if !gate.event_allows {
                return Err(Refusal::EventRefused);
            }
            // `:152-157` — un créatif pose sans item ; les autres paient un.
            if gate.carried.is_none() && !gate.creative {
                return Err(Refusal::NoItem);
            }
            // `:161-166` — un item taillé pour une autre colonie ne se pose pas ici.
            if let (Some(carried), Some(here)) = (gate.carried, gate.colony) {
                if carried != here {
                    return Err(Refusal::WrongColony { carried, here });
                }
            }
            Ok(Admission::Hut)
        }
        // `:266-284` — la fenêtre de décoration, sur le fichier de niveau 1 quand le nom en
        // porte un (`Utils.getBlueprintLevel(fileName)`, `:270`).
        Anchor::Decoration => Ok(Admission::Decoration),
    }
}

/// Les faits que `SuppliesHandler` lit.
#[derive(Clone, Copy, Debug)]
pub struct Chest<'a> {
    /// `StructurePacks.hasPack(packName)`, et le pack n'est pas côté client (`:70`).
    pub pack_known: bool,
    /// `Stats.ITEM_USED(supplyChest)` : combien de caisses ce joueur a déjà posées (`:81`).
    pub used: i32,
    /// `allowInfiniteSupplyChests` (`:81`).
    pub allow_infinite: bool,
    /// Le placement instantané du MH : l'item en main porte `PLACEMENT_NBT=instant`
    /// (`:99-104`, `:137`).
    pub instant: bool,
    /// Le joueur est-il en créatif ? (`:82`)
    pub creative: bool,
    /// Le chemin du blueprint.
    pub path: &'a str,
    /// L'item que la recherche a trouvé, ou `None` (`:106`).
    pub found: Option<Supply>,
}

/// Une caisse de ravitaillement peut-elle être posée, et laquelle ?
///
/// `supplies` est le frère de [`admit`] : même clic droit, autre blueprint. Java le traite
/// dans un handler distinct parce que la règle est différente — une caisse **par joueur**,
/// une statistique qui en fait foi, et deux items possibles selon le chemin.
pub fn supplies(chest: &Chest<'_>) -> Result<Supply, Refusal> {
    if !chest.pack_known {
        return Err(Refusal::MissingPack);
    }
    // `:81-87` — une caisse, une seule, sauf trois exceptions.
    if chest.used > 0 && !chest.allow_infinite && !chest.instant && !chest.creative {
        return Err(Refusal::AlreadyPlaced);
    }
    match chest.found {
        Some(supply) => Ok(supply),
        None => Err(Refusal::NoSupplyItem),
    }
}

/// Quel ravitaillement ce chemin demande-t-il ? (`SuppliesHandler.java:90-97`)
///
/// Renvoie `None` pour tout autre blueprint : c'est alors `SurvivalHandler` qui répond.
pub fn supply_of(path: &str) -> Option<Supply> {
    if path.contains("supplyship") {
        Some(Supply::Chest)
    } else if path.contains("supplycamp") {
        Some(Supply::Camp)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hut_needs_the_permission_before_anything_else() {
        let mut gate = Gate::hut("minecolonies:original/fundamentals/builder1");
        gate.manage_huts = false;
        gate.pack_known = false;
        // Le pack manque aussi, mais c'est la permission que Java annonce en premier
        // (`:104-110` avant `:142-147`).
        assert_eq!(admit(&gate), Err(Refusal::NoPermission));
    }

    #[test]
    fn a_missing_blueprint_is_refused_first_of_all() {
        let mut gate = Gate::hut("minecolonies:original/fundamentals/builder1");
        gate.known = false;
        gate.manage_huts = false;
        assert_eq!(admit(&gate), Err(Refusal::UnknownBlueprint));
    }

    #[test]
    fn a_town_hall_far_from_every_colony_is_refused_by_its_own_rule() {
        let mut gate = Gate::hut("minecolonies:original/fundamentals/townhall1");
        gate.anchor = Anchor::TownHall;
        gate.in_colony = false;
        gate.inside = false;
        gate.far_enough = false;
        assert_eq!(admit(&gate), Err(Refusal::TownHallTooClose));
        // Assez loin : c'est une hutte comme une autre, et elle passe.
        gate.far_enough = true;
        gate.carried = Some(1);
        assert_eq!(admit(&gate), Ok(Admission::Hut));
    }

    #[test]
    fn a_blueprint_that_leaves_the_colony_is_refused() {
        let mut gate = Gate::hut("minecolonies:original/fundamentals/builder1");
        gate.inside = false;
        assert_eq!(admit(&gate), Err(Refusal::OutsideColony));
        // Hors colonie, aucune hutte ne se pose (sauf un hôtel de ville).
        gate.in_colony = false;
        assert_eq!(admit(&gate), Err(Refusal::OutsideColony));
    }

    #[test]
    fn a_hut_still_needs_its_item_and_the_event() {
        let mut gate = Gate::hut("minecolonies:original/fundamentals/builder1");
        assert_eq!(admit(&gate), Err(Refusal::NoItem));
        gate.carried = Some(1);
        assert_eq!(admit(&gate), Ok(Admission::Hut));
        // Un créatif pose sans item (`:153`).
        gate.carried = None;
        gate.creative = true;
        assert_eq!(admit(&gate), Ok(Admission::Hut));
        // Mais pas sans l'événement (`:150`).
        gate.event_allows = false;
        assert_eq!(admit(&gate), Err(Refusal::EventRefused));
    }

    #[test]
    fn an_item_from_another_colony_is_refused_by_name() {
        let mut gate = Gate::hut("minecolonies:original/fundamentals/builder1");
        gate.carried = Some(9);
        gate.colony = Some(1);
        assert_eq!(admit(&gate), Err(Refusal::WrongColony { carried: 9, here: 1 }));
    }

    #[test]
    fn a_scanned_blueprint_is_not_a_building_and_a_known_pack_is_required() {
        let mut gate = Gate::hut("scans/maison");
        gate.carried = Some(1);
        assert_eq!(admit(&gate), Err(Refusal::MissingPack));
        gate.path = "minecolonies:original/fundamentals/builder1";
        gate.pack_known = false;
        assert_eq!(admit(&gate), Err(Refusal::MissingPack));
    }

    #[test]
    fn the_field_and_the_decoration_have_their_own_outcomes() {
        let mut gate = Gate::hut("minecolonies:original/fundamentals/plantationfield1");
        gate.anchor = Anchor::Field;
        assert_eq!(admit(&gate), Ok(Admission::Field));
        gate.anchor = Anchor::Decoration;
        assert_eq!(admit(&gate), Ok(Admission::Decoration));
    }

    #[test]
    fn one_supply_chest_per_player_unless_the_three_exceptions() {
        let base = Chest {
            pack_known: true,
            used: 2,
            allow_infinite: false,
            instant: false,
            creative: false,
            path: "supplyship",
            found: Some(Supply::Chest),
        };
        assert_eq!(supplies(&base), Err(Refusal::AlreadyPlaced));
        for exception in [
            Chest { allow_infinite: true, ..base },
            Chest { instant: true, ..base },
            Chest { creative: true, ..base },
        ] {
            assert_eq!(supplies(&exception), Ok(Supply::Chest));
        }
    }

    #[test]
    fn a_supply_blueprint_needs_its_own_item() {
        let chest = Chest {
            pack_known: true,
            used: 0,
            allow_infinite: false,
            instant: false,
            creative: false,
            path: "supplycamp",
            found: None,
        };
        assert_eq!(supplies(&chest), Err(Refusal::NoSupplyItem));
        assert_eq!(supply_of("original/supplycamp1"), Some(Supply::Camp));
        assert_eq!(supply_of("original/supplyship1"), Some(Supply::Chest));
        assert_eq!(supply_of("original/fundamentals/builder1"), None);
    }
}
