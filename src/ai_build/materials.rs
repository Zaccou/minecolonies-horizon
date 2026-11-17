//! What a builder does when the blueprint wants something it does not have: the
//! material list, the answer to "do we have this, or must we ask?", and the five
//! stages the asking walks.
//!
//! Java reference: `AbstractEntityAIStructure.hasListOfResInInvOrRequest`
//! (`:816-923`, the resolver), `isBlockFree` (`:1000-1006`), `getTotalAmount`
//! (`AbstractEntityAIStructureWithWorkOrder.java:511-537`), `registerBlockAsNeeded`
//! (`:346-353`), `reduceNeededResources` (`:473-477`), `RequestStage` (`:47-62`) and
//! `requestMaterials` (`:232-343`), with `requestMaterialsState` (`:203-221`).
//!
//! ## The one rule that is easy to get wrong
//!
//! `hasListOfResInInvOrRequest` answers three values, and they are **not** a
//! success/failure pair:
//!
//! | Answer | When | What the builder does |
//! | --- | --- | --- |
//! | `SUCCESS` | every stack is in the inventory (or nothing is left to ask for) | place the block (`:922`) |
//! | `RECALC` | some stack is in **neither** the inventory **nor** the hut's bucket (`:840-845`) | forget the material list, `LOAD_STRUCTURE` again (`:509-512`) |
//! | `FAIL` | the list still holds a stack after the inventory pass (`:890-921`) | `NEEDS_ITEM`, and a request exists |
//!
//! `FAIL` is the *normal* case of a build in progress: the builder creates the
//! request (`:903-906`), walks to `NEEDS_ITEM`, and the base table waits for it
//! (`AbstractEntityAIBasic.waitForRequests`, `:534-643`). What the port returns is
//! **which** of the three, plus — for `FAIL` — the stack that stopped it and
//! whether a request was created or found.
//!
//! ## Two Java habits kept, because they change the answer
//!
//! - The `return FAIL` at `:901-921` sits **inside** the loop over the remaining
//!   stacks. The first stack that is missing decides; the rest are never looked at.
//!   The port returns that first stack, not a set of them.
//! - A duplicate stack is merged by `ItemStorage` (`:829-836`), and the merge
//!   **adds the amounts**. Then the `force` branch subtracts what the worker carries
//!   (`:862-879`), while the non-force branch drops a stack as soon as the worker
//!   carries **any** of it (`:880-886`) — the count is ignored there. Both are
//!   ported as they are; the tests hold the difference.
//!
//! ## What this module is not
//!
//! It never touches the request system. Creating a `Stack` request (`:904`),
//! registering the block as needed (`:905`), moving a completed request into the
//! citizen's sync list (`:912`, `:917`): all three are host calls, and
//! `crate::requests` is where a request lives. The decision says *what* to ask for;
//! the caller makes the call.

use crate::ai::Pos;

use super::order::BlockFacts;

/// `Constants.STACKSIZE` (`Constants.java:23`): the ceiling `getTotalAmount` puts on
/// one request (`AbstractEntityAIStructureWithWorkOrder.java:534`).
pub const STACKSIZE: u32 = 64;

/// Where this file's rules come from, as one string a log line can print.
pub const SOURCE: &str = "AbstractEntityAIStructure.java:816-923,:1000-1006 · \
AbstractEntityAIStructureWithWorkOrder.java:47-62,:203-221,:232-343,:346-353,:473-477,:511-537 · \
Constants.java:23";

/// The identity of a needed stack, in the two halves Java concatenates.
///
/// Java reference: `building.getNeededResources().get(stack.getDescriptionId() + "-"
/// + hashCode)` (`AbstractEntityAIStructureWithWorkOrder.java:349-350`, `:519-521`,
/// `:363`), where `hashCode` is `stack.hasTag() ? stack.getTag().hashCode() : 0`
/// (`:348`, `:520`).
///
/// The port keeps the two halves apart instead of building the string, because a
/// test that asserts on `"minecraft:oak_planks-1837465"` asserts on a hash Java
/// never guaranteed; a test that asserts on the pair asserts on the rule.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NeedKey {
    /// `ItemStack.getDescriptionId()`, e.g. `minecraft:oak_planks`.
    pub description_id: String,
    /// The tag's hash, `0` when the stack has no tag.
    pub tag_hash: i32,
}

impl NeedKey {
    /// A key for a stack with no tag.
    pub fn plain(description_id: impl Into<String>) -> NeedKey {
        NeedKey {
            description_id: description_id.into(),
            tag_hash: 0,
        }
    }
}

impl core::fmt::Display for NeedKey {
    /// The string Java builds: `descriptionId + "-" + hashCode` (`:349`).
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}-{}", self.description_id, self.tag_hash)
    }
}

/// One stack the blueprint wants, and what the worker can see of it.
///
/// Java reference: `BuildingBuilderResource` / `ItemStorage` as
/// `hasListOfResInInvOrRequest` reads them — `stack.getCount()` (`:903`),
/// `ItemStackUtils.isEmpty(stack)` (`:893`), `InventoryUtils.hasItemInItemHandler`
/// (`:840`), `building.hasResourceInBucket` (`:841`),
/// `InventoryUtils.filterItemHandler` (`:847-860`),
/// `building.getOpenRequestsOfCitizenOrBuilding` (`:898`) and
/// `getCompletedRequestsOfCitizenOrBuilding` (`:899`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Need {
    /// The stack's identity (`:829-836`).
    pub key: NeedKey,
    /// `stack.getCount()`, accumulated when two stacks merge (`:834`).
    pub count: u32,
    /// `isBlockFree(...)` (`:824`): water, leaves, air and the decoration
    /// placeholder are never asked for.
    pub free: bool,
    /// `ItemStackUtils.isEmpty(stack)` (`:893`).
    pub empty: bool,
    /// `InventoryUtils.hasItemInItemHandler(inventory, matches(stack))` (`:840`) — a
    /// boolean in Java, so a single item is enough to pass this test.
    pub in_inventory: bool,
    /// `building.hasResourceInBucket(stack)` (`:841`).
    pub in_bucket: bool,
    /// How many the worker actually carries, summed over the matching slots
    /// (`:847-860`).
    pub carried: u32,
    /// `getOpenRequestsOfCitizenOrBuilding(...).size()` (`:898`).
    pub open_requests: u32,
    /// `getCompletedRequestsOfCitizenOrBuilding(...).size()` (`:899`).
    pub completed_requests: u32,
}

impl Need {
    /// A stack the blueprint wants, carried by nobody, asked for by nobody.
    pub fn new(key: NeedKey, count: u32) -> Need {
        Need {
            key,
            count,
            free: false,
            empty: false,
            in_inventory: false,
            in_bucket: false,
            carried: 0,
            open_requests: 0,
            completed_requests: 0,
        }
    }

    /// The same stack, in the worker's inventory.
    pub fn carried(mut self, carried: u32) -> Need {
        self.carried = carried;
        self.in_inventory = carried > 0;
        self
    }

    /// The same stack, in the hut's bucket.
    pub fn in_bucket(mut self) -> Need {
        self.in_bucket = true;
        self
    }

    /// The same stack, with an open request on it.
    pub fn opened(mut self, open: u32) -> Need {
        self.open_requests = open;
        self
    }

    /// The same stack, with a completed request on it.
    pub fn completed(mut self, completed: u32) -> Need {
        self.completed_requests = completed;
        self
    }

    /// The same stack, free of charge (`:824`).
    pub fn free(mut self) -> Need {
        self.free = true;
        self
    }
}

/// Why the list still has a stack on it after the inventory pass, i.e. what the
/// `FAIL` of `:890-921` means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three branches of `:890-921`
pub enum Because {
    /// `ItemStackUtils.isEmpty(stack)` (`:893-896`): there is nothing to ask for,
    /// and the builder cannot go on. Java answers `FAIL` without a message.
    EmptyStack,
    /// Neither an open nor a completed request matched, so Java creates one
    /// (`:901-907`).
    RequestCreated,
    /// A request already existed and was moved into the citizen's sync list
    /// (`:908-919`), and Java still answers `FAIL` — the builder waits.
    RequestAlreadyOpen,
}

impl Because {
    /// Java's name for the branch, for a log line.
    pub fn java_name(self) -> &'static str {
        match self {
            Because::EmptyStack => "FAIL (empty stack)",
            Because::RequestCreated => "FAIL (request created)",
            Because::RequestAlreadyOpen => "FAIL (request already open)",
        }
    }
}

/// The stack that stopped the resolution, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed {
    /// `entry.getKey()`, the stack Java was looking at.
    pub need: NeedKey,
    /// Which branch of `:890-921` it took.
    pub because: Because,
}

/// What `hasListOfResInInvOrRequest` answered (`:816-923`).
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: `SUCCESS`, `RECALC` and `FAIL` are the three values Java returns
pub enum Resolution {
    /// `SUCCESS` (`:922`): the worker has, or is about to have, everything.
    Success,
    /// `RECALC` (`:843`): a stack is nowhere, so the material list is stale. The
    /// caller sets `workOrder.setRequested(false)` and goes back to
    /// `LOAD_STRUCTURE` (`:509-512`).
    Recalculate,
    /// `FAIL` (`:895`, `:906`, `:920`).
    Failed(Failed),
}

/// `hasListOfResInInvOrRequest(placer, itemList, force)` (`:816-923`).
///
/// `force` is the second argument of the Java call: `structureStep` passes
/// `result.getBlockResult().getRequiredItems().size() > 1` (`:509`), i.e. `true`
/// when the blueprint wants more than one stack at this position.
pub fn resolve(needs: &[Need], force: bool) -> Resolution {
    // `:821-836` — merge by identity, skipping the free blocks at `:824`.
    let mut merged: Vec<Need> = Vec::new();
    for need in needs {
        if need.free {
            continue;
        }
        match merged.iter_mut().find(|known| known.key == need.key) {
            Some(slot) => {
                // `tempStorage.setAmount(tempStorage.getAmount() + oldSize)` (`:832`).
                slot.count = slot.count.saturating_add(need.count);
                // The facts of two reports of the same stack are unioned: Java has
                // one entry and reads the inventory once, so "any report says it is
                // carried" is the closest honest merge.
                slot.in_inventory |= need.in_inventory;
                slot.in_bucket |= need.in_bucket;
                slot.empty |= need.empty;
                slot.carried = slot.carried.saturating_add(need.carried);
                slot.open_requests = slot.open_requests.max(need.open_requests);
                slot.completed_requests = slot.completed_requests.max(need.completed_requests);
            }
            None => merged.push(need.clone()),
        }
    }

    // `:838-845` — one stack that is nowhere invalidates the whole list.
    if merged
        .iter()
        .any(|need| !need.in_inventory && !need.in_bucket)
    {
        return Resolution::Recalculate;
    }

    // `:847-886` — drop what the worker really carries, or subtract it.
    if force {
        for need in &mut merged {
            if need.carried >= need.count {
                // `:869-871` — covered, so it leaves the map.
                need.count = 0;
            } else {
                // `:873-876` — partially covered, the rest is still wanted.
                need.count -= need.carried;
            }
        }
        merged.retain(|need| need.count > 0);
    } else {
        // `:880-886` — a single carried item drops the entry, whatever the count.
        merged.retain(|need| need.carried == 0);
    }

    // `:890-921` — the first stack left decides, and the loop returns there.
    let Some(need) = merged.first() else {
        return Resolution::Success; // `:922`
    };
    if need.empty {
        // `:893-896` — nothing to ask for.
        return Resolution::Failed(Failed {
            need: need.key.clone(),
            because: Because::EmptyStack,
        });
    }
    let because = if need.open_requests == 0 && need.completed_requests == 0 {
        Because::RequestCreated // `:901-907`
    } else {
        Because::RequestAlreadyOpen // `:908-919`
    };
    Resolution::Failed(Failed {
        need: need.key.clone(),
        because,
    })
}

/// `isBlockFree` (`AbstractEntityAIStructure.java:1000-1006`): a block a wall is
/// made of without costing anything.
///
/// Java answers true for `null`, water, leaves and the decoration placeholder. A
/// `null` state is [`BlockFacts::air`] here — the port has no `null` block.
pub fn is_block_free(block: BlockFacts) -> bool {
    block.air || block.is_water() || block.leaves || block.decoration_placeholder
}

/// `registerBlockAsNeeded` (`AbstractEntityAIStructureWithWorkOrder.java:346-353`):
/// a stack is registered once per identity, with an amount of 1.
///
/// Java builds the key and asks `getNeededResources().get(...) == null`. The port
/// asks the same question of the keys the caller already has.
pub fn needs_registration(known: &[NeedKey], key: &NeedKey) -> bool {
    !known.contains(key)
}

/// What to ask for when a missing stack is seen the first time — Java's
/// `building.addNeededResource(stack, 1)` (`:351`).
pub fn registration(key: &NeedKey) -> Need {
    Need::new(key.clone(), 1)
}

/// `getTotalAmount` (`AbstractEntityAIStructureWithWorkOrder.java:511-537`): the
/// amount a request should carry.
///
/// Java answers `null` for an empty stack (the caller then asks for nothing, `:515-518`),
/// the known amount clamped to [`STACKSIZE`] when the hut knows the resource
/// (`:534`), and the stack itself otherwise.
pub fn total_amount(known: Option<u32>, count: u32) -> u32 {
    match known {
        Some(amount) => amount.min(STACKSIZE),
        None => count,
    }
}

/// `requestMaterialsState`'s arithmetic (`:216-220`): the work order's resource
/// count only ever grows, and starts at the new total when it was zero.
pub fn amount_of_resources(previous: u32, current: u32) -> u32 {
    if previous == 0 || current > previous {
        current
    } else {
        previous
    }
}

/// `reduceNeededResources` (`:473-477` → `building.reduceNeededResource(stack, 1)`):
/// one item used for a placement.
///
/// `None` means the entry is gone: Java's `BuildingBuilderResource` is removed when
/// its amount reaches zero, which is why the caller must not keep a zero entry.
pub fn reduced(count: u32) -> Option<u32> {
    match count.checked_sub(1) {
        Some(left) if left > 0 => Some(left),
        _ => None,
    }
}

/// The five stages of the material request, in the order Java declares them.
///
/// Java reference: `AbstractEntityAIStructureWithWorkOrder.java:47-62`. The
/// declaration order **is** the walking order here (unlike the placement stages,
/// which follow the array each work order gets): `requestMaterials` advances with
/// `RequestStage.values()[requestState.ordinal() + 1]` (`:340`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// exhaustive: the five values of the Java enum
pub enum RequestStage {
    /// `SOLID_FRAME` (`:49`): the frame tags of the blueprint, if it has any.
    SolidFrame,
    /// `SOLID` (`:51`): everything `isAnySolid`.
    Solid,
    /// `WEAK_SOLID` (`:53`): panes, fences, leaves that tick.
    WeakSolid,
    /// `DECO` (`:58`): the decoration items.
    Deco,
    /// `ENTITIES` (`:60`): the blueprint's entities.
    Entities,
}

impl RequestStage {
    /// The five, in Java's declaration order.
    pub const ORDER: [RequestStage; 5] = [
        RequestStage::SolidFrame,
        RequestStage::Solid,
        RequestStage::WeakSolid,
        RequestStage::Deco,
        RequestStage::Entities,
    ];

    /// Java's identifier, which is also what a saved request progress holds.
    pub fn java_name(self) -> &'static str {
        match self {
            RequestStage::SolidFrame => "SOLID_FRAME",
            RequestStage::Solid => "SOLID",
            RequestStage::WeakSolid => "WEAK_SOLID",
            RequestStage::Deco => "DECO",
            RequestStage::Entities => "ENTITIES",
        }
    }

    /// The rank of this stage, `0` first.
    pub fn rank(self) -> usize {
        match self {
            RequestStage::SolidFrame => 0,
            RequestStage::Solid => 1,
            RequestStage::WeakSolid => 2,
            RequestStage::Deco => 3,
            RequestStage::Entities => 4,
        }
    }

    /// `RequestStage.values()[requestState.ordinal() + 1]` (`:340`): the next stage,
    /// `None` past `ENTITIES` — which Java never reaches, because `:317-320` returns
    /// `true` for any unknown stage first.
    pub fn next(self) -> Option<RequestStage> {
        RequestStage::ORDER.get(self.rank() + 1).copied()
    }
}

/// What one `requestMaterials` call does next.
///
/// Java reference: `:232-343` — the switch on the stage, then the `FINISHED`
/// handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the four exits of one request step
pub enum Step {
    /// The step walked its block and found nothing finished: stay in the stage
    /// (`:322-328`, then `return false`).
    Stay,
    /// `FINISHED` in a stage that has a successor (`:329-341`): move on, the next
    /// call walks the next stage.
    Next(RequestStage),
    /// `FINISHED` in `ENTITIES` (`:332-339`): the pass is over **and** restarts at
    /// `SOLID_FRAME` with the material list reset, because placing the blocks changed
    /// what the blueprint wants. That second pass is what makes `requested` true
    /// (`:212-215`).
    Wrap,
    /// The walk is over with nothing left to ask (`:317-320`, and the removal /
    /// infinite-resources exits of `:205-209`).
    Done,
}

/// `requestMaterialsState`'s first question (`:205-209`): is there anything to
/// request **at all**?
///
/// Three reasons to answer no, and they are different events:
/// the config allows infinite resources, the work order already asked once, or the
/// order is a deconstruction (nothing comes back).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the three ways the request is skipped
pub enum Skip {
    /// `Constants.BUILDER_INF_RESOURECES` (`:205`): the pack asks for nothing.
    InfiniteResources,
    /// `building.getWorkOrder().isRequested()` (`:205`): a previous pass already
    /// filled the list.
    AlreadyRequested,
    /// `getWorkOrderType() == REMOVE` (`:205`): a deconstruction builds nothing.
    RemovalOrder,
}

/// Does this `LOAD_STRUCTURE` pass request materials?
pub fn requests(
    infinite_resources: bool,
    already_requested: bool,
    removal: bool,
) -> Result<(), Skip> {
    if infinite_resources {
        return Err(Skip::InfiniteResources);
    }
    if already_requested {
        return Err(Skip::AlreadyRequested);
    }
    if removal {
        return Err(Skip::RemovalOrder);
    }
    Ok(())
}

/// Where `requestMaterials` starts (`:256-259` and `:264-278`).
///
/// A first request always resets the hut's resource list and starts at
/// `SOLID_FRAME` (`:256-259`). But a building that is already standing — level 2
/// and above — has no frame to ask for (`:265-269`), and a blueprint without frame
/// tags has none either (`:270-278`): both start at `SOLID`.
pub fn first_stage(level: i32, has_frame_tags: bool) -> RequestStage {
    if level > 1 || !has_frame_tags {
        RequestStage::Solid
    } else {
        RequestStage::SolidFrame
    }
}

/// One `requestMaterials` step's outcome (`:262-343`).
///
/// `finished` is `result.getBlockResult().getResult() == FINISHED`
/// (`:329`).
pub fn advance(stage: RequestStage, finished: bool) -> Step {
    if !finished {
        return Step::Stay; // `:329`
    }
    if stage == RequestStage::Entities {
        // `:332-339` — reset and start over; the caller sets `recalculated`.
        return Step::Wrap;
    }
    match stage.next() {
        Some(next) => Step::Next(next), // `:340`
        None => Step::Done,
    }
}

/// The position the request walk stores between two calls
/// (`requestProgress`, `:324`), and the stage it is in.
///
/// Kept as one record because the two are always written together (`:322`,
/// `:331`), and a caller that stored one without the other would resume in the
/// wrong stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// `requestProgress`: where the iterator stopped, `None` before the first step.
    pub at: Option<Pos>,
    /// `requestState`.
    pub stage: RequestStage,
}

impl Progress {
    /// A request that has not started (`:253-259`).
    pub fn new() -> Progress {
        Progress {
            at: None,
            stage: RequestStage::SolidFrame,
        }
    }

    /// Apply one step's answer.
    pub fn step(self, step: Step) -> Progress {
        match step {
            Step::Stay => Progress {
                at: self.at,
                stage: self.stage,
            },
            Step::Next(stage) => Progress { at: None, stage },
            Step::Wrap => Progress {
                at: None,
                stage: RequestStage::SolidFrame,
            },
            Step::Done => Progress {
                at: None,
                stage: self.stage,
            },
        }
    }
}

impl Default for Progress {
    fn default() -> Progress {
        Progress::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: &str) -> NeedKey {
        NeedKey::plain(id)
    }

    /// `:824`: a free block never reaches the request list, so a blueprint of water
    /// and leaves asks for nothing.
    #[test]
    fn a_free_block_is_never_asked_for() {
        let needs = vec![
            Need::new(key("minecraft:water"), 1).free(),
            Need::new(key("minecraft:oak_leaves"), 6).free(),
        ];
        assert_eq!(resolve(&needs, false), Resolution::Success);
        assert!(is_block_free(BlockFacts::water())); // :1003
        assert!(is_block_free(BlockFacts::leaves())); // :1004
        assert!(is_block_free(BlockFacts::air())); // :1002, the `null` case
        assert!(!is_block_free(BlockFacts::solid()));
    }

    /// `:838-845`: a stack that is neither carried nor in the bucket invalidates the
    /// list. That is the `RECALC` that sends the builder back to `LOAD_STRUCTURE`
    /// (`:509-512`).
    #[test]
    fn a_stack_nowhere_asks_for_a_recalculation() {
        let needs = vec![Need::new(key("minecraft:oak_planks"), 4)];
        assert_eq!(resolve(&needs, false), Resolution::Recalculate);
    }

    /// The bucket is enough to pass the *location* test (`:841`) — but not to place
    /// the block: the non-force pass drops nothing (the worker carries none), so the
    /// list still ends on that stack and the builder goes to `NEEDS_ITEM` while
    /// `PICK_UP` fetches it from the hut. What the bucket prevents is the `RECALC`.
    #[test]
    fn the_huts_bucket_prevents_a_recalculation_but_not_the_request() {
        let needs = vec![Need::new(key("minecraft:oak_planks"), 4).in_bucket()];
        assert_eq!(
            resolve(&needs, false),
            Resolution::Failed(Failed {
                need: key("minecraft:oak_planks"),
                because: Because::RequestCreated,
            })
        );
    }

    /// `:880-886`, the non-force branch: one carried item drops the entry whatever
    /// the count. Four planks wanted, one carried, and the answer is `SUCCESS`.
    #[test]
    fn the_non_force_pass_ignores_the_count() {
        let needs = vec![Need::new(key("minecraft:oak_planks"), 4).carried(1)];
        assert_eq!(resolve(&needs, false), Resolution::Success);
    }

    /// `:862-879`, the force branch: the same facts, `force = true`, and the three
    /// missing planks come back as a `FAIL` with a request to create.
    #[test]
    fn the_force_pass_subtracts_what_is_carried() {
        let needs = vec![Need::new(key("minecraft:oak_planks"), 4).carried(1)];
        assert_eq!(
            resolve(&needs, true),
            Resolution::Failed(Failed {
                need: key("minecraft:oak_planks"),
                because: Because::RequestCreated,
            })
        );
        // Covered exactly: nothing left to ask for.
        let covered = vec![Need::new(key("minecraft:oak_planks"), 1).carried(1)];
        assert_eq!(resolve(&covered, true), Resolution::Success);
    }

    /// `:901-907` against `:908-919`: the difference between "create a request" and
    /// "move the request you already have into the sync list" is a fact, and both
    /// answer `FAIL`. Every case carries one item, because `:838-845` would answer
    /// `RECALC` for a stack that is nowhere at all.
    #[test]
    fn an_open_or_completed_request_is_named_apart_from_a_created_one() {
        let open = vec![Need::new(key("minecraft:stone"), 8)
            .carried(1)
            .opened(1)];
        assert_eq!(
            resolve(&open, true),
            Resolution::Failed(Failed {
                need: key("minecraft:stone"),
                because: Because::RequestAlreadyOpen,
            })
        );
        let completed = vec![Need::new(key("minecraft:stone"), 8)
            .carried(1)
            .completed(2)];
        assert_eq!(
            resolve(&completed, true),
            Resolution::Failed(Failed {
                need: key("minecraft:stone"),
                because: Because::RequestAlreadyOpen,
            })
        );
        // And with neither, the request is the one to create.
        let none = vec![Need::new(key("minecraft:stone"), 8).carried(1)];
        match resolve(&none, true) {
            Resolution::Failed(failed) => assert_eq!(failed.because, Because::RequestCreated),
            other => panic!("expected a created request, got {other:?}"),
        }
    }

    /// `:890-897`: an entry whose stack is empty stops the builder without a
    /// request. Java has no message for it — the port names the reason.
    #[test]
    fn an_empty_stack_fails_without_a_request() {
        let mut need = Need::new(key("minecraft:barrier"), 0);
        need.empty = true;
        need.in_inventory = true;
        assert_eq!(
            resolve(&[need], false),
            Resolution::Failed(Failed {
                need: key("minecraft:barrier"),
                because: Because::EmptyStack,
            })
        );
    }

    /// `:829-836`: two stacks of the same identity are one entry whose amount is the
    /// sum, so `force` subtracts from the total.
    #[test]
    fn a_duplicate_stack_is_merged_and_its_amounts_added() {
        let needs = vec![
            Need::new(key("minecraft:cobblestone"), 3).carried(4),
            Need::new(key("minecraft:cobblestone"), 5),
        ];
        // 3 + 5 = 8 wanted, 4 carried → 4 still missing.
        assert_eq!(
            resolve(&needs, true),
            Resolution::Failed(Failed {
                need: key("minecraft:cobblestone"),
                because: Because::RequestCreated,
            })
        );
        let exact = vec![
            Need::new(key("minecraft:cobblestone"), 3).carried(2),
            Need::new(key("minecraft:cobblestone"), 1).carried(2),
        ];
        assert_eq!(resolve(&exact, true), Resolution::Success);
    }

    /// The key is Java's `descriptionId + "-" + hashCode` (`:349`), and the two
    /// halves stay visible.
    #[test]
    fn the_need_key_is_the_java_string_without_the_hash() {
        let tagged = NeedKey {
            description_id: "minecraft:oak_planks".to_string(),
            tag_hash: 1837465,
        };
        assert_eq!(tagged.to_string(), "minecraft:oak_planks-1837465");
        assert_eq!(NeedKey::plain("minecraft:dirt").to_string(), "minecraft:dirt-0");
        assert_ne!(tagged, NeedKey::plain("minecraft:oak_planks"));
    }

    /// `:346-353`: a missing stack is registered once, with an amount of 1.
    #[test]
    fn a_block_is_registered_as_needed_once() {
        let known = vec![key("minecraft:oak_planks")];
        assert!(!needs_registration(&known, &key("minecraft:oak_planks")));
        assert!(needs_registration(&known, &key("minecraft:stone")));
        assert_eq!(registration(&key("minecraft:stone")).count, 1);
    }

    /// `:511-537`: a known resource is clamped to a stack, an unknown one is asked
    /// for as the stack says.
    #[test]
    fn the_request_amount_is_clamped_to_a_stack() {
        assert_eq!(total_amount(Some(3), 9), 3);
        assert_eq!(total_amount(Some(200), 9), STACKSIZE); // `Math.min(STACKSIZE, ...)`
        assert_eq!(total_amount(None, 9), 9);
        assert_eq!(STACKSIZE, 64);
    }

    /// `:216-220`: the resource count of the order only grows.
    #[test]
    fn the_orders_resource_count_only_grows() {
        assert_eq!(amount_of_resources(0, 12), 12); // first pass
        assert_eq!(amount_of_resources(12, 30), 30); // a second pass found more
        assert_eq!(amount_of_resources(30, 12), 30); // never shrinks
    }

    /// `:473-477`: using one item decreases the need, and the entry disappears at
    /// zero — which is what makes the material list empty again
    /// (`:213-215`).
    #[test]
    fn using_one_item_decreases_the_need_and_removes_it_at_zero() {
        assert_eq!(reduced(3), Some(2));
        assert_eq!(reduced(1), None);
        assert_eq!(reduced(0), None);
    }

    /// `:205-209`: three separate reasons to ask for nothing, named apart.
    #[test]
    fn three_reasons_skip_the_material_request() {
        assert_eq!(requests(true, false, false), Err(Skip::InfiniteResources));
        assert_eq!(requests(false, true, false), Err(Skip::AlreadyRequested));
        assert_eq!(requests(false, false, true), Err(Skip::RemovalOrder));
        assert_eq!(requests(false, false, false), Ok(()));
    }

    /// `:264-278`: a building at level 2 has no frame to ask for, and neither has a
    /// blueprint without frame tags.
    #[test]
    fn the_frame_stage_is_skipped_for_a_standing_building_or_a_flat_blueprint() {
        assert_eq!(first_stage(0, true), RequestStage::SolidFrame);
        assert_eq!(first_stage(1, true), RequestStage::SolidFrame);
        assert_eq!(first_stage(2, true), RequestStage::Solid); // `:265-269`
        assert_eq!(first_stage(0, false), RequestStage::Solid); // `:270-278`
    }

    /// `:329-340`: a stage that is not finished stays, a finished one moves on, and
    /// `ENTITIES` wraps back to the frame (`:332-339`).
    #[test]
    fn the_request_walks_its_five_stages_and_wraps_once() {
        assert_eq!(advance(RequestStage::Solid, false), Step::Stay);
        assert_eq!(
            advance(RequestStage::SolidFrame, true),
            Step::Next(RequestStage::Solid)
        );
        assert_eq!(
            advance(RequestStage::Solid, true),
            Step::Next(RequestStage::WeakSolid)
        );
        assert_eq!(
            advance(RequestStage::Deco, true),
            Step::Next(RequestStage::Entities)
        );
        assert_eq!(advance(RequestStage::Entities, true), Step::Wrap);
        // The declaration order is the walking order.
        let mut order = Vec::new();
        let mut stage = RequestStage::SolidFrame;
        loop {
            order.push(stage);
            match stage.next() {
                Some(next) => stage = next,
                None => break,
            }
        }
        assert_eq!(order, RequestStage::ORDER);
    }

    /// A wrapped step forgets where it stopped and starts the frame again, which is
    /// the second pass of `:332-339`.
    #[test]
    fn the_progress_restarts_at_the_frame_after_the_entity_pass() {
        let progress = Progress {
            at: Some(Pos::new(4, 70, 4)),
            stage: RequestStage::Entities,
        };
        let wrapped = progress.step(Step::Wrap);
        assert_eq!(wrapped.at, None);
        assert_eq!(wrapped.stage, RequestStage::SolidFrame);
        let next = progress.step(Step::Next(RequestStage::Deco));
        assert_eq!(next.stage, RequestStage::Deco);
        assert_eq!(next.at, None);
        // A step that stayed keeps both halves.
        let stayed = progress.step(Step::Stay);
        assert_eq!(stayed, progress);
        assert_eq!(Progress::new().stage, RequestStage::SolidFrame); // `:256-259`
    }

    /// The three Java names are the three answers, so a log line can print the one
    /// that happened.
    #[test]
    fn each_answer_has_its_java_name() {
        assert_eq!(Because::EmptyStack.java_name(), "FAIL (empty stack)");
        assert_eq!(Because::RequestCreated.java_name(), "FAIL (request created)");
        assert_eq!(
            Because::RequestAlreadyOpen.java_name(),
            "FAIL (request already open)"
        );
        assert!(SOURCE.contains("AbstractEntityAIStructure.java:816-923"));
        assert!(SOURCE.contains(":232-343"));
    }
}
