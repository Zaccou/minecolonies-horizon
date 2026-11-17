//! Which state moves exist, and which actor may make each one.
//!
//! Java reference: `api/colony/requestsystem/request/RequestState.java:18-78`
//! (the thirteen states, printed in their ordinal order),
//! `core/.../management/handlers/RequestHandler.java` (`assignRequestDefault:130`
//! sets `ASSIGNING`, `resolve:244` hands the request to a resolver and `:266-271`
//! sets `IN_PROGRESS`, `resolveRequest:541-544` demands `IN_PROGRESS`,
//! `onRequestCancelledDirectly:517` sets `CANCELLED`),
//! `core/.../management/manager/StandardRequestManager.java:294-332` (the switch
//! that reacts to each new state) and `:346-356` (a player overrule),
//! `core/colony/requestsystem/requests/AbstractRequest.java:137-154` (the setter,
//! which has **no** legality check) and `:355-376` (a child pushing its parent
//! forward), and `requests/StandardRequests.java` for the concrete requestables.
//!
//! **Java has a setter, not a state machine.** `setState` writes the field and
//! tells the parent (`AbstractRequest.java:137-154`); every rule about *when* a
//! move is legal lives in the callers, and the manager is trusted. The port is
//! the layer that has to be trusted, so the moves are a table here: a pair that
//! Java never makes is [`super::Refusal::IllegalTransition`], and a pair Java
//! makes but not for this actor is [`super::Refusal::NotAllowed`].
//!
//! **The thirteen states, collapsed to five, in one place.** Java runs
//! `CREATED → REPORTED → ASSIGNING → ASSIGNED → IN_PROGRESS → RESOLVED →
//! COMPLETED → RECEIVED`, with `OVERRULED`, `CANCELLED`, `FOLLOWUP_IN_PROGRESS`,
//! `FINALIZING` and `FAILED` beside it (`RequestState.java:18-78`). This port has
//! the five `requests::State` holds, and the table below records **which Java
//! transition each edge stands for**, so a reader can tell a collapse from a
//! missing rule:
//!
//! | port edge | Java states it covers |
//! | --- | --- |
//! | `Created → Assigned` | `CREATED`/`REPORTED`/`ASSIGNING` → `ASSIGNED` |
//! | `Assigned → InProgress` | `ASSIGNED` → `IN_PROGRESS` |
//! | `→ Delivered` | `IN_PROGRESS` → `RESOLVED` → `COMPLETED` → `RECEIVED` |
//! | `→ Cancelled` | `CANCELLED`, `FAILED`, `OVERRULED`-as-withdrawal, `FINALIZING` |
//!
//! **Expiry is a reason, not a sixth state.** A request that runs out of retries
//! lands in `Cancelled`, and [`End::Expired`] says why — the retry bound lives in
//! [`super::bounds`], and a sixth `State` variant would break
//! `requests/request.rs`'s saved names and every match on it.
//!
//! **Two collapses this table deliberately does not make.**
//!
//! * `Assigned → Delivered` by a resolver is allowed here because the port's
//!   `Request::deliver` can complete in one call, but a **partial** delivery from
//!   `Assigned` is *not* an edge the port allows: Java always reaches
//!   `IN_PROGRESS` first (`RequestHandler.java:266-273`), so a resolver that hands
//!   in half a count must call [`Move::Begin`] first. The table is stricter than
//!   the model underneath it, on purpose.
//! * `OVERRULED` (a player forcing the goods in) becomes [`Move::Deliver`] by
//!   [`Actor::Player`], because that is what it is; Java reaches it from any
//!   state at all (`StandardRequestManager.java:346-356`), while the port's model
//!   refuses it from `Created` (`requests/request.rs:274`). See the module note in
//!   `super`'s third seam.

use crate::inventory::Stack;
use crate::requests::{Request, State};

use super::assignment::Assignment;
use super::token::Token;
use super::Refusal;

/// Who is asking for the move. Java's manager is trusted and has no such notion
/// (`StandardRequestManager.java:294-332`); the actors come from the call sites.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a closed set of five call sites
pub enum Actor {
    /// The request system itself: it assigns (`RequestHandler.java:244`) and it
    /// cancels when a building goes away (`:584-593`). It is also the one that
    /// *learns* a retry budget is spent (`StandardRetryingRequestResolver.java:159`)
    /// — though the right to cancel is the pair's, not the reason's.
    Manager,
    /// The resolver the request is assigned to: it starts the work and hands the
    /// goods in (`AbstractWarehouseRequestResolver.java:289-297`).
    Resolver,
    /// The building or citizen that asked for it: it withdraws its own need
    /// (`RequestHandler.java:517`, reached by `AbstractBuilding`).
    Requester,
    /// A child request whose own progress drags its parent forward
    /// (`AbstractRequest.java:366-370`).
    Child,
    /// A player, who may force the goods in (`StandardRequestManager.java:346-356`).
    Player,
}

/// Why a request stopped. All three land in `State::Cancelled`; the reason is what
/// a log line and a test need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: a closed set of three ways to stop
pub enum End {
    /// The requester no longer wants it — Java's `CANCELLED`
    /// (`RequestHandler.java:517`).
    Withdrawn,
    /// The supplier could not do it — Java's `FAILED`
    /// (`JobDeliveryman.java:353`, `AbstractJobCrafter.java:260`).
    Failed,
    /// The retry budget is spent — the port's reason for the same outcome, from
    /// `StandardRetryingRequestResolver.java:31`.
    Expired,
}

impl End {
    /// The name a log line uses, namespaced like the refusals.
    pub fn name(self) -> &'static str {
        match self {
            End::Withdrawn => "requestsystem.end.withdrawn",
            End::Failed => "requestsystem.end.failed",
            End::Expired => "requestsystem.end.expired",
        }
    }
}

/// One move on a request.
#[derive(Clone, Debug, PartialEq, Eq)]
// exhaustive: a closed set of four moves
pub enum Move {
    /// Hand it to a resolver — Java's `ASSIGNING` then `ASSIGNED`
    /// (`RequestHandler.java:130`, `ResolverHandler.java:353`). The position is
    /// the resolver's, so a resolver without one cannot be assigned through this
    /// layer (`Refusal::NoPosition` is raised by the caller that knows it).
    Assign { at: [i32; 3] },
    /// Start the work — `RequestHandler.java:266-271`.
    Begin,
    /// Hand goods in; a partial count leaves the request in progress
    /// (`requests/request.rs:267-287`).
    Deliver { stack: Stack },
    /// Stop it, for one of the three reasons.
    End(End),
}

/// What a move actually did, so a caller does not have to re-read the request to
/// learn the outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: one variant per move
pub enum Step {
    /// Now assigned, at this position.
    Assigned { at: [i32; 3] },
    /// Now in progress.
    Began,
    /// Goods accepted, and whether that finished the request.
    Delivered { accepted: u32, done: bool },
    /// Now cancelled, for this reason.
    Ended { why: End },
}

/// How far along a request is, for the one rule that compares two states: a child
/// may only push its parent **forward** (`AbstractRequest.java:366-370`, Java
/// compares `RequestState` ordinals). A finished request has no rank — nothing
/// moves it.
pub fn progress_rank(state: State) -> Option<u8> {
    match state {
        State::Created => Some(0),
        State::Assigned => Some(1),
        State::InProgress => Some(2),
        State::Delivered | State::Cancelled => None,
    }
}

/// Which actors may make the move from `from` to `to`, or an empty slice when
/// Java makes no such move.
///
/// The empty answer is the interesting one: 25 pairs, eight of them real. Every
/// edge carries the Java call that makes it.
pub fn allowed(from: State, to: State) -> &'static [Actor] {
    use Actor::{Child, Manager, Player, Requester, Resolver};
    match (from, to) {
        // `assignRequestDefault`: ASSIGNING (:130), then `addRequestToResolver`
        // sets ASSIGNED (:353). Only the manager assigns — a requester that could
        // pick its own supplier would be a second assignment rule.
        (State::Created, State::Assigned) => &[Manager],
        // `resolve()`: IN_PROGRESS once a resolver has it (:266-271).
        (State::Assigned, State::InProgress) => &[Resolver, Child],
        // The whole count at once, or a player overrule
        // (StandardRequestManager.java:346-356).
        (State::Assigned, State::Delivered) => &[Resolver, Player],
        // A partial hand-in stays in progress (requests/request.rs:268-286).
        (State::InProgress, State::InProgress) => &[Resolver],
        (State::InProgress, State::Delivered) => &[Resolver, Player],
        // `processDirectCancellationOf` (:492-523) and `FAILED` from a supplier.
        (State::Created, State::Cancelled) => &[Requester, Manager],
        (State::Assigned, State::Cancelled) => &[Requester, Manager, Resolver],
        (State::InProgress, State::Cancelled) => &[Requester, Manager, Resolver],
        // Both terminal states, and the two pairs that would go backwards.
        _ => &[],
    }
}

/// Whether `actor` may make the move from `from` to `to`.
pub fn may(from: State, to: State, actor: Actor) -> bool {
    allowed(from, to).contains(&actor)
}

/// Make `mv` on `request`, on behalf of `by`.
///
/// Three checks, in this order, so the refusal names the first thing that is
/// wrong: the pair must be a move Java makes at all
/// ([`Refusal::IllegalTransition`]); the move must be one that can produce that
/// pair; and the actor must be allowed to make it ([`Refusal::NotAllowed`]).
/// The request model's own refusal, if any, comes back unchanged inside
/// [`Refusal::Model`].
pub fn advance(request: &mut Request, mv: Move, by: Actor) -> Result<Step, Refusal> {
    let from = request.state();
    let to = destination(request, &mv);

    if allowed(from, to).is_empty() {
        return Err(Refusal::IllegalTransition { from, to });
    }
    if !produces(&mv, from, to) {
        return Err(Refusal::IllegalTransition { from, to });
    }
    if !may(from, to, by) {
        return Err(Refusal::NotAllowed { to, by });
    }

    match mv {
        Move::Assign { at } => {
            request.assign(at)?;
            Ok(Step::Assigned { at })
        }
        Move::Begin => {
            request.begin()?;
            Ok(Step::Began)
        }
        Move::Deliver { stack } => {
            let accepted = request.deliver(&stack)?;
            Ok(Step::Delivered { accepted, done: request.state() == State::Delivered })
        }
        Move::End(why) => {
            request.cancel()?;
            Ok(Step::Ended { why })
        }
    }
}

/// The state `mv` leads to, computed before the move so the table can be
/// consulted without touching the request. For [`Move::Deliver`] it is arithmetic
/// on the counts, exactly what `Request::deliver` will accept
/// (`requests/request.rs:268`).
fn destination(request: &Request, mv: &Move) -> State {
    match mv {
        Move::Assign { .. } => State::Assigned,
        Move::Begin => State::InProgress,
        Move::Deliver { stack } => {
            let accepted = stack.count().min(request.outstanding());
            if request.delivered() + accepted >= request.count() {
                State::Delivered
            } else {
                State::InProgress
            }
        }
        Move::End(_) => State::Cancelled,
    }
}

/// Whether `mv` is a move that can end in `to` from `from`. One line per move,
/// each citing where Java does it — this is what keeps a partial delivery from
/// `Assigned` out, and what keeps [`Move::Begin`] from pretending to deliver.
fn produces(mv: &Move, from: State, to: State) -> bool {
    match mv {
        // RequestHandler.java:130, :244 — always into ASSIGNED.
        Move::Assign { .. } => matches!((from, to), (State::Created, State::Assigned)),
        // RequestHandler.java:266-271.
        Move::Begin => matches!((from, to), (State::Assigned, State::InProgress)),
        // requests/request.rs:274 accepts a delivery from ASSIGNED too, but Java
        // always reaches IN_PROGRESS first (`RequestHandler.java:266-273`), so a
        // partial count from ASSIGNED is not an edge this table allows: the
        // resolver starts the work first. A whole count at once stays legal
        // because that is what "resolved immediately" means.
        Move::Deliver { .. } => match (from, to) {
            (State::InProgress, State::InProgress | State::Delivered) => true,
            (State::Assigned, State::Delivered) => true,
            _ => false,
        },
        // Handler.java:517 and everything that funnels through it.
        Move::End(_) => to == State::Cancelled,
    }
}

/// The state a child's progress pushes its parent into, or `None` when the parent
/// is already there — `AbstractRequest.childStateUpdated`
/// (`AbstractRequest.java:366-370`): a child starts, and a parent that has not
/// started becomes `IN_PROGRESS`. A parent further along is never pulled back.
pub fn child_started(parent: State) -> Option<State> {
    match progress_rank(parent) {
        Some(rank) if rank < 2 => Some(State::InProgress),
        _ => None,
    }
}

/// Whether a parent whose last child just arrived may be resolved — the
/// `parentRequest.getState() == IN_PROGRESS` branch of
/// `RequestHandler.onRequestCompleted` (`:382-388`). A parent still holding
/// children waits; a parent that is not in progress is not this branch.
pub fn parent_can_resolve(parent: State, remaining_children: usize) -> bool {
    remaining_children == 0 && parent == State::InProgress
}

/// Check that `holder` is the one that may move `id` — the guard Java does not
/// have, built on [`Assignment::holder_of`].
///
/// [`Refusal::NotAssigned`] is Java's own case: `getResolverForRequest` throws
/// when nothing holds the request (`ResolverHandler.java:401-404`).
pub fn holder_may(book: &Assignment, id: i32, holder: Token) -> Result<(), Refusal> {
    match book.holder_of(id) {
        None => Err(Refusal::NotAssigned { id }),
        Some(current) if current == holder => Ok(()),
        Some(_) => Err(Refusal::NotTheHolder { holder }),
    }
}

/// Hand `request` to `holder`: the whole of `assignRequestDefault`'s outcome, in
/// one call — the state move ([`Move::Assign`], `RequestHandler.java:244`) and the
/// book entry (`ResolverHandler.java:351`).
///
/// **A holder with no position is refused** ([`Refusal::NoPosition`]) and nothing
/// is written: not the state, not the book. Java would set `ASSIGNED` with no
/// position at all (`ResolverHandler.java:353`); `requests::Request` stores its
/// assignee as a `[i32; 3]` and repairs "assigned with nobody named" back to
/// `Created` on read (`requests/request.rs:356`), so a half-written assignment
/// would not survive a save. The two holders this affects are the player resolver
/// and the retrying one, both already out of scope for the port
/// (`requests/mod.rs:22-27`).
pub fn assign_to(
    request: &mut Request,
    book: &mut Assignment,
    holder: super::chain::Resolver,
) -> Result<Step, Refusal> {
    let Some(at) = holder.at() else {
        return Err(Refusal::NoPosition { holder: holder.token() });
    };
    let step = advance(request, Move::Assign { at }, Actor::Manager)?;
    book.bind(request.id(), holder.token())?;
    Ok(step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemName;
    use crate::requests::{Requester, Source};
    use crate::colony_requests::chain::Resolver;

    const HUT: [i32; 3] = [10, 64, 10];
    const OTHER: [i32; 3] = [20, 64, 10];

    fn ladder() -> ItemName {
        ItemName::new("minecraft:ladder").expect("a valid name")
    }

    fn asking() -> Request {
        Request::new(1, ladder(), 4, 4, Requester::new("minecolonies:blockhutbuilder", HUT))
            .expect("a nonzero request")
    }

    fn stack(count: u32) -> Stack {
        Stack::new(ladder(), count).expect("a nonzero stack")
    }

    fn assigned() -> Request {
        let mut request = asking();
        assert_eq!(advance(&mut request, Move::Assign { at: OTHER }, Actor::Manager), Ok(Step::Assigned { at: OTHER }));
        request
    }

    fn started() -> Request {
        let mut request = assigned();
        assert_eq!(advance(&mut request, Move::Begin, Actor::Resolver), Ok(Step::Began));
        request
    }

    #[test]
    fn the_table_has_exactly_the_eight_edges_java_makes() {
        let mut real = 0;
        for from in State::ALL {
            for to in State::ALL {
                if !allowed(from, to).is_empty() {
                    real += 1;
                }
            }
        }
        assert_eq!(real, 8, "five forward edges plus three ways into Cancelled");
        assert_eq!(allowed(State::Created, State::Assigned), &[Actor::Manager][..]);
        assert_eq!(
            allowed(State::Assigned, State::InProgress),
            &[Actor::Resolver, Actor::Child][..]
        );
        assert!(allowed(State::Delivered, State::Cancelled).is_empty());
        assert!(allowed(State::Cancelled, State::Created).is_empty());
        assert!(allowed(State::Created, State::InProgress).is_empty());
        assert!(allowed(State::Delivered, State::Delivered).is_empty());
    }

    #[test]
    fn no_edge_goes_backwards_and_every_actor_has_at_least_one() {
        for from in State::ALL {
            for to in State::ALL {
                if allowed(from, to).is_empty() {
                    continue;
                }
                match (progress_rank(from), progress_rank(to)) {
                    // Two live states: the move may not go backwards
                    // (`AbstractRequest.java:366-370` is the one rule that compares
                    // two states, and it only ever pushes forward).
                    (Some(before), Some(after)) => {
                        assert!(after >= before, "{from:?} -> {to:?} goes backwards");
                    }
                    // `Delivered` and `Cancelled` have no rank: they are terminal,
                    // and an edge into one of them is the end of the line.
                    (Some(_), None) => {}
                    (None, _) => panic!("{from:?} is terminal but has an edge to {to:?}"),
                }
            }
        }
        for actor in
            [Actor::Manager, Actor::Resolver, Actor::Requester, Actor::Child, Actor::Player]
        {
            assert!(
                State::ALL
                    .iter()
                    .any(|&from| State::ALL.iter().any(|&to| may(from, to, actor))),
                "{actor:?} may make no move at all"
            );
        }
    }

    #[test]
    fn only_the_manager_assigns_and_it_assigns_where_it_says() {
        let mut request = asking();
        assert_eq!(
            advance(&mut request, Move::Assign { at: OTHER }, Actor::Child),
            Err(Refusal::NotAllowed { to: State::Assigned, by: Actor::Child })
        );
        assert_eq!(request.state(), State::Created, "a refused move changes nothing");
        assert_eq!(
            advance(&mut request, Move::Assign { at: OTHER }, Actor::Manager),
            Ok(Step::Assigned { at: OTHER })
        );
        assert_eq!(request.state(), State::Assigned);
        assert_eq!(request.assigned_to(), Some(OTHER));
        assert_eq!(
            advance(&mut request, Move::Assign { at: OTHER }, Actor::Manager),
            Err(Refusal::IllegalTransition { from: State::Assigned, to: State::Assigned })
        );
    }

    #[test]
    fn work_starts_only_from_assigned_and_never_by_the_requester() {
        let mut request = assigned();
        assert_eq!(
            advance(&mut request, Move::Begin, Actor::Requester),
            Err(Refusal::NotAllowed { to: State::InProgress, by: Actor::Requester })
        );
        assert_eq!(advance(&mut request, Move::Begin, Actor::Child), Ok(Step::Began));
        let mut fresh = asking();
        assert_eq!(
            advance(&mut fresh, Move::Begin, Actor::Resolver),
            Err(Refusal::IllegalTransition { from: State::Created, to: State::InProgress })
        );
    }

    #[test]
    fn a_partial_hand_in_stays_open_and_a_full_one_finishes_it() {
        let mut request = started();
        assert_eq!(
            advance(&mut request, Move::Deliver { stack: stack(2) }, Actor::Resolver),
            Ok(Step::Delivered { accepted: 2, done: false })
        );
        assert_eq!(request.state(), State::InProgress);
        assert_eq!(
            advance(&mut request, Move::Deliver { stack: stack(9) }, Actor::Resolver),
            Ok(Step::Delivered { accepted: 2, done: true }),
            "never more than what was missing"
        );
        assert_eq!(request.state(), State::Delivered);
    }

    #[test]
    fn a_child_may_push_the_state_but_may_not_hand_goods_in() {
        let mut request = assigned();
        assert_eq!(
            advance(&mut request, Move::Deliver { stack: stack(4) }, Actor::Child),
            Err(Refusal::NotAllowed { to: State::Delivered, by: Actor::Child }),
            "the pair exists (Begin), the move does not"
        );
        assert_eq!(request.state(), State::Assigned);
    }

    #[test]
    fn a_player_may_force_the_goods_in_but_not_before_someone_holds_it() {
        let mut request = assigned();
        assert_eq!(
            advance(&mut request, Move::Deliver { stack: stack(4) }, Actor::Player),
            Ok(Step::Delivered { accepted: 4, done: true })
        );
        let mut fresh = asking();
        assert_eq!(
            advance(&mut fresh, Move::Deliver { stack: stack(4) }, Actor::Player),
            Err(Refusal::IllegalTransition { from: State::Created, to: State::Delivered }),
            "Java overrules from any state (:346-356); the port's model does not"
        );
    }

    #[test]
    fn a_partial_delivery_from_assigned_must_start_the_work_first() {
        let mut request = assigned();
        assert_eq!(
            advance(&mut request, Move::Deliver { stack: stack(1) }, Actor::Resolver),
            Err(Refusal::IllegalTransition { from: State::Assigned, to: State::InProgress }),
            "Java always reaches IN_PROGRESS first (RequestHandler.java:266-273)"
        );
        assert_eq!(request.state(), State::Assigned);
    }

    #[test]
    fn the_wrong_item_comes_back_as_the_models_own_reason() {
        let mut request = started();
        let planks = ItemName::new("minecraft:oak_planks").expect("a valid name");
        let wrong = Stack::new(planks, 4).expect("a nonzero stack");
        assert_eq!(
            advance(&mut request, Move::Deliver { stack: wrong }, Actor::Resolver),
            Err(Refusal::Model(crate::requests::Refusal::WrongItem))
        );
        assert_eq!(request.delivered(), 0);
    }

    #[test]
    fn ending_is_three_reasons_into_one_state() {
        for why in [End::Withdrawn, End::Failed, End::Expired] {
            let mut request = started();
            assert_eq!(
                advance(&mut request, Move::End(why), Actor::Resolver),
                Ok(Step::Ended { why })
            );
            assert_eq!(request.state(), State::Cancelled);
            assert_eq!(
                advance(&mut request, Move::End(why), Actor::Resolver),
                Err(Refusal::IllegalTransition { from: State::Cancelled, to: State::Cancelled })
            );
            assert!(why.name().starts_with("requestsystem.end."));
        }
        assert_ne!(End::Withdrawn.name(), End::Expired.name());
    }

    #[test]
    fn a_request_stops_for_three_reasons_and_the_reason_is_only_a_label() {
        // Java's `updateRequestState` takes a state and nothing else
        // (`StandardRequestManager.java:294-332`); the callers decide which state,
        // and no (reason, actor) pair is ever checked. `End` is the port's label
        // on the same move, so it cannot grant or withhold a right.
        for why in [End::Withdrawn, End::Failed, End::Expired] {
            for actor in [Actor::Requester, Actor::Manager, Actor::Resolver] {
                let mut request = started();
                assert_eq!(
                    advance(&mut request, Move::End(why), actor),
                    Ok(Step::Ended { why }),
                    "{actor:?} may cancel (:492-523), whatever it calls it"
                );
                assert_eq!(request.state(), State::Cancelled);
            }
        }
        // An actor the pair does not allow is still refused, reason or not.
        let mut request = started();
        assert_eq!(
            advance(&mut request, Move::End(End::Expired), Actor::Child),
            Err(Refusal::NotAllowed { to: State::Cancelled, by: Actor::Child })
        );
        assert_eq!(
            advance(&mut request, Move::End(End::Withdrawn), Actor::Requester),
            Ok(Step::Ended { why: End::Withdrawn }),
            "a building withdrawing its own need is not a failure"
        );
    }

    #[test]
    fn a_finished_request_takes_no_more_moves() {
        for terminal in [State::Delivered, State::Cancelled] {
            for mv in [
                Move::Assign { at: OTHER },
                Move::Begin,
                Move::Deliver { stack: stack(1) },
                Move::End(End::Withdrawn),
            ] {
                let mut request = if terminal == State::Delivered {
                    let mut request = started();
                    advance(&mut request, Move::Deliver { stack: stack(4) }, Actor::Resolver)
                        .expect("finish it");
                    request
                } else {
                    let mut request = started();
                    advance(&mut request, Move::End(End::Failed), Actor::Resolver)
                        .expect("end it");
                    request
                };
                assert!(matches!(
                    advance(&mut request, mv, Actor::Manager),
                    Err(Refusal::IllegalTransition { .. })
                ));
                assert_eq!(request.state(), terminal);
            }
        }
    }

    #[test]
    fn a_child_pushes_a_fresh_parent_forward_and_never_pulls_one_back() {
        assert_eq!(child_started(State::Created), Some(State::InProgress));
        assert_eq!(child_started(State::Assigned), Some(State::InProgress));
        assert_eq!(child_started(State::InProgress), None, "already there");
        assert_eq!(child_started(State::Delivered), None);
        assert_eq!(child_started(State::Cancelled), None, "nothing is dragged out of a grave");
    }

    #[test]
    fn a_parent_resolves_only_when_the_last_child_is_in_and_it_is_mid_work() {
        assert!(parent_can_resolve(State::InProgress, 0));
        assert!(!parent_can_resolve(State::InProgress, 1), "a child still holds it");
        assert!(!parent_can_resolve(State::Assigned, 0), "not started");
        assert!(!parent_can_resolve(State::Delivered, 0));
        assert!(!parent_can_resolve(State::Cancelled, 0));
    }

    #[test]
    fn only_the_holder_may_act_and_nobody_at_all_is_its_own_refusal() {
        let mut book = Assignment::new();
        assert_eq!(
            holder_may(&book, 1, Token::new(9)),
            Err(Refusal::NotAssigned { id: 1 })
        );
        book.bind(1, Token::new(9)).expect("bind");
        assert_eq!(holder_may(&book, 1, Token::new(9)), Ok(()));
        assert_eq!(
            holder_may(&book, 1, Token::new(8)),
            Err(Refusal::NotTheHolder { holder: Token::new(8) })
        );
    }

    #[test]
    fn assigning_writes_the_state_and_the_book_together() {
        let mut request = asking();
        let mut book = Assignment::new();
        let holder = Resolver::building(Token::new(5), Source::Warehouse, OTHER);
        assert_eq!(
            assign_to(&mut request, &mut book, holder),
            Ok(Step::Assigned { at: OTHER })
        );
        assert_eq!(request.assigned_to(), Some(OTHER));
        assert_eq!(book.holder_of(request.id()), Some(Token::new(5)));
        assert_eq!(book.requests_of(Token::new(5)), &[request.id()][..]);
    }

    #[test]
    fn a_holder_with_no_position_is_refused_and_writes_nothing() {
        let mut request = asking();
        let mut book = Assignment::new();
        for holder in [Resolver::player(Token::new(6)), Resolver::retrying(2)] {
            assert_eq!(
                assign_to(&mut request, &mut book, holder),
                Err(Refusal::NoPosition { holder: holder.token() }),
                "Java sets ASSIGNED with no position (ResolverHandler.java:353)"
            );
        }
        assert_eq!(request.state(), State::Created, "nothing half-written");
        assert_eq!(request.assigned_to(), None);
        assert!(book.is_empty(), "and no phantom holder");
    }

    #[test]
    fn a_second_assignment_changes_neither_the_state_nor_the_book() {
        let mut request = asking();
        let mut book = Assignment::new();
        let first = Resolver::building(Token::new(5), Source::Warehouse, OTHER);
        assign_to(&mut request, &mut book, first).expect("the first assignment");
        let second = Resolver::building(Token::new(6), Source::Warehouse, HUT);
        assert_eq!(
            assign_to(&mut request, &mut book, second),
            Err(Refusal::IllegalTransition { from: State::Assigned, to: State::Assigned })
        );
        assert_eq!(request.assigned_to(), Some(OTHER));
        assert_eq!(book.holder_of(request.id()), Some(Token::new(5)));
        assert_eq!(book.len(), 1, "one request, one holder");
    }

    #[test]
    fn every_ported_state_of_the_model_is_reachable_from_created() {
        // A small proof rather than a claim: each of the four destinations of
        // `Created` is reachable by at least one move the table allows.
        for to in State::ALL {
            if to == State::Created {
                continue;
            }
            let reachable = !allowed(State::Created, to).is_empty()
                || !allowed(State::Assigned, to).is_empty()
                || !allowed(State::InProgress, to).is_empty();
            assert!(reachable, "{to:?} cannot be reached at all");
        }
    }
}
