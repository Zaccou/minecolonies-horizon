//! Ce que chaque handler fait d'une cellule — un bras par classe Java.
//!
//! Chaque fonction citée ci-dessous est celle que Java appelle depuis `handle(...)` de la
//! classe nommée. Le port garde deux choses d'un handler : **ce qu'il écrit** (l'état, et si
//! la donnée du blueprint suit) et **ce qu'il ne fait pas** (passer, ne rien poser, refuser).
//! Ce qu'il *demande* au monde est réduit à ce que `Request` porte : pas un appel, jamais.
//!
//! Trois règles de lecture, valables pour tout le fichier :
//!
//! 1. **Java répond `DENY` quand l'écriture échoue** (`!world.setBlock(...)`,
//!    `!WorldUtil.setBlockState(...)`). Le port ne le fait pas ici : l'échec d'écriture est
//!    un refus de [`crate::colony_world::refusal`], posé par `Intent::plan` avant que quoi
//!    que ce soit ne parte au moteur. Un `Refuses` de ce fichier est donc un refus *du
//!    handler* — il n'y en a que trois, et chacun est nommé à son endroit : une hutte sans
//!    blueprint, une propriété que l'état ne porte pas, un `final_state` illisible.
//! 2. **`PASS` n'est pas « rien à faire ».** C'est le plus souvent « la cellule tient déjà la
//!    réponse », et le port le dit par [`Verdict::Passes`].
//! 3. **`Follow::Data` est une décision, pas un détail.** C'est ce qui fait qu'un rack garde
//!    son inventaire (`Cell::change` → `Change::Keep`) et qu'un bloc ordinaire posé sur un
//!    coffre le remplace franchement.

use std::collections::BTreeMap;

use crate::blueprint::{nbt, BlockState};

use super::kind;
use super::table::Handler;
use super::{Cell, Follow, Refusal, Request, Verdict, Why};

/// Ce que le handler `handler` fait de la cellule que `request` décrit.
///
/// La table a déjà répondu « ce handler-là » ; cette fonction ne re-teste jamais
/// `canHandle`, comme Java ne le re-teste pas après `getHandler`
/// (`PlacementHandlers.java:175-179`).
pub fn decide(handler: Handler, request: &Request<'_>) -> Verdict {
    match handler {
        Handler::WeatheredCopper => weathered_copper(request),
        Handler::InfestedBlocks => infested(request),
        Handler::Hut => hut(request),
        Handler::Lectern => one(request.at, request.state.clone(), Follow::of(request.data)),
        Handler::Netherrack => one(request.at, request.state.clone(), Follow::None),
        Handler::Gate => general(request),
        Handler::WayPoint => waypoint(request),
        Handler::NamedGrave => {
            if request.mode.complete() {
                one(request.at, request.state.clone(), Follow::None)
            } else {
                Verdict::Spares(Why::SketchOnly)
            }
        }
        Handler::Grave => grave(request),
        Handler::Rack => one(request.at, request.state.clone(), Follow::of(request.data)),
        Handler::DimensionFluid => dimension_fluid(request),
        Handler::Field => double(request, false),
        Handler::BarracksTower => one(request.at, request.state.clone(), Follow::of(request.data)),
        Handler::DoDoor => double(request, true),
        Handler::DoBlock => one(request.at, request.state.clone(), Follow::of(request.data)),
        Handler::BuilderIgnore => {
            if request.mode.complete() {
                one(request.at, request.state.clone(), Follow::of(request.data))
            } else {
                Verdict::Spares(Why::SketchOnly)
            }
        }
        Handler::Jigsaw => jigsaw(request),
        Handler::Beehive => beehive(request),
        Handler::General => general(request),
        // Les 22 lignes de structurize : nommées, pas décidées. `MANQUANT.md` §3.
        Handler::Air
        | Handler::SolidSubstitution
        | Handler::Substitution
        | Handler::TagSubstitution
        | Handler::BlackListed
        | Handler::FluidSubstitution
        | Handler::Fire
        | Handler::DirtPath
        | Handler::Grass
        | Handler::Door
        | Handler::Bed
        | Handler::DoublePlant
        | Handler::SpecialBlock
        | Handler::FlowerPot
        | Handler::Hopper
        | Handler::DripStone
        | Handler::FallingBlock
        | Handler::Banner
        | Handler::StructurizeDoBlock
        | Handler::StructurizeDoDoor
        | Handler::Container
        | Handler::StructurizeGeneral => Verdict::Delegated(handler),
    }
}

/// Une cellule, un état, la donnée qui suit.
fn one(at: [i32; 3], state: BlockState, follow: Follow) -> Verdict {
    Verdict::Writes(vec![Cell { at, state, follow }])
}

/// L'état du monde est-il déjà celui que le blueprint demande ?
///
/// C'est le `world.getBlockState(pos).equals(state)` de Java, avec la nuance que le port ne
/// peut pas lire le monde : un appelant qui n'a rien lu répond `false`, donc « pose », ce
/// qui est ce que Java fait devant de l'air.
fn same(world: Option<&BlockState>, wanted: &BlockState) -> bool {
    world == Some(wanted)
}

/// `GeneralBlockPlacementHandler.java:44-94` — le dernier recours, et le comportement des
/// portes de colonie (`GatePlacementHandler` en hérite, `GatePlacementHandler.java:12`).
///
/// Il accepte **tout** ; il ne retire pas le bloc en place (contrairement à celui de
/// structurize, `PlacementHandlers.java:848-852`), il compare d'abord (`:70-73`) et il ne
/// rappelle `setPlacedBy` que s'il y a une donnée à poser (`:80-91`).
///
/// **Ce qu'il ne fait pas :** re-dériver les connexions. `:52-68` redemande un
/// `getStateForPlacement` pour les murs, barrières, barreaux et piliers, afin qu'une clôture
/// collée à une clôture se connecte ; le port écrit l'état du blueprint tel quel
/// (`MANQUANT.md` §1).
fn general(request: &Request<'_>) -> Verdict {
    if same(request.here.state, request.state) {
        return Verdict::Passes;
    }
    one(request.at, request.state.clone(), Follow::of(request.data))
}

/// `WeatheredCopperPlacementHandler.java:82-107` — le cuivre ne rajeunit pas.
///
/// `getExpectedBlockState` (`:38-79`) répond trois choses : l'état du blueprint (on pose),
/// `null` (on garde ce que le monde tient), ou l'état du blueprint encore. Le `null` est le
/// cas intéressant : il ne veut pas dire « ne rien faire parce qu'on ne sait pas », il veut
/// dire « **le cuivre en place est plus vieux** que celui du blueprint, et le
/// vieillissement ne se défait pas » (`:65-78`, `return null`).
fn weathered_copper(request: &Request<'_>) -> Verdict {
    // Pas d'aperçu ? Pas de comparaison : on pose ce que le blueprint dit (`:46-51`).
    if request.mode.complete() {
        return one(request.at, request.state.clone(), Follow::None);
    }
    // Rien n'a été lu : Java lirait de l'air, et l'air n'est pas du cuivre (`:52-56`).
    let Some(world) = request.here.state else {
        return one(request.at, request.state.clone(), Follow::None);
    };
    // Pas du cuivre en place, ou une autre famille de cuivre : on pose (`:52-63`).
    if !kind::is_copper(world)
        || !kind::is_copper(request.state)
        || kind::first_of(world) != kind::first_of(request.state)
    {
        return one(request.at, request.state.clone(), Follow::None);
    }
    // Le monde est-il déjà au moins aussi vieux ? Alors on le garde (`:65-78`, `:96-99`).
    if kind::weathers_to(kind::age_of(request.state), kind::age_of(world)) {
        return Verdict::Passes;
    }
    one(request.at, request.state.clone(), Follow::None)
}

/// `InfestedBlocksPlacementHandler.java:28-58` — l'infesté ne se construit pas.
///
/// En aperçu on colle l'infesté tel quel ; en construction on pose son **hôte**
/// (`hostStateByInfested`, `:70-79`), parce qu'un constructeur ne dispose pas de pierre
/// infestée. Le handler n'écrit jamais de donnée.
fn infested(request: &Request<'_>) -> Verdict {
    let expected = if request.mode.complete() {
        request.state.clone()
    } else {
        match kind::host_of_infested(request.state) {
            Some(host) => host,
            // `getExpectedBlockState` ne répond `null` que si le bloc n'est pas infesté,
            // ce que la table a déjà écarté : Java répondrait `PASS` (`:42-46`).
            None => return Verdict::Passes,
        }
    };
    if same(request.here.state, &expected) {
        return Verdict::Passes;
    }
    one(request.at, expected, Follow::None)
}

/// `HutPlacementHandler.java:40-114` — le bloc d'un bâtiment, et sa donnée.
///
/// Trois choses, dans cet ordre : si la cellule tient déjà l'état, `PASS` (`:48-51`) ; sans
/// blueprint, `DENY` (`:53-56`) ; sinon on écrit, et la donnée du blueprint suit — c'est elle
/// qui porte le `blueprintDataProvider` d'une hutte (ses coins, son pack, son chemin).
///
/// **Ce que ce handler fait de plus que poser un bloc n'est pas ici.** `:68-99` écrit dans
/// la donnée du bloc le chemin du blueprint : le chemin complet pour la cellule d'ancrage,
/// le fichier de niveau 1 pour une hutte secondaire, et le nom de pack pour toutes. C'est le
/// **dossier d'un bâtiment** — `crate::building` le tient — et le port lui en donne les deux
/// faits dont il a besoin : `Request::placing` (le chemin est `Placing::id`) et `at`
/// comparé à `Placing::at` (l'ancrage est la cellule égale). Rien n'est réimplémenté ici.
fn hut(request: &Request<'_>) -> Verdict {
    if same(request.here.state, request.state) {
        return Verdict::Passes;
    }
    if request.placing.is_none() {
        return Verdict::Refuses(Refusal::NoBlueprint { at: request.at });
    }
    one(request.at, request.state.clone(), Follow::of(request.data))
}

/// `WayPointBlockPlacementHandler.java:32-54` — le waypoint n'est pas un bloc, c'est un point.
///
/// L'ordre de Java compte : `world.removeBlock(pos, false)` passe **avant** la question de la
/// colonie (`:40`), donc la cellule finit vide même quand il n'y a pas de colonie. Dans une
/// colonie, en construction, la position est enregistrée dans la colonie et rien n'est
/// reposé (`:44-47`) ; dans une colonie en aperçu, le bloc est reposé (`:48-51`).
fn waypoint(request: &Request<'_>) -> Verdict {
    if request.in_colony && !request.mode.complete() {
        return Verdict::Clears;
    }
    if request.in_colony {
        return one(request.at, request.state.clone(), Follow::None);
    }
    Verdict::Clears
}

/// `GravePlacementHandler.java:36-61` — une tombe debout n'est jamais réécrite.
///
/// C'est la seule règle du dossier qui protège un contenu : `:43-46` répond `SUCCESS` sans
/// rien toucher dès qu'une tombe occupe la cellule, donc un citoyen mort ne perd jamais son
/// inventaire parce qu'un blueprint a été recollé par-dessus.
fn grave(request: &Request<'_>) -> Verdict {
    if request.here.state.is_some_and(kind::is_grave) {
        return Verdict::Spares(Why::Kept);
    }
    one(request.at, request.state.clone(), Follow::of(request.data))
}

/// `DimensionFluidHandler.java:31-84` — l'eau est gratuite, la lave du Nether aussi.
///
/// En construction, un fluide qui n'est pas une source est laissé au monde (`:77-80`) : le
/// moteur remplira la cellule tout seul, et un bloc d'eau qui coule posé à la main ne
/// s'écoule pas. L'aperçu, lui, écrit tout (`:81`).
///
/// **Ce que le port perd :** `:82` programme un tick de fluide
/// (`scheduleTick(..., getTickDelay(world))`). Aucune méthode de l'API ne programme un tick
/// — `MANQUANT.md` §1 — donc l'eau posée reste immobile jusqu'à ce qu'un voisin la réveille.
fn dimension_fluid(request: &Request<'_>) -> Verdict {
    if !request.mode.complete() && !kind::is_fluid_source(request.state) {
        return Verdict::Passes;
    }
    one(request.at, request.state.clone(), Follow::None)
}

/// Un bloc double : `DoDoorBlockPlacementHandler.java:43-75` et
/// `FieldPlacementHandler.java:34-62`.
///
/// Les deux écrivent **les deux cellules** depuis la moitié basse : la basse prend son état,
/// la haute le même état avec `half=upper` (`:58-59`, `:44-45`). La moitié haute, quand le
/// collège y arrive, n'a donc plus rien à faire (`:73-74`, `Spares`) — sinon la cellule
/// serait nommée deux fois et `Intent::plan` refuserait `ConflictingWrite`.
///
/// Deux différences entre les deux classes, toutes deux de Java : la porte de Domum retire
/// d'abord une porte identique (`:52-56`, une entité de bloc à reconstruire), l'épouvantail
/// non ; et la porte écrit ses deux moitiés avec `world.setBlock`, l'épouvantail avec un
/// `setBlock` de drapeau 3. Le port n'a ni l'un ni l'autre : `colony_world` écrit.
///
/// La troisième différence est `twin` : une porte de Domum applique **la même** donnée de
/// bloc aux deux moitiés (`:61-67`), l'épouvantail à la seule cellule courante (`:48-59`).
fn double(request: &Request<'_>, twin: bool) -> Verdict {
    let Some(half) = request.state.property("half") else {
        // Java lèverait une exception au milieu du collage
        // (`DoDoorBlockPlacementHandler.java:50`).
        return Verdict::Refuses(Refusal::MissingProperty {
            state: request.state.to_string(),
            property: "half",
        });
    };
    if half == "upper" {
        return Verdict::Spares(Why::AlreadyPaired);
    }
    let mut upper = request.state.clone();
    upper.properties.insert("half".to_owned(), "upper".to_owned());
    Verdict::Writes(vec![
        Cell {
            at: request.at,
            state: request.state.clone(),
            follow: Follow::of(request.data),
        },
        Cell {
            at: [request.at[0], request.at[1] + 1, request.at[2]],
            state: upper,
            follow: if twin { Follow::of(request.data) } else { Follow::None },
        },
    ])
}

/// `JigsawPlacementHandler.java:34-95` — le jigsaw porte sa propre réponse.
///
/// En aperçu on colle le jigsaw (`:48-64`). En construction, la donnée du bloc porte un
/// `final_state` — la chaîne d'état que le bloc doit devenir — et c'est **elle** qu'on pose
/// (`:66-92`). `structure_void` veut dire « rien du tout » (`:86-89`), un jigsaw sans
/// `final_state` aussi (`:66`, Java ne rentre pas dans le `if`).
///
/// **Et le défaut de Java, que le port refuse.** `:70-84` : une chaîne qui ne se lit pas
/// laisse `finalState` à `Blocks.AIR`, un `catch` écrit une ligne de journal, et `:91` écrit
/// cet air dans le monde. Une faute de frappe dans une donnée de structure **efface** donc la
/// cellule, sans le dire. Le port répond [`Refusal::BadFinalState`] et n'écrit rien.
fn jigsaw(request: &Request<'_>) -> Verdict {
    if request.mode.complete() {
        return one(request.at, request.state.clone(), Follow::of(request.data));
    }
    let Some(data) = request.data else {
        return Verdict::Spares(Why::NoFinalState);
    };
    let Some(text) = nbt::tag(&data.data, "final_state").and_then(nbt::as_text) else {
        return Verdict::Spares(Why::NoFinalState);
    };
    let final_state = match parse_state(&text) {
        Ok(state) => state,
        Err(refusal) => return Verdict::Refuses(refusal),
    };
    if final_state.name == kind::STRUCTURE_VOID {
        return Verdict::Spares(Why::VoidState);
    }
    one(request.at, final_state, Follow::None)
}

/// `BeehivePlacementHandler.java:36-46` — la ruche perd son miel.
///
/// Une seule ligne, et elle jette : `defaultBlockState().setValue(FACING, ...)`. Tout le
/// reste de l'état repart à sa valeur par défaut — `honey_level` à 0 — et la donnée du
/// blueprint n'est jamais lue, donc les abeilles non plus. C'est voulu : une ruche de
/// blueprint ne doit pas arriver pleine.
fn beehive(request: &Request<'_>) -> Verdict {
    let Some(facing) = request.state.property("facing") else {
        return Verdict::Refuses(Refusal::MissingProperty {
            state: request.state.to_string(),
            property: "facing",
        });
    };
    let mut properties = BTreeMap::new();
    properties.insert("facing".to_owned(), facing.to_owned());
    let state = BlockState {
        name: request.state.name.clone(),
        properties,
    };
    one(request.at, state, Follow::None)
}

/// `BlockStateParser.parseForBlock` (`JigsawPlacementHandler.java:74`) : la forme
/// `minecraft:oak_stairs[facing=north,half=bottom]`, sans le tag.
///
/// Le lecteur de Java est **indulgent** (`false` en troisième argument, `:74`) : une
/// propriété absente est acceptée et complétée par la valeur par défaut du bloc. Le port ne
/// connaît pas les valeurs par défaut (pas de registre), donc il garde exactement les
/// propriétés écrites et laisse le moteur compléter — ce que `block_state_id` fait d'un état
/// partiel (`MANQUANT.md` §1).
///
/// Refuse : une chaîne vide, une référence `#tag`, un crochet fermant manquant, une paire
/// sans `=`, une clé ou une valeur vide.
pub fn parse_state(text: &str) -> Result<BlockState, Refusal> {
    let bad = || Refusal::BadFinalState {
        state: text.to_owned(),
    };
    let text = text.trim();
    if text.is_empty() || text.starts_with('#') {
        return Err(bad());
    }
    let (name, rest) = match text.split_once('[') {
        None => (text, None),
        Some((name, rest)) => match rest.strip_suffix(']') {
            Some(rest) => (name, Some(rest)),
            None => return Err(bad()),
        },
    };
    let name = name.trim();
    if name.is_empty() || name.contains(char::is_whitespace) || name.contains(']') {
        return Err(bad());
    }
    let name = if name.contains(':') {
        name.to_owned()
    } else {
        // `parseForBlock` résout un nom sans espace de noms dans `minecraft`.
        format!("minecraft:{name}")
    };
    let mut properties = BTreeMap::new();
    for pair in rest.unwrap_or("").split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let Some((key, value)) = pair.split_once('=') else {
            return Err(bad());
        };
        let (key, value) = (key.trim(), value.trim());
        let bracket = |c: char| c == '[' || c == ']';
        if key.is_empty() || value.is_empty() || key.contains(bracket) || value.contains(bracket) {
            return Err(bad());
        }
        properties.insert(key.to_owned(), value.to_owned());
    }
    Ok(BlockState { name, properties })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placement::fakes::{entity, jigsaw, state, state_of};
    use crate::placement::{Here, Mode};

    /// La première cellule écrite, ou une panique qui dit ce qui est arrivé.
    fn first(verdict: &Verdict) -> &Cell {
        match verdict {
            Verdict::Writes(cells) => cells.first().expect("a write with no cell"),
            other => panic!("expected a write, got {other:?}"),
        }
    }

    /// L'état écrit dans la première cellule.
    fn written(verdict: &Verdict) -> BlockState {
        first(verdict).state.clone()
    }

    #[test]
    fn the_general_handler_passes_when_the_world_already_matches() {
        let wanted = state("minecraft:stone");
        let request = Request::new([1, 70, 2], &wanted).with_world(&wanted, crate::colony_world::guard::Occupant::Solid);
        assert_eq!(decide(Handler::General, &request), Verdict::Passes);
    }

    #[test]
    fn the_general_handler_writes_anything_else_with_the_data_that_follows() {
        let wanted = state("minecraft:stone");
        let data = entity("minecraft:chest");
        let request = Request::new([1, 70, 2], &wanted).with_data(&data);
        assert_eq!(written(&decide(Handler::General, &request)), wanted);
        assert_eq!(first(&decide(Handler::General, &request)).follow, Follow::Data);
    }

    #[test]
    fn a_gate_behaves_like_the_general_handler_it_extends() {
        let gate = state(kind::GATE_WOOD);
        let verdict = decide(Handler::Gate, &Request::new([0, 70, 0], &gate));
        assert_eq!(written(&verdict), gate);
        assert_eq!(first(&verdict).at, [0, 70, 0]);
    }

    #[test]
    fn the_copper_keeps_the_world_block_that_is_already_older() {
        // Une `weathered_copper` en place, une `exposed_copper` dans le blueprint : le monde
        // est plus vieux, Java garde le monde (`:78`).
        let world = state("minecraft:weathered_copper");
        let wanted = state("minecraft:exposed_copper");
        let request = Request::new([0, 70, 0], &wanted).with_world(&world, crate::colony_world::guard::Occupant::Solid);
        assert_eq!(decide(Handler::WeatheredCopper, &request), Verdict::Passes);
    }

    #[test]
    fn the_copper_places_the_blueprint_state_when_the_world_is_younger() {
        let world = state("minecraft:exposed_copper");
        let wanted = state("minecraft:weathered_copper");
        let request = Request::new([0, 70, 0], &wanted).with_world(&world, crate::colony_world::guard::Occupant::Solid);
        assert_eq!(written(&decide(Handler::WeatheredCopper, &request)), wanted);
    }

    #[test]
    fn the_copper_of_another_family_is_always_placed() {
        // `cut_copper` en place, `weathered_copper` demandé : deux familles, on pose
        // (`:58-63`).
        let world = state("minecraft:cut_copper");
        let wanted = state("minecraft:weathered_copper");
        let request = Request::new([0, 70, 0], &wanted).with_world(&world, crate::colony_world::guard::Occupant::Solid);
        assert_eq!(written(&decide(Handler::WeatheredCopper, &request)), wanted);
    }

    #[test]
    fn the_infested_block_becomes_its_host_when_building() {
        let wanted = state("minecraft:infested_stone");
        assert_eq!(
            written(&decide(Handler::InfestedBlocks, &Request::new([0, 70, 0], &wanted))),
            state("minecraft:stone")
        );
    }

    #[test]
    fn the_infested_block_is_copied_as_it_is_in_a_sketch() {
        let wanted = state("minecraft:infested_stone");
        let request = Request::new([0, 70, 0], &wanted).sketch();
        assert_eq!(written(&decide(Handler::InfestedBlocks, &request)), wanted);
    }

    #[test]
    fn a_hut_without_a_blueprint_is_refused() {
        let hut = state(kind::BARRACKS_TOWER);
        assert_eq!(
            decide(Handler::Hut, &Request::new([4, 70, 4], &hut)),
            Verdict::Refuses(Refusal::NoBlueprint { at: [4, 70, 4] })
        );
    }

    #[test]
    fn a_hut_with_a_blueprint_writes_its_state_and_the_data_that_follows() {
        let hut = state(kind::BARRACKS_TOWER);
        let data = entity("minecolonies:colonybuilding");
        let placing = crate::structures::Placing::new("minecolonies:original/fundamentals/builder1", crate::ai::Pos::new(4, 70, 4));
        let request = Request::new([4, 70, 4], &hut)
            .with_data(&data)
            .with_placing(&placing);
        let verdict = decide(Handler::Hut, &request);
        assert_eq!(written(&verdict), hut);
        assert_eq!(first(&verdict).follow, Follow::Data);
    }

    #[test]
    fn the_lectern_keeps_the_book_its_data_carries() {
        let lectern = state(kind::LECTERN);
        let data = entity("minecraft:lectern");
        let verdict = decide(Handler::Lectern, &Request::new([0, 70, 0], &lectern).with_data(&data));
        assert_eq!(written(&verdict), lectern);
        assert_eq!(first(&verdict).follow, Follow::Data);
    }

    #[test]
    fn the_nylium_is_written_as_itself_and_never_as_netherrack() {
        // Le *coût* est de la netherrack (`NetherrackPlacementHandler.java:49`), le bloc posé
        // est le nylium : les deux moitiés du handler ne disent pas la même chose.
        let nylium = state(kind::CRIMSON_NYLIUM);
        assert_eq!(written(&decide(Handler::Netherrack, &Request::new([0, 70, 0], &nylium))), nylium);
    }

    #[test]
    fn the_waypoint_is_recorded_rather_than_placed_in_a_colony() {
        let waypoint = state(kind::WAYPOINT);
        let request = Request::new([0, 70, 0], &waypoint).in_colony();
        assert_eq!(decide(Handler::WayPoint, &request), Verdict::Clears);
    }

    #[test]
    fn the_waypoint_is_still_cleared_when_no_colony_answers() {
        // `:40` retire le bloc avant de demander la colonie : sans colonie, la cellule finit
        // vide et rien n'est reposé.
        let waypoint = state(kind::WAYPOINT);
        assert_eq!(decide(Handler::WayPoint, &Request::new([0, 70, 0], &waypoint)), Verdict::Clears);
    }

    #[test]
    fn the_waypoint_is_written_back_in_a_sketch() {
        let waypoint = state(kind::WAYPOINT);
        let request = Request::new([0, 70, 0], &waypoint).in_colony().sketch();
        assert_eq!(written(&decide(Handler::WayPoint, &request)), waypoint);
    }

    #[test]
    fn a_named_grave_is_only_pasted_in_a_sketch() {
        let grave = state(kind::NAMED_GRAVE);
        assert_eq!(
            decide(Handler::NamedGrave, &Request::new([0, 70, 0], &grave)),
            Verdict::Spares(Why::SketchOnly)
        );
        assert_eq!(written(&decide(Handler::NamedGrave, &Request::new([0, 70, 0], &grave).sketch())), grave);
    }

    #[test]
    fn a_grave_already_standing_is_kept_with_its_contents() {
        let grave = state(kind::GRAVE);
        let request = Request::new([0, 70, 0], &grave).with_world(&grave, crate::colony_world::guard::Occupant::Container);
        assert_eq!(decide(Handler::Grave, &request), Verdict::Spares(Why::Kept));
    }

    #[test]
    fn the_rack_writes_its_state_and_never_a_guest_state() {
        let rack = state_of(kind::RACK, &[("type", "blockminecoloniesrack")]);
        let data = entity("minecolonies:rack");
        let verdict = decide(Handler::Rack, &Request::new([0, 70, 0], &rack).with_data(&data));
        assert_eq!(written(&verdict), rack);
        assert_eq!(first(&verdict).follow, Follow::Data);
    }

    #[test]
    fn flowing_water_is_left_to_the_world_when_building() {
        let flowing = state_of(kind::WATER, &[("level", "3")]);
        assert_eq!(decide(Handler::DimensionFluid, &Request::new([0, 70, 0], &flowing)), Verdict::Passes);
    }

    #[test]
    fn a_source_fluid_is_written_and_so_is_every_fluid_in_a_sketch() {
        let source = state(kind::WATER);
        assert_eq!(written(&decide(Handler::DimensionFluid, &Request::new([0, 70, 0], &source))), source);
        let flowing = state_of(kind::WATER, &[("level", "3")]);
        assert_eq!(
            written(&decide(Handler::DimensionFluid, &Request::new([0, 70, 0], &flowing).sketch())),
            flowing
        );
    }

    #[test]
    fn the_scarecrow_writes_both_halves_from_its_lower_one() {
        let scarecrow = state_of(kind::SCARECROW, &[("half", "lower"), ("lantern", "true")]);
        let verdict = decide(Handler::Field, &Request::new([3, 70, 5], &scarecrow));
        let Verdict::Writes(cells) = &verdict else {
            panic!("expected two cells, got {verdict:?}");
        };
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].at, [3, 70, 5]);
        assert_eq!(cells[0].state.property("half"), Some("lower"));
        assert_eq!(cells[1].at, [3, 71, 5], "the cell above");
        assert_eq!(cells[1].state.property("half"), Some("upper"));
        assert_eq!(cells[1].state.property("lantern"), Some("true"), "other properties survive");
    }

    #[test]
    fn the_upper_half_of_a_scarecrow_is_left_alone() {
        let upper = state_of(kind::SCARECROW, &[("half", "upper")]);
        assert_eq!(
            decide(Handler::Field, &Request::new([3, 71, 5], &upper)),
            Verdict::Spares(Why::AlreadyPaired)
        );
    }

    #[test]
    fn a_door_of_domum_writes_both_halves_too() {
        let door = state_of(kind::DOMUM_FANCY_DOOR, &[("half", "lower")]);
        let Verdict::Writes(cells) = decide(Handler::DoDoor, &Request::new([0, 70, 0], &door)) else {
            panic!("expected two cells");
        };
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[1].state.property("half"), Some("upper"));
    }

    #[test]
    fn only_the_door_carries_its_data_to_the_upper_half() {
        // La porte applique la même donnée aux deux moitiés
        // (`DoDoorBlockPlacementHandler.java:61-67`) ; l'épouvantail à la seule cellule
        // courante (`FieldPlacementHandler.java:48-59`).
        let data = entity("domum_ornamentum:fancy_door");
        let door = state_of(kind::DOMUM_FANCY_DOOR, &[("half", "lower")]);
        let Verdict::Writes(cells) =
            decide(Handler::DoDoor, &Request::new([0, 70, 0], &door).with_data(&data))
        else {
            panic!("expected two cells");
        };
        assert_eq!(cells[0].follow, Follow::Data);
        assert_eq!(cells[1].follow, Follow::Data, "the door twins its data");

        let scarecrow = state_of(kind::SCARECROW, &[("half", "lower")]);
        let scarecrow_data = entity("minecolonies:scarecrow");
        let Verdict::Writes(cells) =
            decide(Handler::Field, &Request::new([0, 70, 0], &scarecrow).with_data(&scarecrow_data))
        else {
            panic!("expected two cells");
        };
        assert_eq!(cells[0].follow, Follow::Data);
        assert_eq!(cells[1].follow, Follow::None, "the scarecrow does not");
    }

    #[test]
    fn a_double_block_without_a_half_is_refused_instead_of_crashing() {
        // Java lèverait une exception en plein collage (`DoDoorBlockPlacementHandler.java:50`).
        let door = state(kind::DOMUM_FANCY_DOOR);
        assert_eq!(
            decide(Handler::DoDoor, &Request::new([0, 70, 0], &door)),
            Verdict::Refuses(Refusal::MissingProperty {
                state: door.to_string(),
                property: "half",
            })
        );
    }

    #[test]
    fn the_barracks_tower_row_writes_what_java_would_have_written_if_it_ran() {
        // La ligne est inatteignable par la table (test de la table), mais son contrat reste
        // écrit : un état, et la donnée qui suit (`BarracksTowerHandler.java:55-71`).
        let tower = state(kind::BARRACKS_TOWER);
        let data = entity("minecolonies:colonybuilding");
        let verdict = decide(Handler::BarracksTower, &Request::new([0, 70, 0], &tower).with_data(&data));
        assert_eq!(written(&verdict), tower);
        assert_eq!(first(&verdict).follow, Follow::Data);
    }

    #[test]
    fn a_structure_block_is_only_pasted_in_a_sketch() {
        let block = state(kind::STRUCTURE_BLOCK);
        assert_eq!(
            decide(Handler::BuilderIgnore, &Request::new([0, 70, 0], &block)),
            Verdict::Spares(Why::SketchOnly)
        );
        assert_eq!(
            written(&decide(Handler::BuilderIgnore, &Request::new([0, 70, 0], &block).sketch())),
            block
        );
    }

    #[test]
    fn the_jigsaw_writes_the_state_its_data_names() {
        let jigsaw_state = state(kind::JIGSAW);
        let data = jigsaw(Some("minecraft:oak_planks[waterlogged=false]"));
        let verdict = decide(Handler::Jigsaw, &Request::new([0, 70, 0], &jigsaw_state).with_data(&data));
        assert_eq!(written(&verdict), state_of("minecraft:oak_planks", &[("waterlogged", "false")]));
    }

    #[test]
    fn a_jigsaw_whose_final_state_is_void_writes_nothing() {
        let jigsaw_state = state(kind::JIGSAW);
        let data = jigsaw(Some(kind::STRUCTURE_VOID));
        assert_eq!(
            decide(Handler::Jigsaw, &Request::new([0, 70, 0], &jigsaw_state).with_data(&data)),
            Verdict::Spares(Why::VoidState)
        );
    }

    #[test]
    fn a_jigsaw_without_a_final_state_writes_nothing() {
        let jigsaw_state = state(kind::JIGSAW);
        let data = jigsaw(None);
        assert_eq!(
            decide(Handler::Jigsaw, &Request::new([0, 70, 0], &jigsaw_state).with_data(&data)),
            Verdict::Spares(Why::NoFinalState)
        );
        // Et pas de donnée du tout, même réponse.
        assert_eq!(
            decide(Handler::Jigsaw, &Request::new([0, 70, 0], &jigsaw_state)),
            Verdict::Spares(Why::NoFinalState)
        );
    }

    #[test]
    fn a_broken_final_state_is_refused_rather_than_erased() {
        // Java écrit de l'air ici (`JigsawPlacementHandler.java:70-91`).
        let jigsaw_state = state(kind::JIGSAW);
        let data = jigsaw(Some("minecraft:oak_stairs[facing=north"));
        assert_eq!(
            decide(Handler::Jigsaw, &Request::new([0, 70, 0], &jigsaw_state).with_data(&data)),
            Verdict::Refuses(Refusal::BadFinalState {
                state: "minecraft:oak_stairs[facing=north".to_owned()
            })
        );
    }

    #[test]
    fn the_jigsaw_is_copied_as_it_is_in_a_sketch_whatever_its_data_says() {
        let jigsaw_state = state(kind::JIGSAW);
        let data = jigsaw(Some("minecraft:oak_planks"));
        let request = Request::new([0, 70, 0], &jigsaw_state).with_data(&data).sketch();
        assert_eq!(written(&decide(Handler::Jigsaw, &request)), jigsaw_state);
    }

    #[test]
    fn the_beehive_drops_its_honey_level_and_its_data() {
        let hive = state_of(kind::BEEHIVE, &[("facing", "south"), ("honey_level", "5")]);
        let data = entity("minecraft:beehive");
        let verdict = decide(Handler::Beehive, &Request::new([0, 70, 0], &hive).with_data(&data));
        let expected = state_of(kind::BEEHIVE, &[("facing", "south")]);
        assert_eq!(written(&verdict), expected);
        assert_eq!(first(&verdict).follow, Follow::None, "the bees do not travel");
    }

    #[test]
    fn every_structurize_row_is_delegated_and_never_decided_here() {
        let stone = state("minecraft:stone");
        let request = Request::new([0, 70, 0], &stone);
        for handler in [
            Handler::Air,
            Handler::Container,
            Handler::Banner,
            Handler::Door,
            Handler::Bed,
            Handler::FallingBlock,
            Handler::StructurizeGeneral,
        ] {
            assert_eq!(decide(handler, &request), Verdict::Delegated(handler), "{}", handler.class());
        }
    }

    #[test]
    fn parse_state_reads_a_vanilla_state_the_way_the_parser_does() {
        assert_eq!(parse_state("minecraft:oak_stairs"), Ok(state("minecraft:oak_stairs")));
        assert_eq!(
            parse_state("oak_stairs[facing=north,half=bottom]"),
            Ok(state_of("minecraft:oak_stairs", &[("facing", "north"), ("half", "bottom")]))
        );
        // L'indulgence de Java : une paire vide ne casse rien (`:74`, `false`).
        assert_eq!(
            parse_state("stone[ ]"),
            Ok(state("minecraft:stone"))
        );
    }

    #[test]
    fn parse_state_refuses_what_it_cannot_read() {
        for text in [
            "",
            "   ",
            "#minecraft:logs",
            "minecraft:oak_stairs[facing=north",
            "minecraft:oak_stairs[facing]",
            "minecraft:oak_stairs[=north]",
            "minecraft:oak stairs",
            "minecraft:oak_stairs[facing=north]]",
        ] {
            assert!(parse_state(text).is_err(), "{text:?} should be refused");
        }
    }

    #[test]
    fn a_container_keeps_its_data_and_a_plain_block_asks_for_a_replacement() {
        let rack = state(kind::RACK);
        let here = Here {
            occupant: crate::colony_world::guard::Occupant::Container,
            state: Some(&rack),
            data: None,
        };
        // Le même bloc que le monde, avec la donnée du blueprint : rien à écrire, la donnée
        // s'applique (`engine.rs:60-71`, `Change::Keep`).
        let cell = Cell { at: [1, 2, 3], state: rack.clone(), follow: Follow::Data };
        assert_eq!(cell.change(&here), crate::colony_world::intent::Change::Keep { at: [1, 2, 3] });

        // Un bloc ordinaire sur un conteneur : un remplacement, que `plan` refusera.
        let stone = state("minecraft:stone");
        let cell = Cell { at: [1, 2, 3], state: stone, follow: Follow::None };
        assert!(matches!(
            cell.change(&here),
            crate::colony_world::intent::Change::Replace { at: [1, 2, 3], .. }
        ));

        // Et une cellule libre : une pose, pas un remplacement.
        let open: Here<'_> = Here::open();
        assert!(matches!(
            cell.change(&open),
            crate::colony_world::intent::Change::Place { at: [1, 2, 3], .. }
        ));
    }

    #[test]
    fn a_hut_over_a_chest_is_refused_by_the_plan_and_never_overwritten() {
        use crate::colony_world::fakes::{Cells, OWNED, OVERWORLD, colony};
        use crate::colony_world::guard::{Guard, Occupant, State};
        use crate::colony_world::intent::Intent;

        let hut = state(kind::BARRACKS_TOWER);
        let chest = state("minecraft:chest");
        let here = Here {
            occupant: Occupant::Container,
            state: Some(&chest),
            data: None,
        };
        let change = Cell { at: [8, 70, 8], state: hut, follow: Follow::None }.change(&here);
        let world = Cells::of(Occupant::Container);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        let refused = Intent::plan(&colony(), &guard, State::Active, vec![change]).unwrap_err();
        assert_eq!(refused.name(), "colony_world.refusal.would_drop_contents");
        assert_eq!(refused.index, 0);
    }

    #[test]
    fn the_same_state_on_stone_is_a_placement_the_guard_refuses_as_occupied() {
        use crate::colony_world::fakes::{Cells, OWNED, OVERWORLD, colony};
        use crate::colony_world::guard::{Guard, Occupant, State};
        use crate::colony_world::intent::Intent;

        // `Cell::change` traduit un sol plein en `Replace` — mais une *pose* sur du stone
        // reste refusée : c'est la différence entre les deux opérations, et c'est pour ça
        // que le port ne choisit pas à la place de l'appelant.
        let stone = state("minecraft:stone");
        let here = Here {
            occupant: Occupant::Solid,
            state: Some(&stone),
            data: None,
        };
        let cell = Cell { at: [8, 70, 8], state: state("minecraft:dirt"), follow: Follow::None };
        assert!(matches!(
            cell.change(&here),
            crate::colony_world::intent::Change::Replace { .. }
        ));
        let place = crate::colony_world::intent::Change::Place {
            at: [8, 70, 8],
            block: "minecraft:dirt".to_owned(),
        };
        let world = Cells::of(Occupant::Solid);
        let guard = Guard::new(OVERWORLD, &OWNED, &world);
        let refused = Intent::plan(&colony(), &guard, State::Active, vec![place]).unwrap_err();
        assert_eq!(refused.refusal, crate::colony_world::refusal::Refusal::Occupied { at: [8, 70, 8] });
    }

    #[test]
    fn the_mode_says_what_complete_means() {
        // `complete == !fancyPlacement()` (`InfestedBlocksPlacementHandler.java:41`).
        assert!(Mode::Sketch.complete());
        assert!(!Mode::Fancy.complete());
    }
}
