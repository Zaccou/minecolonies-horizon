//! Les onze états que l'enchanteur, le guérisseur et le croque-mort ajoutent à
//! `AIWorkerState`, le drapeau de chacun, et les refus qu'ils nomment.
//!
//! Référence Java : `api/entity/ai/statemachine/states/AIWorkerState.java`, une
//! énumération pour **tous** les métiers (127 valeurs). Le port garde cette forme —
//! [`ServiceState`] est une énumération unique, pas une par métier — parce que les
//! tables Java sont écrites contre l'énumération unique : `decide` de l'enchanteur rend
//! `ENCHANT` (`EntityAIWorkEnchanter.java:204`) et `START_WORKING` (`:126`) depuis la
//! même méthode.
//!
//! **Pourquoi `Base(...)`.** `crate::ai::state::WorkerState` nomme déjà les dix-huit
//! états de base de ce port, et cette énumération appartient à `crate::ai` (ce module
//! n'écrit que dans `src/ai_work_12`). Un état de base est donc [`ServiceState::Base`]
//! enveloppant le `WorkerState` connu, et [`ServiceState::worker`] le rend : aucun état
//! n'est écrit deux fois.
//!
//! **Onze états et non douze.** `WANDER` (`:613`) est un seul état de `AIWorkerState`
//! que le guérisseur (`EntityAIWorkHealer.java:93`) et le croque-mort
//! (`EntityAIWorkUndertaker.java:92`) enregistrent tous les deux ; il n'est donc nommé
//! qu'une fois ici.

use core::fmt;

use crate::ai::state::WorkerState;

/// Un état de la table d'un de ces trois métiers : un état de base rendu à
/// `crate::ai`, ou un état que ce module nomme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
// exhaustif : les sept états de base que ces tables traversent plus les onze du métier
pub enum ServiceState {
    /// Un état de `crate::ai::state::WorkerState`.
    Base(WorkerState),
    /// `ENCHANTER_DRAIN` (`:599`) — aller au bâtiment à drainer et en tirer de l'XP
    /// (`EntityAIWorkEnchanter.java:306-433`).
    EnchanterDrain,
    /// `ENCHANT` (`:604`) — fabriquer un livre enchanté depuis un tome ancien
    /// (`EntityAIWorkEnchanter.java:225-288`).
    Enchant,
    /// `REQUEST_CURE` (`:609`) — demander les remèdes du patient (`EntityAIWorkHealer.java:241-304`).
    RequestCure,
    /// `CURE` (`:611`) — donner les remèdes au patient (`EntityAIWorkHealer.java:311-383`).
    Cure,
    /// `WANDER` (`:613`) — marcher vers un patient lointain ; aussi le repos du
    /// croque-mort (`EntityAIWorkUndertaker.java:153-168`).
    Wander,
    /// `FREE_CURE` (`:615`) — guérison magique sans remèdes (`EntityAIWorkHealer.java:390-438`).
    FreeCure,
    /// `CURE_PLAYER` (`:617`) — soigner un joueur blessé (`EntityAIWorkHealer.java:445-467`).
    CurePlayer,
    /// `EMPTY_GRAVE` (`:169`) — vider une tombe dans l'inventaire du cimetière
    /// (`EntityAIWorkUndertaker.java:176-226`).
    EmptyGrave,
    /// `DIG_GRAVE` (`:174`) — casser la tombe (`EntityAIWorkUndertaker.java:233-277`).
    DigGrave,
    /// `BURY_CITIZEN` (`:179`) — poser une tombe nommée (`EntityAIWorkUndertaker.java:440-499`).
    BuryCitizen,
    /// `TRY_RESURRECT` (`:184`) — tenter une résurrection (`EntityAIWorkUndertaker.java:309-379`).
    TryResurrect,
}

impl ServiceState {
    /// Les dix-huit états de ce module : sept de base dans l'ordre de `WorkerState`,
    /// puis les onze du métier dans l'ordre de `AIWorkerState`.
    pub const ALL: [ServiceState; 18] = [
        ServiceState::Base(WorkerState::Idle),
        ServiceState::Base(WorkerState::InventoryFull),
        ServiceState::Base(WorkerState::StartWorking),
        ServiceState::Base(WorkerState::NeedsItem),
        ServiceState::Base(WorkerState::Decide),
        ServiceState::Base(WorkerState::Paused),
        ServiceState::Base(WorkerState::GatheringRequiredMaterials),
        ServiceState::EmptyGrave,
        ServiceState::DigGrave,
        ServiceState::BuryCitizen,
        ServiceState::TryResurrect,
        ServiceState::EnchanterDrain,
        ServiceState::Enchant,
        ServiceState::RequestCure,
        ServiceState::Cure,
        ServiceState::Wander,
        ServiceState::FreeCure,
        ServiceState::CurePlayer,
    ];

    /// L'identifiant Java, celui que montre l'historique d'états et le panneau du
    /// citoyen (`AbstractEntityAIBasic.java:475`).
    pub fn java(self) -> &'static str {
        match self {
            ServiceState::Base(state) => state.java_name(),
            ServiceState::EnchanterDrain => "ENCHANTER_DRAIN",
            ServiceState::Enchant => "ENCHANT",
            ServiceState::RequestCure => "REQUEST_CURE",
            ServiceState::Cure => "CURE",
            ServiceState::Wander => "WANDER",
            ServiceState::FreeCure => "FREE_CURE",
            ServiceState::CurePlayer => "CURE_PLAYER",
            ServiceState::EmptyGrave => "EMPTY_GRAVE",
            ServiceState::DigGrave => "DIG_GRAVE",
            ServiceState::BuryCitizen => "BURY_CITIZEN",
            ServiceState::TryResurrect => "TRY_RESURRECT",
        }
    }

    /// La ligne qui déclare cet état dans `AIWorkerState.java`.
    pub fn java_declaration(self) -> &'static str {
        match self {
            ServiceState::Base(_) => "crate::ai::state::WorkerState (voir AIWorkerState.java)",
            ServiceState::EnchanterDrain => "AIWorkerState.java:599",
            ServiceState::Enchant => "AIWorkerState.java:604",
            ServiceState::RequestCure => "AIWorkerState.java:609",
            ServiceState::Cure => "AIWorkerState.java:611",
            ServiceState::Wander => "AIWorkerState.java:613",
            ServiceState::FreeCure => "AIWorkerState.java:615",
            ServiceState::CurePlayer => "AIWorkerState.java:617",
            ServiceState::EmptyGrave => "AIWorkerState.java:169",
            ServiceState::DigGrave => "AIWorkerState.java:174",
            ServiceState::BuryCitizen => "AIWorkerState.java:179",
            ServiceState::TryResurrect => "AIWorkerState.java:184",
        }
    }

    /// `AIWorkerState.isOkayToEat` : le drapeau que lit
    /// `AbstractAISkeleton.canBeInterrupted()` (`AbstractAISkeleton.java:126-131`) et
    /// que `inventoryNeedsDump` consulte (`AbstractEntityAIBasic.java:411`).
    ///
    /// `ENCHANT(false)` (`:604`) ne peut pas être interrompu ; `ENCHANTER_DRAIN(true)`
    /// (`:599`) le peut, et les six états du guérisseur plus `WANDER` aussi. Les quatre
    /// états du croque-mort sont tous `false` (`:169`, `:174`, `:179`, `:184`) : une
    /// pelletée commencée se finit.
    pub fn okay_to_eat(self) -> bool {
        match self {
            ServiceState::Base(state) => state.okay_to_eat(),
            ServiceState::EnchanterDrain
            | ServiceState::RequestCure
            | ServiceState::Cure
            | ServiceState::Wander
            | ServiceState::FreeCure
            | ServiceState::CurePlayer => true,
            ServiceState::Enchant
            | ServiceState::EmptyGrave
            | ServiceState::DigGrave
            | ServiceState::BuryCitizen
            | ServiceState::TryResurrect => false,
        }
    }

    /// Le `WorkerState` derrière un état de base, `None` pour un état de métier.
    pub fn worker(self) -> Option<WorkerState> {
        match self {
            ServiceState::Base(state) => Some(state),
            _ => None,
        }
    }

    /// Cet état appartient-il à la table de `trade` ? Les états de base n'appartiennent
    /// à personne : ils sont à `crate::ai`.
    pub fn belongs_to(self, trade: super::ServiceTrade) -> bool {
        use super::ServiceTrade;
        match trade {
            ServiceTrade::Enchanter => matches!(
                self,
                ServiceState::EnchanterDrain | ServiceState::Enchant
            ),
            ServiceTrade::Healer => matches!(
                self,
                ServiceState::RequestCure
                    | ServiceState::Cure
                    | ServiceState::FreeCure
                    | ServiceState::CurePlayer
            ),
            ServiceTrade::Undertaker => matches!(
                self,
                ServiceState::EmptyGrave
                    | ServiceState::DigGrave
                    | ServiceState::BuryCitizen
                    | ServiceState::TryResurrect
            ),
        }
    }
}

impl fmt::Display for ServiceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java())
    }
}

/// Pourquoi un de ces métiers ne va pas plus loin, nommé.
///
/// Java n'a pas ce type : chaque branche qui ne peut pas avancer fait `return getState()`
/// (ou `return IDLE`) et la raison reste dans la forme du `if`. Un joueur qui demande
/// « pourquoi mon croque-mort tourne en rond » en a besoin, et un test qui vérifie une
/// suite d'états doit savoir *quelle* branche a tenu : chaque variante porte sa ligne.
///
/// **Un refus n'est pas un échec.** [`ServiceRefusal::in_progress`] nomme ceux qui
/// accompagnent un travail en cours (un drain, une pelletée, une guérison magique) : le
/// tick agit, il attend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
// exhaustif : une variante par branche des quatorze méthodes portées
pub enum ServiceRefusal {
    /// Le scénario ne porte pas les faits de ce métier : erreur d'appelant, jamais un
    /// état de Java. Nommée pour ne pas ressembler à « rien à faire ».
    NoFactsForTrade,
    // ---------------------------------------------------------- enchanteur
    /// `!walkToBuilding()` (`EntityAIWorkEnchanter.java:124-127`) ou
    /// `!walkToBuilding(buildingWorker)` (`:314-317`) : le citoyen marche encore.
    NotAtHut,
    /// `getNextCraftingState()` a rendu un état d'atelier et la journée n'a pas passé
    /// 6 000 ticks (`:129-133`) : l'atelier du mod a la main.
    CraftingDue,
    /// `wantInventoryDumped()` (`:135-139`). **Branche morte en Java** : l'enchanteur
    /// n'écrase pas `wantInventoryDumped()`, qui rend `false`
    /// (`AbstractEntityAIBasic.java:435-438`).
    DumpDue,
    /// `module.getBuildingsToGatherFrom().isEmpty()` (`:145-153`) : aucune station à
    /// drainer n'est réglée, et le citoyen le dit.
    NoDrainStation,
    /// Aucun livre, ni dans l'inventaire ni dans la hutte (`:155-166`) : une requête
    /// est créée.
    NoBook,
    /// Le livre est dans les coffres de la hutte (`:158-163`) : aller le chercher.
    BookInHutOnly,
    /// `module.getRandomBuildingToDrainFrom() == null` (`:168-171`) : toutes les
    /// stations sont déjà drainées.
    NoGatherableBuilding,
    /// Aucun tome ancien, ni dans l'inventaire ni dans la hutte (`:190-200`).
    NoAncientTome,
    /// Le tome est dans les coffres de la hutte (`:193-198`) : aller le chercher.
    AncientTomeInHutOnly,
    /// `job.getPosToDrainFrom() == null` (`:308-311`) : rien à drainer.
    NoDrainTarget,
    /// Le bâtiment à drainer n'existe plus (`:319-324`) : la station est retirée.
    DrainBuildingGone,
    /// Aucun citoyen affecté au bâtiment (`:341-345`) : le drain est abandonné.
    NoWorkerToDrain,
    /// Le citoyen choisi n'a plus d'entité (`:354-358`) : le choix est oublié.
    DrainTargetAbsent,
    /// Trop loin du citoyen à drainer (`:363-371`) : attendre, un tick de plus.
    DrainTooFar,
    /// Les soixante ticks d'attente sont écoulés (`:365-369`,
    /// `JobEnchanter.java:22`) : le drain est abandonné.
    DrainWalkGaveUp,
    /// `progressTicks < MAX_PROGRESS_TICKS` (`:374-406`) : le drain est en cours.
    Draining,
    /// Aucun livre en main au bout du drain (`:408-430`) : l'XP et la mana ne sont pas
    /// payés, le drain se termine quand même.
    NoBookToSpend,
    /// `getFirstFulfillableRecipe(...)` a rendu `null` (`:228-233`) : pas de recette de
    /// livre enchanté, retour au travail.
    NoEnchantingRecipe,
    /// `progressTicks < MAX_ENCHANTMENT_TICKS / niveau` (`:235-266`) : l'enchantement
    /// est en cours.
    Enchanting,
    // ------------------------------------------------------------- guérisseur
    /// `!walkToBuilding()` (`EntityAIWorkHealer.java:106-109`).
    NotAtHospital,
    /// Aucun patient à soigner, aucun joueur blessé, aucun citoyen lointain
    /// (`:233`) : rester dans `DECIDE`.
    NoPatient,
    /// Le dossier de patient ne correspond plus à un citoyen malade (`:121-125`,
    /// `:248-253`, `:318-323`) : le dossier est retiré.
    PatientGone,
    /// L'inventaire du patient est plein (`:149`, `:195-198`, `:363-368`) : le
    /// guérisseur le signale et passe au suivant.
    PatientInventoryFull,
    /// Ni l'inventaire du guérisseur ni celui de la hutte ne portent les remèdes
    /// (`:340-355`, `:274-275`), ou le patient traité ne les porte plus (`:209-213`).
    CureMissing,
    /// Les remèdes sont dans la hutte (`:164-169`, `:342-352`) : aller les chercher.
    CureInHutOnly,
    /// Aucune requête de ce remède n'existait (`:187-191`, `:294-298`) : elle est
    /// créée et le patient repasse à `NEW`.
    CureRequested,
    /// Une requête de ce remède est déjà ouverte ou servie (`:170-186`) : le patient
    /// repasse à `NEW` sans nouvelle demande.
    CureAlreadyRequested,
    /// `walkToSafePos` / `walkToUnSafePos` a rendu faux (`:256-259`, `:326-329`,
    /// `:405-409`, `:458-461`, `:488-491`).
    NotAtPatient,
    /// `progressTicks < MAX_PROGRESS_TICKS` (`:411-429`) : la guérison magique est en
    /// cours.
    FreeCureRunning,
    /// `playerToHeal == null` (`:447-450`).
    NoPlayerToHeal,
    /// `playerToHeal.blockPosition().distSqr(building.getPosition()) > 50 * 50`
    /// (`:452-456`).
    PlayerOutOfRange,
    /// `remotePatient == null` ou sans entité (`:482-485`).
    NoRemotePatient,
    /// `disease == null` (`:332-338`) : le guérisseur rend dix points de vie sans
    /// remède, et paie l'XP.
    HealedWithoutCure,
    // ----------------------------------------------------------- croque-mort
    /// `checkForToolOrWeapon(shovel)` a rendu vrai (`:180`, `:237`, `:288`, `:313`,
    /// `:445`) : pas de pelle assez bonne, une requête est ouverte, le métier est
    /// `STUCK`.
    ShovelMissing,
    /// `building.getGraveToWorkOn() == null` (`:180-183`, `:237-240`).
    NoGraveToWorkOn,
    /// Le bloc de la tombe n'est pas un `TileEntityGrave` (`:225`, `:276`) : la tombe
    /// courante est oubliée.
    NotAGrave,
    /// `walkWithProxy(gravePos, 3)` a rendu vrai (`:192-195`, `:253-256`, `:329-332`) :
    /// le citoyen marche encore.
    NotAtGrave,
    /// `effortCounter < EFFORT_*` (`:210-215`, `:337-345`, `:475-481`) ou
    /// `digIfAble` a rendu faux (`:264-267`) : le geste est en cours.
    EffortPending,
    /// `worker.getInventoryCitizen().isFull()` (`:205-208`) : aller vider.
    WorkerInventoryFull,
    /// `chance < random.nextDouble()` (`:378`) : la résurrection a échoué, la tombe
    /// sera creusée.
    ResurrectFailed,
    /// Aucune place libre dans le cimetière (`:456-468`) : le joueur est prévenu et la
    /// donnée de tombe est oubliée.
    NoBurialSpot,
    /// `module.getLastGraveData() == null` (`:445-448`) : aucune tombe à poser.
    NoDiggedGrave,
    /// `walkWithProxy(burialPos, 4)` a rendu vrai (`:470-473`).
    NotAtBurial,
    /// `module.buryCitizenHere(...)` a rendu faux (`:485-489`).
    BurialFailed,
}

impl ServiceRefusal {
    /// La ligne Java qui décide ce refus.
    pub fn java(self) -> &'static str {
        match self {
            ServiceRefusal::NoFactsForTrade => "port: src/ai_work_12, pas une branche Java",
            ServiceRefusal::NotAtHut => "EntityAIWorkEnchanter.java:124-127,:314-317",
            ServiceRefusal::CraftingDue => "EntityAIWorkEnchanter.java:129-133",
            ServiceRefusal::DumpDue => "EntityAIWorkEnchanter.java:135-139",
            ServiceRefusal::NoDrainStation => "EntityAIWorkEnchanter.java:145-153",
            ServiceRefusal::NoBook => "EntityAIWorkEnchanter.java:155-166",
            ServiceRefusal::BookInHutOnly => "EntityAIWorkEnchanter.java:158-163",
            ServiceRefusal::NoGatherableBuilding => "EntityAIWorkEnchanter.java:168-171",
            ServiceRefusal::NoAncientTome => "EntityAIWorkEnchanter.java:190-200",
            ServiceRefusal::AncientTomeInHutOnly => "EntityAIWorkEnchanter.java:193-198",
            ServiceRefusal::NoDrainTarget => "EntityAIWorkEnchanter.java:308-311",
            ServiceRefusal::DrainBuildingGone => "EntityAIWorkEnchanter.java:319-324",
            ServiceRefusal::NoWorkerToDrain => "EntityAIWorkEnchanter.java:341-345",
            ServiceRefusal::DrainTargetAbsent => "EntityAIWorkEnchanter.java:354-358",
            ServiceRefusal::DrainTooFar => "EntityAIWorkEnchanter.java:363-371",
            ServiceRefusal::DrainWalkGaveUp => {
                "EntityAIWorkEnchanter.java:365-369 / JobEnchanter.java:22"
            }
            ServiceRefusal::Draining => "EntityAIWorkEnchanter.java:374-406",
            ServiceRefusal::NoBookToSpend => "EntityAIWorkEnchanter.java:408-430",
            ServiceRefusal::NoEnchantingRecipe => "EntityAIWorkEnchanter.java:228-233",
            ServiceRefusal::Enchanting => "EntityAIWorkEnchanter.java:235-266",
            ServiceRefusal::NotAtHospital => "EntityAIWorkHealer.java:106-109",
            ServiceRefusal::NoPatient => "EntityAIWorkHealer.java:233",
            ServiceRefusal::PatientGone => "EntityAIWorkHealer.java:121-125,:248-253,:318-323",
            ServiceRefusal::PatientInventoryFull => {
                "EntityAIWorkHealer.java:149,:195-198,:363-368"
            }
            ServiceRefusal::CureMissing => "EntityAIWorkHealer.java:209-213,:340-355",
            ServiceRefusal::CureInHutOnly => "EntityAIWorkHealer.java:164-169,:342-352",
            ServiceRefusal::CureRequested => "EntityAIWorkHealer.java:187-191,:294-298",
            ServiceRefusal::CureAlreadyRequested => "EntityAIWorkHealer.java:170-186",
            ServiceRefusal::NotAtPatient => {
                "EntityAIWorkHealer.java:256-259,:326-329,:405-409,:458-461,:488-491"
            }
            ServiceRefusal::FreeCureRunning => "EntityAIWorkHealer.java:411-429",
            ServiceRefusal::NoPlayerToHeal => "EntityAIWorkHealer.java:447-450",
            ServiceRefusal::PlayerOutOfRange => "EntityAIWorkHealer.java:452-456",
            ServiceRefusal::NoRemotePatient => "EntityAIWorkHealer.java:482-485",
            ServiceRefusal::HealedWithoutCure => "EntityAIWorkHealer.java:332-338",
            ServiceRefusal::ShovelMissing => "EntityAIWorkUndertaker.java:180,:237,:288,:313,:445",
            ServiceRefusal::NoGraveToWorkOn => "EntityAIWorkUndertaker.java:180-183,:237-240",
            ServiceRefusal::NotAGrave => "EntityAIWorkUndertaker.java:225,:276",
            ServiceRefusal::NotAtGrave => "EntityAIWorkUndertaker.java:192-195,:329-332",
            ServiceRefusal::EffortPending => {
                "EntityAIWorkUndertaker.java:210-215,:264-267,:337-345,:475-481"
            }
            ServiceRefusal::WorkerInventoryFull => "EntityAIWorkUndertaker.java:205-208",
            ServiceRefusal::ResurrectFailed => "EntityAIWorkUndertaker.java:378",
            ServiceRefusal::NoBurialSpot => "EntityAIWorkUndertaker.java:456-468",
            ServiceRefusal::NoDiggedGrave => "EntityAIWorkUndertaker.java:445-448",
            ServiceRefusal::NotAtBurial => "EntityAIWorkUndertaker.java:470-473",
            ServiceRefusal::BurialFailed => "EntityAIWorkUndertaker.java:485-489",
        }
    }

    /// Une clé stable pour le message qu'un joueur lira.
    pub fn key(self) -> &'static str {
        match self {
            ServiceRefusal::NoFactsForTrade => "ai_work_12.refusal.no_facts_for_trade",
            ServiceRefusal::NotAtHut => "ai_work_12.refusal.not_at_hut",
            ServiceRefusal::CraftingDue => "ai_work_12.refusal.crafting_due",
            ServiceRefusal::DumpDue => "ai_work_12.refusal.dump_due",
            ServiceRefusal::NoDrainStation => "ai_work_12.refusal.no_drain_station",
            ServiceRefusal::NoBook => "ai_work_12.refusal.no_book",
            ServiceRefusal::BookInHutOnly => "ai_work_12.refusal.book_in_hut_only",
            ServiceRefusal::NoGatherableBuilding => "ai_work_12.refusal.no_gatherable_building",
            ServiceRefusal::NoAncientTome => "ai_work_12.refusal.no_ancient_tome",
            ServiceRefusal::AncientTomeInHutOnly => {
                "ai_work_12.refusal.ancient_tome_in_hut_only"
            }
            ServiceRefusal::NoDrainTarget => "ai_work_12.refusal.no_drain_target",
            ServiceRefusal::DrainBuildingGone => "ai_work_12.refusal.drain_building_gone",
            ServiceRefusal::NoWorkerToDrain => "ai_work_12.refusal.no_worker_to_drain",
            ServiceRefusal::DrainTargetAbsent => "ai_work_12.refusal.drain_target_absent",
            ServiceRefusal::DrainTooFar => "ai_work_12.refusal.drain_too_far",
            ServiceRefusal::DrainWalkGaveUp => "ai_work_12.refusal.drain_walk_gave_up",
            ServiceRefusal::Draining => "ai_work_12.refusal.draining",
            ServiceRefusal::NoBookToSpend => "ai_work_12.refusal.no_book_to_spend",
            ServiceRefusal::NoEnchantingRecipe => "ai_work_12.refusal.no_enchanting_recipe",
            ServiceRefusal::Enchanting => "ai_work_12.refusal.enchanting",
            ServiceRefusal::NotAtHospital => "ai_work_12.refusal.not_at_hospital",
            ServiceRefusal::NoPatient => "ai_work_12.refusal.no_patient",
            ServiceRefusal::PatientGone => "ai_work_12.refusal.patient_gone",
            ServiceRefusal::PatientInventoryFull => "ai_work_12.refusal.patient_inventory_full",
            ServiceRefusal::CureMissing => "ai_work_12.refusal.cure_missing",
            ServiceRefusal::CureInHutOnly => "ai_work_12.refusal.cure_in_hut_only",
            ServiceRefusal::CureRequested => "ai_work_12.refusal.cure_requested",
            ServiceRefusal::CureAlreadyRequested => "ai_work_12.refusal.cure_already_requested",
            ServiceRefusal::NotAtPatient => "ai_work_12.refusal.not_at_patient",
            ServiceRefusal::FreeCureRunning => "ai_work_12.refusal.free_cure_running",
            ServiceRefusal::NoPlayerToHeal => "ai_work_12.refusal.no_player_to_heal",
            ServiceRefusal::PlayerOutOfRange => "ai_work_12.refusal.player_out_of_range",
            ServiceRefusal::NoRemotePatient => "ai_work_12.refusal.no_remote_patient",
            ServiceRefusal::HealedWithoutCure => "ai_work_12.refusal.healed_without_cure",
            ServiceRefusal::ShovelMissing => "ai_work_12.refusal.shovel_missing",
            ServiceRefusal::NoGraveToWorkOn => "ai_work_12.refusal.no_grave_to_work_on",
            ServiceRefusal::NotAGrave => "ai_work_12.refusal.not_a_grave",
            ServiceRefusal::NotAtGrave => "ai_work_12.refusal.not_at_grave",
            ServiceRefusal::EffortPending => "ai_work_12.refusal.effort_pending",
            ServiceRefusal::WorkerInventoryFull => "ai_work_12.refusal.worker_inventory_full",
            ServiceRefusal::ResurrectFailed => "ai_work_12.refusal.resurrect_failed",
            ServiceRefusal::NoBurialSpot => "ai_work_12.refusal.no_burial_spot",
            ServiceRefusal::NoDiggedGrave => "ai_work_12.refusal.no_digged_grave",
            ServiceRefusal::NotAtBurial => "ai_work_12.refusal.not_at_burial",
            ServiceRefusal::BurialFailed => "ai_work_12.refusal.burial_failed",
        }
    }

    /// Ce refus accompagne-t-il un travail **en cours**, c'est-à-dire un tick qui agit
    /// et attend, plutôt qu'un tick qui ne peut rien faire ?
    ///
    /// Java ne distingue pas les deux : `return getState()` sert aux deux. Un panneau
    /// qui affiche « en cours » au lieu de « bloqué » a besoin de la différence.
    pub fn in_progress(self) -> bool {
        matches!(
            self,
            ServiceRefusal::Draining
                | ServiceRefusal::Enchanting
                | ServiceRefusal::FreeCureRunning
                | ServiceRefusal::EffortPending
                | ServiceRefusal::DrainTooFar
                | ServiceRefusal::NotAtGrave
                | ServiceRefusal::NotAtBurial
                | ServiceRefusal::NotAtPatient
                | ServiceRefusal::NotAtHut
                | ServiceRefusal::NotAtHospital
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_work_12::ServiceTrade;

    #[test]
    fn the_table_names_eighteen_states_and_no_two_share_a_name() {
        assert_eq!(ServiceState::ALL.len(), 18);
        let mut names: Vec<&str> = ServiceState::ALL.iter().map(|state| state.java()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 18, "deux états partagent un nom Java");
        let trade = ServiceState::ALL
            .iter()
            .filter(|state| state.worker().is_none())
            .count();
        assert_eq!(trade, 11, "onze états de métier");
        assert_eq!(ServiceState::Enchant.java_declaration(), "AIWorkerState.java:604");
        assert_eq!(ServiceState::Wander.java(), "WANDER");
    }

    #[test]
    fn the_eat_flag_is_false_for_the_five_states_java_marks_false() {
        // `ENCHANT(false)` `:604`, et les quatre du croque-mort `:169`, `:174`, `:179`,
        // `:184`.
        assert!(!ServiceState::Enchant.okay_to_eat());
        assert!(!ServiceState::EmptyGrave.okay_to_eat());
        assert!(!ServiceState::DigGrave.okay_to_eat());
        assert!(!ServiceState::BuryCitizen.okay_to_eat());
        assert!(!ServiceState::TryResurrect.okay_to_eat());
        // `ENCHANTER_DRAIN(true)` `:599` et les six du guérisseur.
        assert!(ServiceState::EnchanterDrain.okay_to_eat());
        assert!(ServiceState::Cure.okay_to_eat());
        // Un état de base garde le drapeau que `crate::ai` lui donne (`:23`).
        assert!(!ServiceState::Base(WorkerState::InventoryFull).okay_to_eat());
        assert!(ServiceState::Base(WorkerState::StartWorking).okay_to_eat());
    }

    #[test]
    fn each_trade_owns_only_its_own_trade_states() {
        assert!(ServiceState::Enchant.belongs_to(ServiceTrade::Enchanter));
        assert!(!ServiceState::Enchant.belongs_to(ServiceTrade::Healer));
        assert!(ServiceState::CurePlayer.belongs_to(ServiceTrade::Healer));
        assert!(ServiceState::BuryCitizen.belongs_to(ServiceTrade::Undertaker));
        // `WANDER` est aux deux métiers à la fois (`:93`, `:92`) : aucun des deux ne le
        // revendique seul, donc il appartient à aucun.
        assert!(!ServiceState::Wander.belongs_to(ServiceTrade::Healer));
        assert!(!ServiceState::Wander.belongs_to(ServiceTrade::Undertaker));
        // Un état de base n'appartient à personne.
        assert!(!ServiceState::Base(WorkerState::Decide).belongs_to(ServiceTrade::Healer));
    }

    #[test]
    fn every_refusal_names_a_java_line_and_a_message_key() {
        use ServiceRefusal::*;
        let all = [
            NoFactsForTrade, NotAtHut, CraftingDue, DumpDue, NoDrainStation, NoBook,
            BookInHutOnly, NoGatherableBuilding, NoAncientTome, AncientTomeInHutOnly,
            NoDrainTarget, DrainBuildingGone, NoWorkerToDrain, DrainTargetAbsent,
            DrainTooFar, DrainWalkGaveUp, Draining, NoBookToSpend, NoEnchantingRecipe,
            Enchanting, NotAtHospital, NoPatient, PatientGone, PatientInventoryFull,
            CureMissing, CureInHutOnly, CureRequested, CureAlreadyRequested, NotAtPatient,
            FreeCureRunning, NoPlayerToHeal, PlayerOutOfRange, NoRemotePatient,
            HealedWithoutCure, ShovelMissing, NoGraveToWorkOn, NotAGrave, NotAtGrave,
            EffortPending, WorkerInventoryFull, ResurrectFailed, NoBurialSpot,
            NoDiggedGrave, NotAtBurial, BurialFailed,
        ];
        assert_eq!(all.len(), 45, "une variante par branche portée");
        for refusal in all {
            assert!(!refusal.java().is_empty(), "{refusal:?}");
            assert!(
                refusal.key().starts_with("ai_work_12.refusal."),
                "{refusal:?}"
            );
        }
        assert!(NoFactsForTrade.java().starts_with("port:"));
        // Les clés sont uniques : deux refus ne peuvent pas partager un message.
        let mut keys: Vec<&str> = all.iter().map(|refusal| refusal.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), all.len());
    }

    #[test]
    fn ten_refusals_mean_a_pass_is_running_and_the_rest_mean_it_stopped() {
        assert!(ServiceRefusal::Draining.in_progress());
        assert!(ServiceRefusal::EffortPending.in_progress());
        assert!(ServiceRefusal::FreeCureRunning.in_progress());
        assert!(ServiceRefusal::Enchanting.in_progress());
        // Un citoyen qui marche n'a pas fini son geste : c'est « en cours » aussi.
        assert!(ServiceRefusal::NotAtGrave.in_progress());
        assert!(ServiceRefusal::NotAtHut.in_progress());
        // Ceux-là disent qu'il n'y a rien à faire, pas que ça avance.
        assert!(!ServiceRefusal::NoBook.in_progress());
        assert!(!ServiceRefusal::NoGraveToWorkOn.in_progress());
        assert!(!ServiceRefusal::CureMissing.in_progress());
        assert!(!ServiceRefusal::NoPatient.in_progress());
    }
}
