//! La table : 41 lignes, une par `canHandle`, dans l'ordre où Java les essaie.
//!
//! Java n'a pas de score : `PlacementHandlers.getHandler`
//! (`PlacementHandlers.java:162-184`) prend la **première** ligne qui accepte l'état, avec un
//! cache par bloc (`:153`) et un repli sur le handler général en dernier ressort (`:182-183`).
//! Ce fichier reproduit les trois, et l'ordre n'est pas recopié : il est **rejoué**.
//!
//! | Étape | Ce qu'elle fait | Source |
//! | --- | --- | --- |
//! | [`Table::java`] | part de 22 lignes de structurize, puis rejoue `initHandlers()` | `:55-79`, `PlacementHandlerInitializer.java:20-41` |
//! | [`insert_at_three`] | `add(handler)` insère à l'index 3 — d'où l'ordre inversé | `:141-148` |
//! | [`replace`] | `add(handler, override)` remplace la ligne de la classe nommée | `:87-101` |
//!
//! **Le cache est par bloc, pas par état.** `IdentityHashMap<Block, IPlacementHandler>`
//! (`:153`) : `oak_stairs[facing=east]` et `oak_stairs[facing=west]` partagent l'entrée,
//! parce qu'aucun `canHandle` ne lit une propriété. Le port garde la clé (le nom du bloc) et
//! le fait dire par un test, plutôt que de laisser croire à une justesse qui n'est due qu'à
//! la discipline des 41 règles.
//!
//! **Et un défaut mesuré, gardé pour mémoire.** `add(handler)` vide le cache (`:146`) ;
//! `add(handler, override)` **non** (`:87-101`). Un bloc résolu avant que
//! `PlacementHandlerInitializer` ne remplace le handler général garderait donc l'ancien. En
//! pratique l'initialiseur tourne à la construction du mod, avant le premier collage : le
//! défaut ne mord pas, mais il existe, et le port ne le reproduit pas — il n'a pas d'ordre
//! d'initialisation à préserver.

use crate::blueprint::BlockState;

use super::kind;
use super::{decide, Request, Verdict};

/// Qui possède une ligne : MineColonies, ou structurize.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustif : les deux sources de lignes de la liste
pub enum Owner {
    /// `com.minecolonies.core.placementhandlers` — porté ici.
    MineColonies,
    /// `com.ldtteam.structurize.placement.handlers.placement` — nommé, pas porté.
    Structurize,
}

/// Une ligne de la liste, par sa classe Java.
///
/// Les 19 variantes de MineColonies sont celles de
/// `PlacementHandlerInitializer.initHandlers()` (`:20-41`) ; les 22 de structurize sont celles
/// du bloc `static` de `PlacementHandlers` (`:55-79`). Deux classes existent sans ligne :
/// `SolidPlaceholderPlacementHandler` n'est jamais enregistrée, et la classe générale de
/// structurize est remplacée en place par celle de MineColonies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustif : les 41 classes qui peuvent occuper une ligne
pub enum Handler {
    /// `WeatheredCopperPlacementHandler` — le cuivre ne rajeunit pas.
    WeatheredCopper,
    /// `InfestedBlocksPlacementHandler` — l'infesté devient son hôte.
    InfestedBlocks,
    /// `HutPlacementHandler` — le bloc de bâtiment et sa donnée.
    Hut,
    /// `LecternPlacementHandler` — le pupitre et son livre.
    Lectern,
    /// `NetherrackPlacementHandler` — le nylium n'est pas constructible, la netherrack si.
    Netherrack,
    /// `GatePlacementHandler` — une porte de colonie.
    Gate,
    /// `WayPointBlockPlacementHandler` — le waypoint devient une position de colonie.
    WayPoint,
    /// `NamedGravePlacementHandler` — la tombe nommée, posée seulement en aperçu.
    NamedGrave,
    /// `GravePlacementHandler` — la tombe, jamais réécrite.
    Grave,
    /// `RackPlacementHandler` — le rack et son inventaire.
    Rack,
    /// `DimensionFluidHandler` — l'eau est gratuite, la lave du Nether aussi.
    DimensionFluid,
    /// `FieldPlacementHandler` — l'épouvantail, bloc double.
    Field,
    /// `BarracksTowerHandler` — **jamais atteinte** : `Hut` la précède.
    BarracksTower,
    /// `DoDoorBlockPlacementHandler` — la porte de Domum, deux cellules.
    DoDoor,
    /// `DoBlockPlacementHandler` — tout bloc de Domum sauf le rack.
    DoBlock,
    /// `BuilderIgnorePlacementHandler` — le bloc de structure, posé seulement en aperçu.
    BuilderIgnore,
    /// `JigsawPlacementHandler` — le jigsaw et son `final_state`.
    Jigsaw,
    /// `BeehivePlacementHandler` — la ruche sans son miel.
    Beehive,
    /// `GeneralBlockPlacementHandler` (MineColonies) — tout le reste.
    General,
    /// `AirPlacementHandler` — vider la cellule.
    Air,
    /// `SolidSubstitutionPlacementHandler` (structurize) — le placeholder de sol.
    SolidSubstitution,
    /// `SubstitutionPlacementHandler` (structurize) — le placeholder libre.
    Substitution,
    /// `BlockTagSubstitutionPlacementHandler` (structurize) — le placeholder de tag.
    TagSubstitution,
    /// `BlackListedBlockPlacementHandler` (structurize) — le tag de liste noire.
    BlackListed,
    /// `FluidSubstitutionPlacementHandler` (structurize) — le placeholder de fluide.
    FluidSubstitution,
    /// `FirePlacementHandler` (structurize) — le feu.
    Fire,
    /// `BlockGrassPathPlacementHandler` (structurize) — le chemin de terre.
    DirtPath,
    /// `GrassPlacementHandler` (structurize) — l'herbe posée en terre.
    Grass,
    /// `DoorPlacementHandler` (structurize) — la porte vanilla.
    Door,
    /// `BedPlacementHandler` (structurize) — le lit.
    Bed,
    /// `DoublePlantPlacementHandler` (structurize) — la plante double.
    DoublePlant,
    /// `SpecialBlockPlacementAttemptHandler` (structurize) — portail, spawner, œuf.
    SpecialBlock,
    /// `FlowerPotPlacementHandler` (structurize) — le pot et son contenu.
    FlowerPot,
    /// `HopperClientLagPlacementHandler` (structurize) — l'entonnoir et son `enabled`.
    Hopper,
    /// `DripStoneBlockPlacementHandler` (structurize) — la stalactite et sa colonne.
    DripStone,
    /// `FallingBlockPlacementHandler` (structurize) — le bloc qui tombe, posé sur un support.
    FallingBlock,
    /// `BannerPlacementHandler` (structurize) — la bannière debout.
    Banner,
    /// `DoBlockPlacementHandler` (structurize) — **jamais atteinte** : celle de
    /// MineColonies la précède.
    StructurizeDoBlock,
    /// `DoDoorBlockPlacementHandler` (structurize) — morte pour la même raison.
    StructurizeDoDoor,
    /// `ContainerPlacementHandler` (structurize) — le bloc à entité et son inventaire.
    Container,
    /// `GeneralBlockPlacementHandler` (structurize) — remplacée en place, donc absente de la
    /// table finale.
    StructurizeGeneral,
}

impl Handler {
    /// Le nom de classe Java, tel qu'un rapport le cite.
    pub fn class(self) -> &'static str {
        match self {
            Handler::WeatheredCopper => "WeatheredCopperPlacementHandler",
            Handler::InfestedBlocks => "InfestedBlocksPlacementHandler",
            Handler::Hut => "HutPlacementHandler",
            Handler::Lectern => "LecternPlacementHandler",
            Handler::Netherrack => "NetherrackPlacementHandler",
            Handler::Gate => "GatePlacementHandler",
            Handler::WayPoint => "WayPointBlockPlacementHandler",
            Handler::NamedGrave => "NamedGravePlacementHandler",
            Handler::Grave => "GravePlacementHandler",
            Handler::Rack => "RackPlacementHandler",
            Handler::DimensionFluid => "DimensionFluidHandler",
            Handler::Field => "FieldPlacementHandler",
            Handler::BarracksTower => "BarracksTowerHandler",
            Handler::DoDoor => "DoDoorBlockPlacementHandler",
            Handler::DoBlock => "DoBlockPlacementHandler",
            Handler::BuilderIgnore => "BuilderIgnorePlacementHandler",
            Handler::Jigsaw => "JigsawPlacementHandler",
            Handler::Beehive => "BeehivePlacementHandler",
            Handler::General => "GeneralBlockPlacementHandler",
            Handler::Air => "AirPlacementHandler",
            Handler::SolidSubstitution => "SolidSubstitutionPlacementHandler",
            Handler::Substitution => "SubstitutionPlacementHandler",
            Handler::TagSubstitution => "BlockTagSubstitutionPlacementHandler",
            Handler::BlackListed => "BlackListedBlockPlacementHandler",
            Handler::FluidSubstitution => "FluidSubstitutionPlacementHandler",
            Handler::Fire => "FirePlacementHandler",
            Handler::DirtPath => "BlockGrassPathPlacementHandler",
            Handler::Grass => "GrassPlacementHandler",
            Handler::Door => "DoorPlacementHandler",
            Handler::Bed => "BedPlacementHandler",
            Handler::DoublePlant => "DoublePlantPlacementHandler",
            Handler::SpecialBlock => "SpecialBlockPlacementAttemptHandler",
            Handler::FlowerPot => "FlowerPotPlacementHandler",
            Handler::Hopper => "HopperClientLagPlacementHandler",
            Handler::DripStone => "DripStoneBlockPlacementHandler",
            Handler::FallingBlock => "FallingBlockPlacementHandler",
            Handler::Banner => "BannerPlacementHandler",
            Handler::StructurizeDoBlock => "DoBlockPlacementHandler (structurize)",
            Handler::StructurizeDoDoor => "DoDoorBlockPlacementHandler (structurize)",
            Handler::Container => "ContainerPlacementHandler",
            Handler::StructurizeGeneral => "GeneralBlockPlacementHandler (structurize)",
        }
    }

    /// À qui la ligne appartient.
    pub fn owner(self) -> Owner {
        match self {
            Handler::WeatheredCopper
            | Handler::InfestedBlocks
            | Handler::Hut
            | Handler::Lectern
            | Handler::Netherrack
            | Handler::Gate
            | Handler::WayPoint
            | Handler::NamedGrave
            | Handler::Grave
            | Handler::Rack
            | Handler::DimensionFluid
            | Handler::Field
            | Handler::BarracksTower
            | Handler::DoDoor
            | Handler::DoBlock
            | Handler::BuilderIgnore
            | Handler::Jigsaw
            | Handler::Beehive
            | Handler::General => Owner::MineColonies,
            _ => Owner::Structurize,
        }
    }

    /// La ligne du `canHandle` qui fait accepter cette ligne, pour un rapport.
    pub fn source(self) -> &'static str {
        match self {
            Handler::WeatheredCopper => "WeatheredCopperPlacementHandler.java:82-85",
            Handler::InfestedBlocks => "InfestedBlocksPlacementHandler.java:28-31",
            Handler::Hut => "HutPlacementHandler.java:35-38",
            Handler::Lectern => "LecternPlacementHandler.java:27-32",
            Handler::Netherrack => "NetherrackPlacementHandler.java:23-26",
            Handler::Gate => "GatePlacementHandler.java:15-18",
            Handler::WayPoint => "WayPointBlockPlacementHandler.java:27-30",
            Handler::NamedGrave => "NamedGravePlacementHandler.java:23-26",
            Handler::Grave => "GravePlacementHandler.java:30-33",
            Handler::Rack => "RackPlacementHandler.java:29-32",
            Handler::DimensionFluid => "DimensionFluidHandler.java:32-35",
            Handler::Field => "FieldPlacementHandler.java:29-32",
            Handler::BarracksTower => "BarracksTowerHandler.java:31-34",
            Handler::DoDoor => "DoDoorBlockPlacementHandler.java:37-40",
            Handler::DoBlock => "DoBlockPlacementHandler.java:50-53",
            Handler::BuilderIgnore => "BuilderIgnorePlacementHandler.java:31-34",
            Handler::Jigsaw => "JigsawPlacementHandler.java:35-38",
            Handler::Beehive => "BeehivePlacementHandler.java:31-34",
            Handler::General => "GeneralBlockPlacementHandler.java:38-41",
            Handler::Air => "PlacementHandlers.java:728-732",
            Handler::SolidSubstitution => "PlacementHandlers.java:1180-1184",
            Handler::Substitution => "PlacementHandlers.java:1238-1242",
            Handler::TagSubstitution => "BlockTagSubstitutionPlacementHandler.java:33-36",
            Handler::BlackListed => "PlacementHandlers.java:1140-1144",
            Handler::FluidSubstitution => "PlacementHandlers.java:196-202",
            Handler::Fire => "PlacementHandlers.java:288-294",
            Handler::DirtPath => "PlacementHandlers.java:783-787",
            Handler::Grass => "PlacementHandlers.java:406-412",
            Handler::Door => "PlacementHandlers.java:455-461",
            Handler::Bed => "PlacementHandlers.java:528-534",
            Handler::DoublePlant => "PlacementHandlers.java:580-586",
            Handler::SpecialBlock => "PlacementHandlers.java:627-635",
            Handler::FlowerPot => "PlacementHandlers.java:670-676",
            Handler::Hopper => "PlacementHandlers.java:958-965",
            Handler::DripStone => "PlacementHandlers.java:1041-1047",
            Handler::FallingBlock => "PlacementHandlers.java:331-337",
            Handler::Banner => "PlacementHandlers.java:982-988",
            Handler::StructurizeDoBlock => "placement/DoBlockPlacementHandler.java:44-47",
            Handler::StructurizeDoDoor => "placement/DoDoorBlockPlacementHandler.java:39-43",
            Handler::Container => "PlacementHandlers.java:888-894",
            Handler::StructurizeGeneral => "PlacementHandlers.java:832-838",
        }
    }

    /// Ce module décide-t-il cette ligne ?
    pub fn ported(self) -> bool {
        self.owner() == Owner::MineColonies
    }
}

/// Ce qui fait qu'une ligne accepte un état — le corps d'un `canHandle`.
#[derive(Clone, Copy)]
// exhaustif : les quatre formes de canHandle rencontrées
pub enum Rule {
    /// `return true` : une seule ligne fait ça, et elle est dernière.
    Always,
    /// Un identifiant de bloc, exact.
    Id(&'static str),
    /// Une famille, par fonction d'id — chaque fonction citant son `instanceof`.
    Family(fn(&BlockState) -> bool),
    /// Un tag de bloc. Le port ne sait pas lire un tag depuis un id : la ligne n'accepte
    /// donc rien, et `MANQUANT.md` §2 dit ce qui manque.
    Tag(&'static str),
}

impl Rule {
    /// La ligne accepte-t-elle cet état ?
    pub fn accepts(&self, state: &BlockState) -> bool {
        match self {
            Rule::Always => true,
            Rule::Id(id) => state.name == *id,
            Rule::Family(family) => family(state),
            Rule::Tag(_) => false,
        }
    }
}

impl std::fmt::Debug for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rule::Always => f.write_str("Always"),
            Rule::Id(id) => write!(f, "Id({id})"),
            Rule::Family(_) => f.write_str("Family(..)"),
            Rule::Tag(tag) => write!(f, "Tag({tag})"),
        }
    }
}

/// Une ligne : un handler, et ce qui le fait accepter.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    /// La classe Java.
    pub handler: Handler,
    /// Son `canHandle`.
    pub rule: Rule,
}

impl Row {
    /// Une ligne.
    pub fn new(handler: Handler, rule: Rule) -> Row {
        Row { handler, rule }
    }
}

/// Le cache de Java : un handler par **bloc**, pas par état
/// (`PlacementHandlers.java:153`, `IdentityHashMap<Block, IPlacementHandler>`).
pub type Cache = std::collections::BTreeMap<String, Handler>;

/// La liste, dans l'ordre où Java l'essaie.
#[derive(Clone, Debug)]
pub struct Table {
    rows: Vec<Row>,
}

/// `PlacementHandlers.add(handler)` (`:141-148`) : insère à l'index 3, juste après l'air et
/// les deux placeholders de substitution. C'est ce `3` qui inverse les 18 lignes de
/// MineColonies, et c'est pour ça qu'il est nommé plutôt qu'écrit dans un `vec![]`.
pub fn insert_at_three(rows: &mut Vec<Row>, row: Row) {
    let at = 3.min(rows.len());
    rows.insert(at, row);
}

/// `PlacementHandlers.add(handler, override)` (`:87-101`) : remplace la ligne dont la classe
/// est `override`, et n'ajoute rien si elle est absente.
pub fn replace(rows: &mut Vec<Row>, handler: Handler, row: Row) -> bool {
    match rows.iter().position(|existing| existing.handler == handler) {
        Some(index) => {
            rows[index] = row;
            true
        }
        None => false,
    }
}

impl Table {
    /// La table du jeu : 40 lignes, rejouées depuis les deux sources Java.
    pub fn java() -> Table {
        // `PlacementHandlers.java:55-79`, dans l'ordre du bloc `static`.
        let mut rows = vec![
            Row::new(Handler::Air, Rule::Id(kind::AIR)),
            Row::new(Handler::SolidSubstitution, Rule::Id(kind::SOLID_SUBSTITUTION)),
            Row::new(Handler::Substitution, Rule::Id(kind::SUBSTITUTION)),
            Row::new(Handler::TagSubstitution, Rule::Id(kind::TAG_SUBSTITUTION)),
            Row::new(Handler::BlackListed, Rule::Tag(kind::BLUEPRINT_BLACKLIST)),
            Row::new(Handler::FluidSubstitution, Rule::Id(kind::FLUID_SUBSTITUTION)),
            Row::new(Handler::Fire, Rule::Family(kind::is_fire)),
            Row::new(Handler::DirtPath, Rule::Family(kind::is_dirt_path)),
            Row::new(Handler::Grass, Rule::Family(kind::is_grass)),
            Row::new(Handler::Door, Rule::Family(kind::is_vanilla_door)),
            Row::new(Handler::Bed, Rule::Family(kind::is_bed)),
            Row::new(Handler::DoublePlant, Rule::Family(kind::is_double_plant)),
            Row::new(Handler::SpecialBlock, Rule::Family(kind::is_special)),
            Row::new(Handler::FlowerPot, Rule::Family(kind::is_flower_pot)),
            Row::new(Handler::Hopper, Rule::Id("minecraft:hopper")),
            Row::new(Handler::DripStone, Rule::Id("minecraft:pointed_dripstone")),
            Row::new(Handler::FallingBlock, Rule::Family(kind::is_falling)),
            Row::new(Handler::Banner, Rule::Family(kind::is_banner)),
            Row::new(Handler::StructurizeDoBlock, Rule::Family(kind::is_domum)),
            Row::new(Handler::StructurizeDoDoor, Rule::Family(kind::is_domum_door)),
            Row::new(Handler::Container, Rule::Family(kind::is_container)),
            Row::new(Handler::StructurizeGeneral, Rule::Always),
        ];

        // `PlacementHandlerInitializer.java:22` : la seule ligne remplacée, pas ajoutée.
        replace(
            &mut rows,
            Handler::StructurizeGeneral,
            Row::new(Handler::General, Rule::Always),
        );

        // `PlacementHandlerInitializer.java:23-40`, dans l'ordre des appels ; chacune insère
        // à l'index 3, donc la dernière appelée passe la première.
        for (handler, rule) in [
            (Handler::Beehive, Rule::Family(kind::is_beehive)),
            (Handler::Jigsaw, Rule::Family(kind::is_jigsaw)),
            (Handler::BuilderIgnore, Rule::Family(kind::is_builder_ignored)),
            (Handler::DoBlock, Rule::Family(is_domum_block)),
            (Handler::DoDoor, Rule::Family(kind::is_domum_door)),
            (Handler::BarracksTower, Rule::Id(kind::BARRACKS_TOWER)),
            (Handler::Field, Rule::Family(kind::is_scarecrow)),
            (Handler::DimensionFluid, Rule::Family(kind::is_fluid)),
            (Handler::Rack, Rule::Family(kind::is_rack)),
            (Handler::Grave, Rule::Family(kind::is_grave)),
            (Handler::NamedGrave, Rule::Family(kind::is_named_grave)),
            (Handler::WayPoint, Rule::Family(kind::is_waypoint)),
            (Handler::Gate, Rule::Family(kind::is_gate)),
            (Handler::Netherrack, Rule::Family(kind::is_nylium)),
            (Handler::Lectern, Rule::Family(kind::is_lectern)),
            (Handler::Hut, Rule::Family(kind::is_hut)),
            (Handler::InfestedBlocks, Rule::Family(kind::is_infested)),
            (Handler::WeatheredCopper, Rule::Family(kind::is_copper)),
        ] {
            insert_at_three(&mut rows, Row::new(handler, rule));
        }

        Table { rows }
    }

    /// Une table choisie par l'appelant — pour une suite de tests qui veut une table à deux
    /// lignes, ou une table **sans** la ligne générale.
    pub fn of(rows: Vec<Row>) -> Table {
        Table { rows }
    }

    /// Les lignes, dans l'ordre où elles sont essayées.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// La première ligne qui accepte l'état, exactement comme `getHandler`
    /// (`PlacementHandlers.java:173-180`).
    pub fn first(&self, state: &BlockState) -> Option<Handler> {
        self.rows
            .iter()
            .find(|row| row.rule.accepts(state))
            .map(|row| row.handler)
    }

    /// La même réponse, par le cache de Java : la clé est le **nom du bloc**, donc deux états
    /// du même bloc partagent l'entrée (`:153-171`).
    pub fn first_cached(&self, cache: &mut Cache, state: &BlockState) -> Option<Handler> {
        if let Some(hit) = cache.get(&state.name) {
            return Some(*hit);
        }
        let handler = self.first(state)?;
        cache.insert(state.name.clone(), handler);
        Some(handler)
    }

    /// La réponse complète : la ligne, puis sa décision.
    pub fn answer(&self, request: &Request<'_>) -> Verdict {
        match self.first(request.state) {
            Some(handler) => decide(handler, request),
            None => Verdict::Refuses(super::Refusal::NoHandler {
                state: request.state.to_string(),
            }),
        }
    }
}

/// Un bloc de Domum qui n'est pas une porte ? — `DoBlockPlacementHandler.java:52` exclut le
/// rack (`&& blockState.getBlock() != ModBlocks.blockRack`) et la ligne des portes de Domum,
/// plus haut dans la liste, prend les portes.
fn is_domum_block(state: &BlockState) -> bool {
    kind::is_domum(state) && !kind::is_domum_door(state) && !kind::is_rack(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placement::fakes::state;

    /// L'index d'une ligne, pour les assertions d'ordre.
    fn at(table: &Table, handler: Handler) -> usize {
        table
            .rows()
            .iter()
            .position(|row| row.handler == handler)
            .unwrap_or_else(|| panic!("{} has no row", handler.class()))
    }

    #[test]
    fn the_java_table_has_forty_rows_and_starts_where_it_should() {
        let table = Table::java();
        assert_eq!(table.rows().len(), 40, "22 structurize rows, 18 inserted, one replaced");
        // `insert_at_three` : la dernière ligne enregistrée par MineColonies passe avant
        // toutes les autres de MineColonies (`:141-148`).
        assert_eq!(table.rows()[3].handler, Handler::WeatheredCopper);
        assert_eq!(table.rows()[0].handler, Handler::Air);
        assert_eq!(table.rows()[1].handler, Handler::SolidSubstitution);
        assert_eq!(table.rows()[2].handler, Handler::Substitution);
        // Et la ligne générale est la dernière, remplacée en place.
        assert_eq!(table.rows()[39].handler, Handler::General);
        assert_eq!(table.rows()[39].handler.owner(), Owner::MineColonies);
    }

    #[test]
    fn the_registration_order_is_reversed_at_the_head() {
        let table = Table::java();
        // Les 18 lignes de MineColonies occupent 3..=20, dans l'ordre inverse des appels de
        // `initHandlers()` (`:23-40`) : l'abeille enregistrée en premier finit dernière.
        assert_eq!(at(&table, Handler::Beehive), 20);
        assert_eq!(at(&table, Handler::Jigsaw), 19);
        assert_eq!(at(&table, Handler::Hut), 5);
        assert_eq!(at(&table, Handler::Gate), 8);
        assert_eq!(at(&table, Handler::DoDoor), 16);
        assert_eq!(at(&table, Handler::DoBlock), 17);
        // Toutes avant la première ligne de structurize.
        assert!(at(&table, Handler::WeatheredCopper) < at(&table, Handler::TagSubstitution));
    }

    #[test]
    fn the_barracks_tower_is_never_reached_and_the_hut_takes_it() {
        let table = Table::java();
        let tower = state(kind::BARRACKS_TOWER);
        // `BarracksTowerHandler` est en 15 et `HutPlacementHandler` en 5
        // (`BarracksTowerHandler.java:33`, `HutPlacementHandler.java:37`).
        assert_eq!(table.first(&tower), Some(Handler::Hut));
        assert!(at(&table, Handler::Hut) < at(&table, Handler::BarracksTower));
    }

    #[test]
    fn the_two_structurize_do_rows_are_shadowed_too() {
        let table = Table::java();
        let door = state(kind::DOMUM_FANCY_DOOR);
        assert_eq!(table.first(&door), Some(Handler::DoDoor));
        let frame = state("domum_ornamentum:materially_retexturable");
        assert_eq!(table.first(&frame), Some(Handler::DoBlock));
        assert!(at(&table, Handler::DoBlock) < at(&table, Handler::StructurizeDoBlock));
        assert!(at(&table, Handler::DoDoor) < at(&table, Handler::StructurizeDoDoor));
    }

    #[test]
    fn a_gate_wins_over_the_vanilla_door_row_because_it_is_earlier() {
        let table = Table::java();
        // La porte est aussi une `DoorBlock` (`AbstractBlockGate.java:40`), et la ligne
        // `Door` de structurize est en 27 : l'ordre suffit, aucune règle spéciale.
        assert_eq!(table.first(&state(kind::GATE_WOOD)), Some(Handler::Gate));
        assert_eq!(table.first(&state("minecraft:oak_door")), Some(Handler::Door));
    }

    #[test]
    fn the_rack_is_the_rack_and_not_a_domum_block() {
        let table = Table::java();
        assert_eq!(table.first(&state(kind::RACK)), Some(Handler::Rack));
        assert!(at(&table, Handler::Rack) < at(&table, Handler::DoBlock));
        // La table dit la même chose même si l'exclusion du Java était retirée : l'ordre
        // suffirait. Le test fige les deux, parce que l'un des deux peut changer seul.
        assert_eq!(
            Table::of(vec![
                Row::new(Handler::Rack, Rule::Family(kind::is_rack)),
                Row::new(Handler::DoBlock, Rule::Always),
            ])
            .first(&state(kind::RACK)),
            Some(Handler::Rack)
        );
    }

    #[test]
    fn a_blacklisted_block_falls_through_because_the_port_cannot_read_a_tag() {
        let table = Table::java();
        // Le tag de liste noire n'est pas dans l'id : la ligne n'accepte rien, et le bloc
        // part au handler général, qui l'écrit. C'est `MANQUANT.md` §2, mesuré ici.
        assert!(!Rule::Tag(kind::BLUEPRINT_BLACKLIST).accepts(&state("minecraft:stone")));
        assert_eq!(table.first(&state("minecraft:stone")), Some(Handler::General));
    }

    #[test]
    fn the_cache_is_keyed_by_block_and_not_by_state() {
        let table = Table::java();
        let mut cache = Cache::new();
        let east = crate::placement::fakes::state_of(
            "minecraft:oak_stairs",
            &[("facing", "east"), ("half", "bottom")],
        );
        let west = crate::placement::fakes::state_of(
            "minecraft:oak_stairs",
            &[("facing", "west"), ("half", "bottom")],
        );
        assert_eq!(table.first_cached(&mut cache, &east), Some(Handler::General));
        assert_eq!(table.first_cached(&mut cache, &west), Some(Handler::General));
        assert_eq!(cache.len(), 1, "one entry per block, not per state");
        // Et le cache répond toujours la même chose que la table.
        for id in [
            kind::RACK,
            kind::SCARECROW,
            kind::GATE_IRON,
            kind::WATER,
            kind::JIGSAW,
            "minecraft:oak_door",
        ] {
            let probe = state(id);
            assert_eq!(table.first_cached(&mut cache, &probe), table.first(&probe), "{id}");
        }
    }

    #[test]
    fn a_table_without_its_general_row_refuses_instead_of_falling_back() {
        // Java journalise et retombe sur `new GeneralBlockPlacementHandler()`
        // (`:182-183`). Le port refuse : une table sans dernière ligne n'a pas de réponse.
        let table = Table::of(vec![Row::new(Handler::Rack, Rule::Family(kind::is_rack))]);
        let stone = state("minecraft:stone");
        assert_eq!(table.first(&stone), None);
        let verdict = table.answer(&crate::placement::Request::new([0, 0, 0], &stone));
        assert_eq!(
            verdict,
            Verdict::Refuses(crate::placement::Refusal::NoHandler {
                state: "minecraft:stone".to_owned()
            })
        );
    }
}
