//! Les scans de fours et l'arithmétique du transfert : ce que la couche four décide
//! sans rien toucher.
//!
//! Référence Java : `AbstractEntityAIRequestSmelter.java` — `countOfBurningFurnaces`
//! (`:648-667`), `getMaxUsableFurnaces` (`:673-688`), `accelerateFurnaces`'s formula
//! (`:122`), `areFurnacesLoaded` (`:631-641`), `getExtendedCount` (`:145-172`),
//! `getFurnaceToRetrieveOutputFrom` (`:551-575`),
//! `getFurnaceToRetrieveUnrelatedInputFrom` (`:581-602`), `getFurnaceWithoutFuel`
//! (`:531-542`), `getEmptyFurnaceWithFuel` (`:508-525`), the split of `fillUpFurnace`
//! (`:391-409`), the fuel request size (`:111`), `isCorrectFuel` (`:735-738`) and
//! `getActivePossibleFuels` (`:710-733`).
//!
//! **Chaque scan retient le premier four qui correspond, dans l'ordre de la hutte.** Ce
//! n'est pas un détail : `getFurnaces()` est une liste, et Java la parcourt sans jamais
//! trier (`:511`, `:533`, `:553`, `:583`, `:678`). Le port rend donc un **indice** dans
//! [`SmelterFacts::furnaces`] — le même fait que le `BlockPos` de Java, sans une seconde
//! recherche pour retrouver les slots au tick suivant.
//!
//! **Chaque fonction est pure.** Aucun scan ne lit le monde, n'écrit un slot, ni ne
//! compte un tick : ils lisent les faits et rendent une réponse. Les trois qui, dans
//! Java, écrivent au passage (`removeFromFurnaces` à `:597`, `getExtendedCount` qui lit
//! les slots, `getActivePossibleFuels` qui retire deux items de la liste) le disent dans
//! leur doc, et l'écriture reste à l'appelant.
//!
//! **Les < et les > sont recopiés tels quels.** `:477` demande `> 1` là où `:183` demande
//! `>= 1` : un four peut recevoir du carburant avec **un** item en réserve, mais la
//! recherche d'un four sans carburant en exige **deux**. Ce n'est pas une coquille du
//! port, c'est la ligne, et [`has_spare_fuel`] la nomme.

use crate::ai_build::materials::NeedKey;

use super::facts::{Furnace, SmelterFacts, FUEL_SLOT, RESULT_SLOT, SMELTABLE_SLOT, STACKSIZE};

/// Ce que [`unrelated_target`] a trouvé — et les trois réponses de Java, dont une qui
/// n'est pas une position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three exits of `getFurnaceToRetrieveUnrelatedInputFrom`
pub enum UnrelatedScan {
    /// Aucune entrée étrangère nulle part (`:601`).
    None,
    /// L'indice du premier four qui porte une entrée étrangère (`:588-593`).
    At(usize),
    /// L'indice d'une position qui n'est plus un four (`:595-599`) : Java la retire de
    /// la hutte et **arrête** le scan, il ne cherche pas plus loin.
    Forget(usize),
}

/// Ce que [`output_target`] a trouvé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a position or nothing
pub enum OutputScan {
    /// Aucun four n'a de sortie à prendre (`:574`).
    None,
    /// L'indice du four à vider (`:564`, `:570`).
    At(usize),
}

/// `areFurnacesLoaded` (`:631-641`) : un seul four déchargé et tout le chemin four
/// s'arrête (`:450-453`).
///
/// Aucun four est « tous chargés » : la liste vide est traitée avant, par
/// [`TradeRefusal::NoFurnaces`](super::state::TradeRefusal::NoFurnaces) (`:439-448`).
pub fn are_all_loaded(furnaces: &[Furnace]) -> bool {
    furnaces.iter().all(|furnace| furnace.loaded)
}

/// `countOfBurningFurnaces` (`:648-667`) : les fours **chargés** qui brûlent **et**
/// portent une entrée (`:659`).
pub fn count_of_burning(furnaces: &[Furnace]) -> usize {
    furnaces
        .iter()
        .filter(|furnace| furnace.loaded)
        .filter(|furnace| furnace.is_burning())
        .count()
}

/// `getMaxUsableFurnaces` (`:673-688`) : `min(niveau / 10 + 1, fours qui ont du
/// carburant)`.
///
/// La compétence est la **primaire** du métier (`:675`,
/// `getModuleForJob().getPrimarySkill()`). Le compte des fours à carburant, lui, ne
/// teste pas le chargement (`:678-685`) : Java ne le fait pas non plus.
pub fn max_usable_furnaces(primary_skill: u8, furnaces: &[Furnace]) -> usize {
    let by_skill = usize::from(primary_skill) / 10 + 1; // `:675`
    let fuelled = furnaces
        .iter()
        .filter(|furnace| !furnace.is_empty(FUEL_SLOT)) // `:681`
        .count();
    by_skill.min(fuelled) // `:687`
}

/// `accelerateFurnaces`'s formula (`:122`) : `(secondaire / 10) * 2` ticks de four
/// offerts par passe.
///
/// La compétence est la **secondaire** du métier, alors que [`max_usable_furnaces`] lit
/// la primaire : deux lignes, deux compétences, et c'est ce qui rend les deux fonctions
/// nécessaires.
pub fn acceleration_ticks(secondary_skill: u8) -> u32 {
    (u32::from(secondary_skill) / 10) * 2
}

/// `getExtendedCount(stack)` (`:145-172`) : ce que les fours comptent déjà pour `key`.
///
/// Java additionne le slot d'entrée **et** le slot de sortie de chaque four
/// (`:155-164`), et seulement quand la recette passe par un four (`:147`) — sinon zéro
/// (`:171`). Le `else if` de `:157-164` compte chaque slot une fois : un four dont
/// l'entrée et la sortie portent la même clé est compté deux fois, ce qui est le
/// comportement voulu (les deux quantités existent).
pub fn extended_count(facts: &SmelterFacts, key: &NeedKey) -> u32 {
    if !facts.intermediate_is_furnace {
        return 0; // `:147`, `:171`
    }
    facts
        .furnaces
        .iter()
        .map(|furnace| {
            let in_slot = furnace
                .slot(SMELTABLE_SLOT)
                .filter(|stack| &stack.key == key)
                .map(|stack| stack.count)
                .unwrap_or(0); // `:157-160`
            let in_result = furnace
                .slot(RESULT_SLOT)
                .filter(|stack| &stack.key == key)
                .map(|stack| stack.count)
                .unwrap_or(0); // `:161-164`
            in_slot + in_result
        })
        .sum()
}

/// `getFurnaceToRetrieveOutputFrom` (`:551-575`).
///
/// Deux façons de retenir un four : il est froid ou son entrée est vide (`:556`) **et**
/// (sa sortie est pleine à ras bord (`:562-565`) ou elle contient quelque chose
/// (`:568-571`)). Un four qui brûle avec une entrée garde sa sortie.
pub fn output_target(furnaces: &[Furnace]) -> OutputScan {
    for (index, furnace) in furnaces.iter().enumerate() {
        let may_take = !furnace.lit || furnace.is_empty(SMELTABLE_SLOT); // `:556`
        if !may_take {
            continue; // lit with an input: not yet
        }
        let Some(result) = furnace.slot(RESULT_SLOT) else {
            continue; // `:559` — nothing in the slot, nothing to take
        };
        if result.is_full() {
            return OutputScan::At(index); // `:562-565`
        }
        if result.count > 0 {
            return OutputScan::At(index); // `:568-571`
        }
    }
    OutputScan::None // `:574`
}

/// `getFurnaceToRetrieveUnrelatedInputFrom` (`:581-602`).
///
/// Une entrée est étrangère quand elle existe et qu'elle n'est pas la première entrée de
/// la recette courante — ou quand il n'y a pas de recette (`:589-590`). Une position qui
/// n'est plus un four est retirée de la hutte et arrête le scan (`:595-599`).
pub fn unrelated_target(furnaces: &[Furnace], smeltable: Option<&NeedKey>) -> UnrelatedScan {
    for (index, furnace) in furnaces.iter().enumerate() {
        if !furnace.loaded {
            // Java reads `world.getBlockEntity(pos)` without a load test here
            // (`:585`); an unloaded chunk reports no entity, which is the same
            // `else` branch as a position that is not a furnace. The caller reports
            // `loaded = false` for those, so this is `:595-599`.
            return UnrelatedScan::Forget(index);
        }
        let Some(stack) = furnace.slot(SMELTABLE_SLOT) else {
            continue; // `:588` — an empty input is not a foreign one
        };
        let ours = smeltable.map(|key| key == &stack.key).unwrap_or(false);
        if !ours {
            return UnrelatedScan::At(index); // `:588-593`
        }
    }
    UnrelatedScan::None // `:601`
}

/// `getFurnaceWithoutFuel` (`:531-542`) : le premier four **éteint** dont le slot de
/// carburant est vide.
pub fn without_fuel(furnaces: &[Furnace]) -> Option<usize> {
    furnaces
        .iter()
        .position(|furnace| !furnace.lit && furnace.is_empty(FUEL_SLOT)) // `:536`
}

/// `getEmptyFurnaceWithFuel` (`:508-525`) : le premier four dont l'entrée **et** la
/// sortie sont vides et dont le carburant ne l'est pas.
pub fn empty_with_fuel(furnaces: &[Furnace]) -> Option<usize> {
    furnaces.iter().position(|furnace| {
        furnace.is_empty(SMELTABLE_SLOT) && furnace.is_empty(RESULT_SLOT) && !furnace.is_empty(FUEL_SLOT)
    }) // `:516-519`
}

/// La répartition de `fillUpFurnace` (`:391-409`) : combien d'items partir dans **ce**
/// four.
///
/// `pending` est `(max - compteur) - déjà dans les fours` (`:372`), `in_inventory` ce
/// que le citoyen porte, et `available = min(pending, in_inventory)` (`:378`). Trois
/// branches, dans l'ordre de Java :
///
/// 1. plus que `STACKSIZE × fours libres` : un stack plein (`:395-398`) ;
/// 2. un seul four libre : tout ce qui est disponible (`:399-402`) ;
/// 3. sinon : la part égale **plus le reste sur le premier** (`:403-408`), plafonnée à
///    un stack.
pub fn fill_split(pending: i64, in_inventory: u32, max_furnaces: usize, burning: usize) -> u32 {
    let available = pending.min(i64::from(in_inventory)).max(0) as u32; // `:378`
    let free = max_furnaces.saturating_sub(burning); // `:392`
    if free == 0 {
        return 0; // `:393` — no furnace to put it in
    }
    if available > STACKSIZE.saturating_mul(free as u32) {
        return STACKSIZE; // `:395-398`
    }
    if free == 1 {
        return available; // `:399-402`
    }
    // `:407` — `(available / free) + (available % free)`, clamped to a stack.
    ((available / free as u32) + (available % free as u32)).min(STACKSIZE)
}

/// La taille de la requête de carburant (`:111`) : `STACKSIZE × nombre de fours`.
pub fn fuel_request_size(furnace_count: usize) -> u32 {
    STACKSIZE.saturating_mul(furnace_count as u32)
}

/// Le minimum de la même requête (`:111`) : `1`.
pub const FUEL_REQUEST_MIN: u32 = 1;

/// `isCorrectFuel(possibleFuels)` (`:735-738`) :
/// `ItemStackUtils.compareItemStackListIgnoreStackSize(possibleFuels, item)` — l'item
/// est dans la liste.
pub fn is_correct_fuel(fuel_list: &[NeedKey], stack: &NeedKey) -> bool {
    fuel_list.iter().any(|fuel| fuel == stack)
}

/// `getActivePossibleFuels` (`:710-733`) : les carburants déclarés, moins la sortie et
/// l'entrée de la recette courante.
///
/// Un four qui fond du sable pour faire du verre ne doit pas brûler du verre ni du
/// sable : Java retire les deux par `removeIf` (`:715-719`), dans cet ordre, et seulement
/// s'il y a une recette chargée. La liste vide est rendue telle quelle — c'est
/// `getAllowedFuel()` qui vaut `[]` et c'est l'appelant qui refuse (`:456-463`).
pub fn active_fuels(
    fuel_list: &[NeedKey],
    output: Option<&NeedKey>,
    smeltable: Option<&NeedKey>,
) -> Vec<NeedKey> {
    let mut fuels: Vec<NeedKey> = fuel_list.to_vec(); // `:711`
    if let Some(output) = output {
        fuels.retain(|fuel| fuel != output); // `:717`
    }
    if let Some(smeltable) = smeltable {
        fuels.retain(|fuel| fuel != smeltable); // `:719`
    }
    fuels
}

/// `:477` : la hutte en a **plus d'un**, ou le citoyen en porte.
///
/// C'est la seule des trois lectures du compte qui demande `> 1` ; `:108` demande
/// `< 1` et `:183` `>= 1`. Une fonction séparée pour que la ligne se voie.
pub fn has_spare_fuel(fuel_in_building: u32, fuel_in_inventory: bool) -> bool {
    fuel_in_building > 1 || fuel_in_inventory
}

/// `:362` : `burningFurnaces > maxFurnaces` arrête l'emplissage, avant même de marcher.
pub fn more_burning_than_usable(burning: usize, max_furnaces: usize) -> bool {
    burning > max_furnaces
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::Pos;
    use crate::ai_work_03::facts::FurnaceStack;

    fn furnace(x: i32) -> Furnace {
        Furnace::empty(Pos::new(x, 64, 0))
    }

    fn fuel(count: u32) -> Option<FurnaceStack> {
        Some(FurnaceStack::new(NeedKey::plain("minecraft:coal"), count))
    }

    fn smeltable(name: &str, count: u32) -> Option<FurnaceStack> {
        Some(FurnaceStack::new(NeedKey::plain(name), count))
    }

    #[test]
    fn the_burning_count_reads_lit_and_holding_a_smeltable() {
        // `:659`.
        let mut lit = furnace(0);
        lit.lit = true;
        lit.smeltable = smeltable("minecraft:sand", 4);
        let mut lit_empty = furnace(1);
        lit_empty.lit = true;
        let mut cold_holding = furnace(2);
        cold_holding.smeltable = smeltable("minecraft:sand", 1);
        assert_eq!(count_of_burning(&[lit, lit_empty, cold_holding]), 1);
        assert_eq!(count_of_burning(&[]), 0);
    }

    #[test]
    fn the_burning_count_ignores_an_unloaded_furnace() {
        // `:654` — `WorldUtil.isBlockLoaded` gates the read.
        let mut burning = furnace(0);
        burning.lit = true;
        burning.smeltable = smeltable("minecraft:sand", 1);
        burning.loaded = false;
        assert_eq!(count_of_burning(&[burning]), 0);
    }

    #[test]
    fn max_usable_furnaces_is_the_skill_rule_capped_by_the_fuelled_ones() {
        // `:675` puis `:687` : `min(niveau / 10 + 1, fours avec carburant)`.
        let mut fuelled_a = furnace(0);
        fuelled_a.fuel = fuel(8);
        let mut fuelled_b = furnace(1);
        fuelled_b.fuel = fuel(1);
        let dry = furnace(2);
        let three = [fuelled_a, fuelled_b, dry];

        assert_eq!(max_usable_furnaces(0, &three), 1, "skill 0 -> one furnace");
        assert_eq!(max_usable_furnaces(10, &three), 2, "skill 10 -> two");
        assert_eq!(max_usable_furnaces(99, &three), 2, "capped by the fuelled ones");
        assert_eq!(max_usable_furnaces(99, &[]), 0, "no furnace at all");
        // An unloaded furnace with fuel still counts: `:678-685` has no load test.
        let mut unloaded = furnace(3);
        unloaded.fuel = fuel(1);
        unloaded.loaded = false;
        assert_eq!(max_usable_furnaces(99, &[unloaded]), 1);
    }

    #[test]
    fn acceleration_is_two_ticks_per_ten_levels_of_the_secondary_skill() {
        // `:122` — `(secondaire / 10) * 2`.
        assert_eq!(acceleration_ticks(0), 0);
        assert_eq!(acceleration_ticks(9), 0);
        assert_eq!(acceleration_ticks(10), 2);
        assert_eq!(acceleration_ticks(25), 4);
        assert_eq!(acceleration_ticks(99), 18);
    }

    #[test]
    fn one_unloaded_furnace_stops_the_whole_furnace_path() {
        // `:631-641` answers false on the first unloaded one; `:450-453` then returns
        // `START_WORKING` without asking anything else.
        let loaded = furnace(0);
        let mut unloaded = furnace(1);
        unloaded.loaded = false;
        assert!(are_all_loaded(&[loaded.clone()]));
        assert!(are_all_loaded(&[]), "no furnace is not an unloaded one");
        assert!(!are_all_loaded(&[loaded, unloaded]));
    }

    #[test]
    fn the_extended_count_adds_the_input_and_the_result_slots() {
        // `:155-164`.
        let mut four = furnace(0);
        four.smeltable = smeltable("minecraft:sand", 5);
        four.result = smeltable("minecraft:sand", 2);
        let mut facts = SmelterFacts {
            intermediate_is_furnace: true,
            ..SmelterFacts::default()
        };
        facts.furnaces = vec![four];
        assert_eq!(extended_count(&facts, &NeedKey::plain("minecraft:sand")), 7);
        assert_eq!(extended_count(&facts, &NeedKey::plain("minecraft:glass")), 0);
        // `:147`, `:171` — without a furnace recipe the count is zero.
        facts.intermediate_is_furnace = false;
        assert_eq!(extended_count(&facts, &NeedKey::plain("minecraft:sand")), 0);
    }

    #[test]
    fn the_output_scan_takes_a_cold_or_empty_furnace_with_something_in_the_result() {
        // `:556` then `:568-571`.
        let mut burning_full = furnace(0);
        burning_full.lit = true;
        burning_full.smeltable = smeltable("minecraft:sand", 3);
        burning_full.result = smeltable("minecraft:glass", 1);
        let mut cold_with_result = furnace(1);
        cold_with_result.result = smeltable("minecraft:glass", 1);
        assert_eq!(
            output_target(&[burning_full.clone()]),
            OutputScan::None,
            "lit with an input keeps its result"
        );
        assert_eq!(
            output_target(&[burning_full, cold_with_result]),
            OutputScan::At(1)
        );
        assert_eq!(output_target(&[furnace(2)]), OutputScan::None);
    }

    #[test]
    fn a_full_result_slot_is_taken_even_from_a_burning_furnace() {
        // `:562-565` wins before `:556`'s condition matters — but the condition is on
        // the same `if`, so the furnace must still be cold *or* have an empty input.
        // Java's condition is `(!isLit() || smeltable.isEmpty())`, so a burning furnace
        // with a full result and a non-empty input is **not** taken.
        let mut full_result = furnace(0);
        full_result.result = Some(FurnaceStack::new(NeedKey::plain("minecraft:glass"), 64));
        assert_eq!(output_target(&[full_result.clone()]), OutputScan::At(0));
        // Same furnace, now lit and holding a smeltable: the outer condition fails.
        full_result.lit = true;
        full_result.smeltable = smeltable("minecraft:sand", 1);
        assert_eq!(output_target(&[full_result]), OutputScan::None);
        // A full result in a *cold* furnace is taken (`:562-565`).
        let mut cold = furnace(1);
        cold.result = Some(FurnaceStack::new(NeedKey::plain("minecraft:glass"), 64));
        assert_eq!(output_target(&[cold]), OutputScan::At(0));
    }

    #[test]
    fn the_unrelated_scan_names_a_foreign_input_and_accepts_our_own() {
        // `:588-593`.
        let mut ours = furnace(0);
        ours.smeltable = smeltable("minecraft:sand", 3);
        let mut foreign = furnace(1);
        foreign.smeltable = smeltable("minecraft:cobblestone", 1);
        let sand = NeedKey::plain("minecraft:sand");
        assert_eq!(
            unrelated_target(&[ours.clone()], Some(&sand)),
            UnrelatedScan::None,
            "our own input is not foreign"
        );
        assert_eq!(
            unrelated_target(&[ours, foreign], Some(&sand)),
            UnrelatedScan::At(1)
        );
        // `:589` — with no recipe loaded, any input at all is foreign.
        let mut any = furnace(2);
        any.smeltable = smeltable("minecraft:sand", 1);
        assert_eq!(unrelated_target(&[any], None), UnrelatedScan::At(0));
    }

    #[test]
    fn a_position_that_is_not_a_furnace_is_forgotten_and_stops_the_scan() {
        // `:595-599` — Java removes it and returns null, it does not scan further.
        let mut gone = furnace(0);
        gone.loaded = false;
        let mut foreign = furnace(1);
        foreign.smeltable = smeltable("minecraft:cobblestone", 1);
        assert_eq!(
            unrelated_target(&[gone, foreign], Some(&NeedKey::plain("minecraft:sand"))),
            UnrelatedScan::Forget(0)
        );
    }

    #[test]
    fn a_furnace_without_fuel_is_one_that_is_cold_and_empty() {
        // `:536` — `!isLit() && getItem(FUEL_SLOT).isEmpty()`.
        let mut lit = furnace(0);
        lit.lit = true;
        let mut dry = furnace(1);
        dry.loaded = false;
        let mut fuelled = furnace(2);
        fuelled.fuel = fuel(4);
        assert_eq!(without_fuel(&[lit, dry.clone(), fuelled.clone()]), Some(1));
        assert_eq!(without_fuel(&[fuelled]), None);
        // The load test is absent from this scan, like Java's (`:533-541`).
        let mut lit_dry = furnace(3);
        lit_dry.lit = true;
        assert_eq!(without_fuel(&[lit_dry]), None, "a lit furnace is not fuelled");
    }

    #[test]
    fn an_empty_furnace_with_fuel_is_the_fill_target() {
        // `:516-519`.
        let mut with_input = furnace(1);
        with_input.fuel = fuel(8);
        with_input.smeltable = smeltable("minecraft:sand", 1);
        let mut with_result = furnace(2);
        with_result.fuel = fuel(8);
        with_result.result = smeltable("minecraft:glass", 1);
        assert_eq!(empty_with_fuel(&[with_input, with_result]), None);
        // Only the fourth furnace is empty and fuelled.
        let mut only_fuelled = furnace(4);
        only_fuelled.fuel = fuel(8);
        assert_eq!(
            empty_with_fuel(&[furnace(3), only_fuelled]),
            Some(1),
            "the first furnace has no fuel"
        );
        assert_eq!(empty_with_fuel(&[furnace(5)]), None, "no fuel, no fill");
    }

    #[test]
    fn the_fill_split_front_loads_the_remainder_on_the_first_furnace() {
        // `:378` then `:391-409`, branch by branch.
        let pending = 20;
        // `:399-402` — a single free furnace takes everything available.
        assert_eq!(fill_split(pending, 20, 1, 0), 20);
        // `:403-408` — two free: 20 / 2 = 10 and 20 % 2 = 0.
        assert_eq!(fill_split(pending, 20, 2, 0), 10);
        // 21 over two free: 10 + 1 = 11 on the first, 10 on the second.
        assert_eq!(fill_split(21, 21, 2, 0), 11);
        // `:395-398` — more than a stack per free furnace: one stack.
        assert_eq!(fill_split(500, 500, 2, 0), STACKSIZE);
        // `:378` — the inventory caps what can leave.
        assert_eq!(fill_split(64, 3, 1, 0), 3);
        // `:392-393` — no free furnace, nothing moves.
        assert_eq!(fill_split(20, 20, 1, 1), 0);
        assert_eq!(fill_split(20, 20, 0, 0), 0);
        // A negative pending is Java's "already covered"; it transfers nothing.
        assert_eq!(fill_split(-5, 20, 2, 0), 0);
    }

    #[test]
    fn the_fuel_request_is_the_stack_size_times_the_furnaces() {
        // `:111` — `STACKSIZE * module.getFurnaces().size()`, minimum 1.
        assert_eq!(fuel_request_size(0), 0);
        assert_eq!(fuel_request_size(1), 64);
        assert_eq!(fuel_request_size(3), 192);
        assert_eq!(FUEL_REQUEST_MIN, 1);
    }

    #[test]
    fn the_active_fuels_drop_our_own_input_and_output() {
        // `:710-733` — `removeIf` twice, in that order, only with a recipe loaded.
        let coal = NeedKey::plain("minecraft:coal");
        let sand = NeedKey::plain("minecraft:sand");
        let glass = NeedKey::plain("minecraft:glass");
        let list = vec![coal.clone(), sand.clone(), glass.clone()];

        assert_eq!(
            active_fuels(&list, Some(&glass), Some(&sand)),
            vec![coal.clone()],
            "both the output and the input leave"
        );
        assert_eq!(
            active_fuels(&list, None, None),
            list.clone(),
            "no recipe, nothing removed"
        );
        assert_eq!(active_fuels(&[], Some(&glass), Some(&sand)), Vec::new());
        assert!(is_correct_fuel(&list, &coal));
        assert!(!is_correct_fuel(&list, &NeedKey::plain("minecraft:oak_planks")));
        assert!(!is_correct_fuel(&[], &coal), "an empty list allows nothing");
    }

    #[test]
    fn spare_fuel_is_two_in_the_hut_or_one_in_the_hand() {
        // `:477` — `hasBuildingEnoughElseCount(...) > 1 || hasItemInItemHandler(...)`,
        // the only one of the three fuel counts that asks for more than one.
        assert!(!has_spare_fuel(0, false));
        assert!(!has_spare_fuel(1, false), "`> 1`, not `>= 1`");
        assert!(has_spare_fuel(2, false));
        assert!(has_spare_fuel(0, true));
        assert!(!more_burning_than_usable(1, 1));
        assert!(more_burning_than_usable(2, 1), "`:362`");
    }
}
