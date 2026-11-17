//! Ce qu'un état de bloc *est*, pour la table — le `instanceof` de Java, en ids.
//!
//! Les 41 `canHandle` de Java ne lisent que `blockState.getBlock()` : pas une position, pas
//! un monde (vérifié ligne à ligne, et c'est ce qui rend supportable le cache par bloc de
//! `PlacementHandlers.java:153`). Le port n'a pas de classes : il a des identifiants, donc
//! chaque famille ci-dessous est une fonction de l'id — et chaque fonction cite le test
//! Java qu'elle remplace.
//!
//! **Ce qui est mesuré et ce qui est déduit.** Les identifiants `minecolonies:` et
//! `structurize:` viennent du Java (`ModBlocks`, `getRegistryName`) *et* du corpus : les
//! 9 515 `.nbt` de `data/minecolonies/structure/` ont été balayés, et les comptes cités
//! dans les commentaires sont ceux du corpus (`blockminecoloniesrack` 10 886 cellules,
//! `blockhutbarrackstower` 542, `blockwaypoint` 143, `blockminecoloniesnamedgrave` 415,
//! `gate_iron` 186, `gate_wood` 116, `blockhutfield` 216, `blocksubstitution` 8 960,
//! `blocksolidsubstitution` 7 278, `blockfluidsubstitution` 426). Les familles vanilla,
//! elles, ne peuvent pas être mesurées ainsi : leur liste est celle des blocs que le
//! corpus utilise, et `MANQUANT.md` §2 dit ce qu'un id absent coûte.

use crate::blueprint::BlockState;

/// `Blocks.AIR` — et rien d'autre : `AirPlacementHandler` teste `blockState.is(Blocks.AIR)`
/// (`PlacementHandlers.java:731`), donc `cave_air` et `void_air`, qui sont d'autres blocs,
/// tombent au handler général.
pub const AIR: &str = "minecraft:air";

/// `ModBlocks.blockSolidSubstitution` (`api/blocks/ModBlocks.java` du côté structurize) :
/// le placeholder « sol plein », 7 278 cellules du corpus.
pub const SOLID_SUBSTITUTION: &str = "structurize:blocksolidsubstitution";

/// `ModBlocks.blockSubstitution` : le placeholder « n'importe quoi », 8 960 cellules.
pub const SUBSTITUTION: &str = "structurize:blocksubstitution";

/// `ModBlocks.blockFluidSubstitution` : 426 cellules.
pub const FLUID_SUBSTITUTION: &str = "structurize:blockfluidsubstitution";

/// `ModBlocks.blockTagSubstitution` : un placeholder qui nomme un tag, 264 cellules.
pub const TAG_SUBSTITUTION: &str = "structurize:blocktagsubstitution";

/// Le rack de MineColonies (`core/blocks/BlockMinecoloniesRack.java:71`), 10 886 cellules.
pub const RACK: &str = "minecolonies:blockminecoloniesrack";

/// La tombe (`core/blocks/BlockMinecoloniesGrave.java:54`) : posée au runtime, jamais dans
/// un blueprint — le corpus n'en a aucune.
pub const GRAVE: &str = "minecolonies:blockminecoloniesgrave";

/// La tombe nommée (`core/blocks/BlockMinecoloniesNamedGrave.java:42`), 415 cellules.
pub const NAMED_GRAVE: &str = "minecolonies:blockminecoloniesnamedgrave";

/// Le waypoint du pathfinder (`core/blocks/schematic/BlockWaypoint.java:23`), 143 cellules.
pub const WAYPOINT: &str = "minecolonies:blockwaypoint";

/// L'épouvantail de champ (`core/blocks/BlockScarecrow.java:70` lisant
/// `AbstractBlockMinecoloniesDefault.REGISTRY_NAME`,
/// `api/blocks/huts/AbstractBlockMinecoloniesDefault.java:44`), 216 cellules.
pub const SCARECROW: &str = "minecolonies:blockhutfield";

/// Le champ de plantation (`core/blocks/BlockPlantationField.java:61`), 98 cellules. Son id
/// commence par `blockhut` et ce n'est **pas** une hutte.
pub const PLANTATION_FIELD: &str = "minecolonies:blockhutplantationfield";

/// La tour de caserne (`core/blocks/huts/BlockHutBarracksTower.java:32`), 542 cellules.
pub const BARRACKS_TOWER: &str = "minecolonies:blockhutbarrackstower";

/// La boîte aux lettres : une `AbstractBlockHut` dont l'id ne commence pas par `blockhut`
/// (`api/blocks/ModBlocks.java:170`).
pub const POSTBOX: &str = "minecolonies:blockpostbox";

/// Le stash : même cas (`api/blocks/ModBlocks.java:171`).
pub const STASH: &str = "minecolonies:blockstash";

/// La porte de fer de colonie (`api/blocks/decorative/AbstractBlockGate.java:45`), 186.
pub const GATE_IRON: &str = "minecolonies:gate_iron";

/// La porte de bois (`AbstractBlockGate.java:46`), 116.
pub const GATE_WOOD: &str = "minecolonies:gate_wood";

/// Une porte de Domum (`abstract class AbstractBlockDoor`), 3 828 cellules de `fancy_door`.
pub const DOMUM_FANCY_DOOR: &str = "domum_ornamentum:fancy_door";

/// La porte vanilla rétexturée de Domum, 1 734 cellules.
pub const DOMUM_VANILLA_DOOR: &str = "domum_ornamentum:vanilla_doors_compat";

/// Le pupitre (`LecternBlock`).
pub const LECTERN: &str = "minecraft:lectern";

/// Le jigsaw.
pub const JIGSAW: &str = "minecraft:jigsaw";

/// La ruche et le nid : les deux blocs que `BeehivePlacementHandler` réclame
/// (`BeehivePlacementHandler.java:33`).
pub const BEEHIVE: &str = "minecraft:beehive";

/// Le nid d'abeilles.
pub const BEE_NEST: &str = "minecraft:bee_nest";

/// Le bloc de structure (`StructureBlock`).
pub const STRUCTURE_BLOCK: &str = "minecraft:structure_block";

/// Le vide de structure (`StructureVoidBlock`).
pub const STRUCTURE_VOID: &str = "minecraft:structure_void";

/// L'eau, la lave et la colonne de bulles : les trois que `DimensionFluidHandler` réclame
/// (`DimensionFluidHandler.java:34`).
pub const WATER: &str = "minecraft:water";

/// La lave.
pub const LAVA: &str = "minecraft:lava";

/// La colonne de bulles.
pub const BUBBLE_COLUMN: &str = "minecraft:bubble_column";

/// Le nylium carmin et biscornu (`NyliumBlock`, `NetherrackPlacementHandler.java:25`).
pub const CRIMSON_NYLIUM: &str = "minecraft:crimson_nylium";

/// Le nylium biscornu.
pub const WARPED_NYLIUM: &str = "minecraft:warped_nylium";

/// Le tag que `BlackListedBlockPlacementHandler` teste
/// (`PlacementHandlers.java:1143`, `structurize`'s `ModTags.BLUEPRINT_BLACKLIST`).
pub const BLUEPRINT_BLACKLIST: &str = "structurize:blueprint_blacklist";

/// Une `AbstractBlockHut` ? — `HutPlacementHandler.java:37`.
///
/// Trois pièges, tous mesurés : `blockPostBox` et `blockStash` sont des huttes sans le
/// préfixe (`api/blocks/ModBlocks.java:170-171`), et deux ids **ont** le préfixe sans être
/// des huttes — l'épouvantail, qui hérite d'`AbstractBlockMinecoloniesDefault`
/// (`api/blocks/ModBlocks.java:90`), et le champ de plantation, qui hérite
/// d'`AbstractBlockMinecoloniesHorizontal` (`:91`). Un simple `starts_with("blockhut")`
/// poserait donc l'épouvantail comme une hutte et lui demanderait un
/// `TileEntityColonyBuilding` qu'il n'a pas.
pub fn is_hut(state: &BlockState) -> bool {
    match state.name.as_str() {
        POSTBOX | STASH => true,
        SCARECROW | PLANTATION_FIELD => false,
        other => other.starts_with("minecolonies:blockhut"),
    }
}

/// Une `AbstractBlockGate` ? — `GatePlacementHandler.java:17`.
///
/// **Une porte de colonie est aussi une `DoorBlock`** (`AbstractBlockGate.java:40`), donc
/// elle serait réclamée par la ligne `Door` de structurize si la ligne `Gate` n'arrivait pas
/// avant elle. C'est exactement ce que l'ordre de la table décide, et le test de la table le
/// prouve plutôt que de le supposer.
pub fn is_gate(state: &BlockState) -> bool {
    matches!(state.name.as_str(), GATE_IRON | GATE_WOOD)
}

/// L'épouvantail ? — `FieldPlacementHandler.java:31`.
pub fn is_scarecrow(state: &BlockState) -> bool {
    state.name == SCARECROW
}

/// Le rack ? — `RackPlacementHandler.java:31`.
pub fn is_rack(state: &BlockState) -> bool {
    state.name == RACK
}

/// La tombe ? — `GravePlacementHandler.java:32`.
pub fn is_grave(state: &BlockState) -> bool {
    state.name == GRAVE
}

/// La tombe nommée ? — `NamedGravePlacementHandler.java:25`.
pub fn is_named_grave(state: &BlockState) -> bool {
    state.name == NAMED_GRAVE
}

/// Le waypoint ? — `WayPointBlockPlacementHandler.java:29`.
pub fn is_waypoint(state: &BlockState) -> bool {
    state.name == WAYPOINT
}

/// Le pupitre ? — `LecternPlacementHandler.java:31`.
pub fn is_lectern(state: &BlockState) -> bool {
    state.name == LECTERN
}

/// Le jigsaw ? — `JigsawPlacementHandler.java:37`.
pub fn is_jigsaw(state: &BlockState) -> bool {
    state.name == JIGSAW
}

/// Une ruche ou un nid ? — `BeehivePlacementHandler.java:33`.
pub fn is_beehive(state: &BlockState) -> bool {
    matches!(state.name.as_str(), BEEHIVE | BEE_NEST)
}

/// Un bloc que le constructeur ignore ? — `BuilderIgnorePlacementHandler.java:33`.
///
/// `StructureVoidBlock` n'est pas une `StructureBlock` : les deux sont nommés, comme dans le
/// `||` de Java.
pub fn is_builder_ignored(state: &BlockState) -> bool {
    matches!(state.name.as_str(), STRUCTURE_BLOCK | STRUCTURE_VOID)
}

/// Un nylium ? — `NetherrackPlacementHandler.java:25`, `instanceof NyliumBlock`.
pub fn is_nylium(state: &BlockState) -> bool {
    matches!(state.name.as_str(), CRIMSON_NYLIUM | WARPED_NYLIUM)
}

/// Un bloc infesté ? — `InfestedBlocksPlacementHandler.java:30`, `instanceof InfestedBlock`.
pub fn is_infested(state: &BlockState) -> bool {
    state.name.starts_with("minecraft:infested_")
}

/// L'état hôte d'un bloc infesté : `InfestedBlock.hostStateByInfested`
/// (`InfestedBlocksPlacementHandler.java:78`), qui est le préfixe retiré —
/// `infested_deepslate` donne `deepslate`, `infested_stone_bricks` donne `stone_bricks`.
pub fn host_of_infested(state: &BlockState) -> Option<BlockState> {
    let path = state.name.strip_prefix("minecraft:infested_")?;
    Some(BlockState {
        name: format!("minecraft:{path}"),
        properties: state.properties.clone(),
    })
}

/// Un cuivre qui vieillit ? — `WeatheredCopperPlacementHandler.java:84`,
/// `instanceof WeatheringCopper`.
///
/// Trois exclusions que la classe fait et que l'id ne dit pas tout seul : le cuivre ciré
/// n'est pas un `WeatheringCopper` (`waxed_`), le minerai de cuivre non plus (`_ore`), et le
/// bloc de cuivre brut encore moins (`raw_`).
pub fn is_copper(state: &BlockState) -> bool {
    let id = state.name.as_str();
    id.contains("copper") && !id.contains("waxed") && !id.contains("_ore") && !id.contains("raw_")
}

/// L'âge d'un cuivre, `WeatheringCopper.WeatherState`, dans l'ordre du vieillissement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
// exhaustif : les quatre états de WeatherState
pub enum Age {
    /// `UNAFFECTED` : `copper_block`, `cut_copper`.
    Unaffected,
    /// `EXPOSED`.
    Exposed,
    /// `WEATHERED`.
    Weathered,
    /// `OXIDIZED`.
    Oxidized,
}

/// `WeatheringCopper.getAge()` (`WeatheredCopperPlacementHandler.java:67`), lu sur l'id.
pub fn age_of(state: &BlockState) -> Age {
    let id = state.name.as_str();
    if id.contains("oxidized") {
        Age::Oxidized
    } else if id.contains("weathered") {
        Age::Weathered
    } else if id.contains("exposed") {
        Age::Exposed
    } else {
        Age::Unaffected
    }
}

/// `WeatheringCopper.getFirst(block)` : le bloc de la **même famille** au premier âge
/// (`WeatheredCopperPlacementHandler.java:60`).
///
/// La comparaison de Java porte sur l'identité du bloc, pas sur son nom d'âge :
/// `getFirst(oxidized_cut_copper_stairs)` est `cut_copper_stairs`, et
/// `getFirst(oxidized_copper)` est `copper_block` — pas `copper`, qui n'existe pas.
pub fn first_of(state: &BlockState) -> String {
    let id = state.name.as_str();
    for prefix in ["minecraft:oxidized_", "minecraft:weathered_", "minecraft:exposed_"] {
        if let Some(rest) = id.strip_prefix(prefix) {
            return if rest == "copper" {
                "minecraft:copper_block".to_owned()
            } else {
                format!("minecraft:{rest}")
            };
        }
    }
    if id == "minecraft:copper" {
        "minecraft:copper_block".to_owned()
    } else {
        id.to_owned()
    }
}

/// L'âge du monde atteint-il celui du blueprint en vieillissant ?
///
/// C'est la boucle de `getExpectedBlockState` (`WeatheredCopperPlacementHandler.java:65-78`) :
/// partir du bloc du blueprint et appeler `getNext` jusqu'à l'âge du monde. Si on l'atteint,
/// le monde est **plus** vieux et Java garde le monde (`return null`, `:78`) ; si la chaîne
/// se termine avant, le monde est plus jeune et Java pose le blueprint. Or la chaîne est
/// totale et ordonnée, donc `getNext` jusqu'à `to` réussit exactement quand `to >= from`.
pub fn weathers_to(from: Age, to: Age) -> bool {
    to >= from
}

/// Une porte vanilla ? — `PlacementHandlers.java:460`, `instanceof DoorBlock`.
///
/// Toutes les portes vanilla finissent par `_door` et aucune autre ne le fait ; les portes
/// de colonie, qui sont des `DoorBlock` d'une autre classe, sont en `minecolonies:` et
/// capturées plus haut par `is_gate`.
pub fn is_vanilla_door(state: &BlockState) -> bool {
    state.namespace() == "minecraft" && state.name.ends_with("_door")
}

/// Un bloc de Domum Ornamentum ? — `DoBlockPlacementHandler.java:52`,
/// `instanceof IMateriallyTexturedBlock`.
pub fn is_domum(state: &BlockState) -> bool {
    state.namespace() == "domum_ornamentum"
}

/// Une porte de Domum ? — `DoDoorBlockPlacementHandler.java:39`,
/// `IMateriallyTexturedBlock && AbstractBlockDoor`.
///
/// Une trappe de Domum n'est **pas** une porte (`TrapdoorBlock` n'étend pas
/// `AbstractBlockDoor`), donc elle reste au handler DO des blocs.
pub fn is_domum_door(state: &BlockState) -> bool {
    matches!(state.name.as_str(), DOMUM_FANCY_DOOR | DOMUM_VANILLA_DOOR)
}

/// Un fluide ? — `DimensionFluidHandler.java:34`, `LiquidBlock || BubbleColumnBlock`.
pub fn is_fluid(state: &BlockState) -> bool {
    matches!(state.name.as_str(), WATER | LAVA | BUBBLE_COLUMN)
}

/// Le fluide est-il une **source** ? — `BlockState.getFluidState().isSource()`
/// (`DimensionFluidHandler.java:60`, `:77`).
///
/// L'eau et la lave portent `level` : 0 est la source, 1 à 7 un fluide qui coule, et
/// `falling=true` une chute. Une colonne de bulles est toujours une source.
pub fn is_fluid_source(state: &BlockState) -> bool {
    match state.name.as_str() {
        WATER | LAVA => {
            state.property("level").is_none_or(|level| level == "0")
                && state.property("falling") != Some("true")
        }
        BUBBLE_COLUMN => true,
        _ => false,
    }
}

/// De l'herbe ou de la terre ? — `PlacementHandlers.java:411`.
pub fn is_grass(state: &BlockState) -> bool {
    matches!(state.name.as_str(), "minecraft:grass_block" | "minecraft:dirt")
}

/// Un lit ? — `PlacementHandlers.java:533`.
pub fn is_bed(state: &BlockState) -> bool {
    state.name.starts_with("minecraft:") && state.name.ends_with("_bed")
}

/// Une bannière **debout** ? — `PlacementHandlers.java:987`, `instanceof BannerBlock`.
///
/// `BannerBlock` ne couvre pas les bannières murales (`WallBannerBlock` est une sœur, pas une
/// fille) et ne couvre pas non plus les drapeaux de colonie, qui étendent
/// `AbstractBannerBlock` directement (`api/blocks/decorative/AbstractColonyFlagBanner.java:34`)
/// : un drapeau de colonie part donc au handler général, comme en Java.
pub fn is_banner(state: &BlockState) -> bool {
    state.name.starts_with("minecraft:")
        && state.name.ends_with("_banner")
        && !state.name.ends_with("_wall_banner")
}

/// Une plante double ? — `PlacementHandlers.java:585`, `instanceof DoublePlantBlock`.
pub fn is_double_plant(state: &BlockState) -> bool {
    matches!(
        state.name.as_str(),
        "minecraft:tall_grass"
            | "minecraft:large_fern"
            | "minecraft:sunflower"
            | "minecraft:lilac"
            | "minecraft:rose_bush"
            | "minecraft:peony"
    )
}

/// Un bloc que structurize refuse de reposer ? — `PlacementHandlers.java:632-634`
/// (`EndPortalBlock`, `SpawnerBlock`, `DragonEggBlock`).
pub fn is_special(state: &BlockState) -> bool {
    matches!(
        state.name.as_str(),
        "minecraft:end_portal" | "minecraft:spawner" | "minecraft:dragon_egg"
    )
}

/// Un pot de fleurs ? — `PlacementHandlers.java:675`, `instanceof FlowerPotBlock`, qui
/// couvre le pot vide et tous les `potted_*`.
pub fn is_flower_pot(state: &BlockState) -> bool {
    state.name == "minecraft:flower_pot" || state.name.starts_with("minecraft:potted_")
}

/// Un bloc qui tombe ? — `PlacementHandlers.java:336`, `FallingBlock || Fallable`.
///
/// La classe ne se lit pas dans un id : la liste est celle des blocs qui tombent et que le
/// corpus emploie, et `MANQUANT.md` §2 dit ce qu'un oubli coûte (le bloc part au général et
/// tombe au premier tick, ce que Java évite en posant un support d'abord).
pub fn is_falling(state: &BlockState) -> bool {
    let id = state.name.as_str();
    matches!(
        id,
        "minecraft:sand"
            | "minecraft:red_sand"
            | "minecraft:gravel"
            | "minecraft:suspicious_sand"
            | "minecraft:suspicious_gravel"
            | "minecraft:anvil"
            | "minecraft:chipped_anvil"
            | "minecraft:damaged_anvil"
    ) || id.ends_with("_concrete_powder")
}

/// Un bloc à entité ? — `PlacementHandlers.java:893`, `instanceof BaseEntityBlock`.
///
/// Encore une classe, remplacée par la liste des blocs à entité que le corpus porte. Un
/// oubli n'est pas grave : le bloc tombe au handler général, qui écrit aussi la donnée
/// (`GeneralBlockPlacementHandler.java:80-91`) ; ce que le conteneur ajoute, c'est de refuser
/// d'écrire quand la donnée de l'inventaire ne se charge pas (`:904-917`).
pub fn is_container(state: &BlockState) -> bool {
    let id = state.name.as_str();
    matches!(
        id,
        "minecraft:chest"
            | "minecraft:trapped_chest"
            | "minecraft:barrel"
            | "minecraft:furnace"
            | "minecraft:blast_furnace"
            | "minecraft:smoker"
            | "minecraft:dispenser"
            | "minecraft:dropper"
            | "minecraft:brewing_stand"
            | "minecraft:beacon"
            | "minecraft:jukebox"
            | "minecraft:ender_chest"
            | "minecraft:enchanting_table"
            | "minecraft:chiseled_bookshelf"
            | "minecraft:decorated_pot"
            | "minecraft:campfire"
            | "minecraft:soul_campfire"
    ) || id.ends_with("_shulker_box")
        || id.ends_with("_sign")
        || id.ends_with("_hanging_sign")
}

/// Le chemin de terre ? — `PlacementHandlers.java:786`.
pub fn is_dirt_path(state: &BlockState) -> bool {
    state.name == "minecraft:dirt_path"
}

/// Le feu ? — `PlacementHandlers.java:293`, `blockState.is(Blocks.FIRE)`.
pub fn is_fire(state: &BlockState) -> bool {
    state.name == "minecraft:fire"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placement::fakes::{state, state_of};

    #[test]
    fn a_scarecrow_is_not_a_hut_whatever_its_id_says() {
        // `blockhutfield` hérite d'`AbstractBlockMinecoloniesDefault`
        // (`api/blocks/ModBlocks.java:90`) : le préfixe ment, la liste corrige.
        assert!(!is_hut(&state(SCARECROW)));
        assert!(!is_hut(&state(PLANTATION_FIELD)));
        assert!(is_scarecrow(&state(SCARECROW)));
        // Les deux huttes sans préfixe (`ModBlocks.java:170-171`).
        assert!(is_hut(&state(POSTBOX)));
        assert!(is_hut(&state(STASH)));
        assert!(is_hut(&state(BARRACKS_TOWER)));
        assert!(is_hut(&state("minecolonies:blockhutbuilder")));
    }

    #[test]
    fn a_gate_is_also_a_vanilla_door() {
        // `AbstractBlockGate extends DoorBlock` (`AbstractBlockGate.java:40`).
        assert!(is_gate(&state(GATE_IRON)));
        assert!(!is_vanilla_door(&state(GATE_IRON)), "another namespace");
        assert!(is_vanilla_door(&state("minecraft:oak_door")));
        assert!(!is_vanilla_door(&state("minecraft:oak_trapdoor")));
        assert!(!is_vanilla_door(&state("minecraft:dark_oak_fence_gate")));
    }

    #[test]
    fn copper_is_age_before_it_is_a_block() {
        assert!(is_copper(&state("minecraft:oxidized_cut_copper_stairs")));
        assert!(!is_copper(&state("minecraft:waxed_oxidized_copper")));
        assert!(!is_copper(&state("minecraft:copper_ore")));
        assert!(!is_copper(&state("minecraft:raw_copper_block")));
        assert_eq!(age_of(&state("minecraft:exposed_copper")), Age::Exposed);
        assert_eq!(age_of(&state("minecraft:cut_copper")), Age::Unaffected);
    }

    #[test]
    fn the_first_copper_of_a_family_is_the_unaffected_one() {
        // `getFirst(oxidized_copper)` est `copper_block`, pas `copper`.
        assert_eq!(first_of(&state("minecraft:oxidized_copper")), "minecraft:copper_block");
        assert_eq!(
            first_of(&state("minecraft:oxidized_cut_copper_stairs")),
            "minecraft:cut_copper_stairs"
        );
        assert_eq!(first_of(&state("minecraft:cut_copper")), "minecraft:cut_copper");
        // Deux blocs de la même famille, et deux familles différentes.
        assert_eq!(
            first_of(&state("minecraft:weathered_copper")),
            first_of(&state("minecraft:copper_block"))
        );
        assert_ne!(
            first_of(&state("minecraft:weathered_copper")),
            first_of(&state("minecraft:weathered_cut_copper"))
        );
    }

    #[test]
    fn get_next_stops_at_the_older_age_only() {
        // La boucle de `:65-78` atteint `to` depuis `from` exactement quand `to >= from`.
        assert!(weathers_to(Age::Unaffected, Age::Oxidized));
        assert!(weathers_to(Age::Exposed, Age::Exposed));
        assert!(!weathers_to(Age::Weathered, Age::Exposed));
    }

    #[test]
    fn a_fluid_is_a_source_only_at_level_zero() {
        assert!(is_fluid_source(&state(WATER)));
        assert!(is_fluid_source(&state_of(WATER, &[("level", "0")])));
        assert!(!is_fluid_source(&state_of(WATER, &[("level", "3")])));
        assert!(!is_fluid_source(&state_of(WATER, &[("level", "0"), ("falling", "true")])));
        assert!(is_fluid_source(&state(BUBBLE_COLUMN)));
        assert!(!is_fluid_source(&state("minecraft:stone")));
    }

    #[test]
    fn a_banner_is_the_standing_one_only() {
        assert!(is_banner(&state("minecraft:white_banner")));
        assert!(!is_banner(&state("minecraft:white_wall_banner")));
        // Un drapeau de colonie étend `AbstractBannerBlock`, pas `BannerBlock`.
        assert!(!is_banner(&state("minecolonies:colony_banner")));
        assert!(!is_banner(&state("minecolonies:colony_wall_banner")));
    }

    #[test]
    fn the_infested_host_is_the_id_without_the_prefix() {
        assert_eq!(
            host_of_infested(&state("minecraft:infested_deepslate")),
            Some(state("minecraft:deepslate"))
        );
        assert_eq!(
            host_of_infested(&state("minecraft:infested_stone_bricks")),
            Some(state("minecraft:stone_bricks"))
        );
        assert_eq!(host_of_infested(&state("minecraft:stone")), None);
    }
}
