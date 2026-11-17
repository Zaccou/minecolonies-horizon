//! `colony/quests/*.json`: the `triggerOrder` expression, checked before a colony
//! ever evaluates it.
//!
//! Java reference: `core/datalistener/QuestJsonListener.java:246-391` (the parse)
//! and `:399-453` (the evaluation), with the symbols in
//! `core/quests/QuestParsingConstants.java:26-31`. 74 quest files ship and the
//! `triggerOrder` field takes exactly four distinct values in them:
//! `"1 || 2"` ×32, `"1 && (2 || 3)"` ×2, `"(1 || 2) && 3"` ×2, and
//! `"6 && ((1 && 4) || (2 && 4 && 1) || (1 && 3 && 4)) && 5"` ×2.
//!
//! **An absent or empty order means "all of them", and it is not a special case
//! of the parser.** `order.isEmpty()` returns an `AND` over the triggers in list
//! order (`:249-267`), with no tokenising at all. That is the branch 32 of the 74
//! files take.
//!
//! **Three ways a non-empty order passes Java's only check and still fails.** The
//! check is one line: the number of **distinct indices** must equal the number of
//! triggers (`:296-300`). It compares counts, never ranges, and it is applied
//! before the tree is built.
//!
//! | Order | Triggers | `types.size() == triggers.size()` | What happens next |
//! | --- | --- | --- | --- |
//! | `1 \|\| 3` | 2 | 2 == 2, **passes** | `triggerMap.get("3")` is null → NPE at `:445` |
//! | `0 \|\| 1` | 2 | 2 == 2, **passes** | index 0 is never a trigger → NPE at `:445` |
//! | `1 \|\| 1` | 2 | 1 != 2, refused | `null` is stored as the quest's trigger function, so the quest never fires |
//!
//! And one operator is tokenised, built into the tree, and never evaluated: `!` is
//! a case of the *builder* (`:335-338`) and has no case in `evaluate` (`:401-452`),
//! so it falls to `default`, looks up a trigger named `"!"`, finds null and throws.
//! No shipped file uses it. [`Refusal::NotOperator`] is that hole, named.
//!
//! **The arity check is this port's own** and is labelled so ([`Refusal::Arity`]):
//! Java builds the tree by walking the tokens backwards with a 100-slot depth
//! array (`:316`), so a stray `(` drives `current` negative and a trailing `&&`
//! dereferences a null child. Requiring `binary operators + 1 == operands` refuses
//! both, and every one of the four shipped orders satisfies it.
//!
//! **What is not here**: the tree and `evaluate`, which is the quest manager's
//! (`quests::`), and `ITriggerReturnData`, which needs a `Colony`. This file answers
//! one question — is this order usable at all — so a bad data pack is refused at
//! load instead of throwing inside a colony tick.

use super::Refused;

/// `QuestParsingConstants.OR` (`:28`).
pub const OR: &str = "||";
/// `QuestParsingConstants.AND` (`:29`).
pub const AND: &str = "&&";
/// `QuestParsingConstants.NOT` (`:30`): tokenised at `:335-338`, never evaluated.
pub const NOT: &str = "!";

/// One token of a `triggerOrder`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: the five symbols of the grammar, and a number
pub enum Token {
    /// A 1-based trigger position, as `Integer.parseInt` reads it (`:284`).
    ///
    /// `u32` and not `u16`: a digit run that no `int` could hold must still be
    /// *refused as out of range*, not turned into a parse error.
    Index(u32),
    /// `&&`.
    And,
    /// `||`.
    Or,
    /// `!` — refused by [`validate`], tokenised here for the message.
    Not,
    /// `(`.
    Open,
    /// `)`.
    Close,
}

/// Why an order is refused, named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: this port's own refusal set, closed on purpose
pub enum Refusal {
    /// A character that is none of `0-9`, `&`, `|`, `!`, `(`, `)`, or a space.
    UnknownCharacter { at: usize, got: char },
    /// A lone `&` or `|`: the grammar has two-character operators only.
    LoneAmpersand { at: usize },
    /// A lone `|`.
    LonePipe { at: usize },
    /// Different numbers of `(` and `)` — Java's depth array goes negative
    /// (`:324-331`).
    UnbalancedBraces { open: usize, close: usize },
    /// **Port-only**: `binary operators + 1 != operands`.
    Arity { operands: usize, operators: usize },
    /// `!` is built but never evaluated (`:335-338` vs `:401-452`).
    NotOperator { at: usize },
    /// An index outside `1..=trigger_count`. Java's count check lets this through
    /// (`:296-300`) and throws at `:445`.
    IndexOutOfRange { index: u32, count: usize },
    /// The distinct indices do not number the triggers (`:296-300`). Java logs and
    /// stores `null` as the quest's trigger function.
    CountMismatch { found: usize, expected: usize },
}

impl Refused for Refusal {
    fn name(&self) -> String {
        match *self {
            Refusal::UnknownCharacter { at, got } => {
                format!("data_listeners.refusal.quest_order_unknown_character({at}.{got})")
            }
            Refusal::LoneAmpersand { at } => {
                format!("data_listeners.refusal.quest_order_lone_ampersand({at})")
            }
            Refusal::LonePipe { at } => {
                format!("data_listeners.refusal.quest_order_lone_pipe({at})")
            }
            Refusal::UnbalancedBraces { open, close } => {
                format!("data_listeners.refusal.quest_order_unbalanced_braces({open}.{close})")
            }
            Refusal::Arity { operands, operators } => {
                format!("data_listeners.refusal.quest_order_arity({operands}.{operators})")
            }
            Refusal::NotOperator { at } => {
                format!("data_listeners.refusal.quest_order_not_operator({at})")
            }
            Refusal::IndexOutOfRange { index, count } => {
                format!("data_listeners.refusal.quest_order_index_out_of_range({index}.{count})")
            }
            Refusal::CountMismatch { found, expected } => {
                format!("data_listeners.refusal.quest_order_count_mismatch({found}.{expected})")
            }
        }
    }
}

/// Splits an order into tokens. Whitespace is dropped first (`:276`).
pub fn tokenize(order: &str) -> Result<Vec<Token>, Refusal> {
    let chars: Vec<char> = order.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        match c {
            c if c.is_whitespace() => at += 1,
            '0'..='9' => {
                // `Integer.parseInt` throws on an overflow; an index that large is
                // out of range whatever the count is, so saturating is enough and
                // keeps the refusal about the range rather than about the parse.
                let mut index: u32 = 0;
                while at < chars.len() && chars[at].is_ascii_digit() {
                    let digit = chars[at] as u32 - '0' as u32;
                    index = index.saturating_mul(10).saturating_add(digit);
                    at += 1;
                }
                tokens.push(Token::Index(index));
            }
            '&' => {
                if chars.get(at + 1) == Some(&'&') {
                    tokens.push(Token::And);
                    at += 2;
                } else {
                    return Err(Refusal::LoneAmpersand { at });
                }
            }
            '|' => {
                if chars.get(at + 1) == Some(&'|') {
                    tokens.push(Token::Or);
                    at += 2;
                } else {
                    return Err(Refusal::LonePipe { at });
                }
            }
            '!' => {
                tokens.push(Token::Not);
                at += 1;
            }
            '(' => {
                tokens.push(Token::Open);
                at += 1;
            }
            ')' => {
                tokens.push(Token::Close);
                at += 1;
            }
            other => return Err(Refusal::UnknownCharacter { at, got: other }),
        }
    }
    Ok(tokens)
}

/// Reads an order against the number of triggers the quest declares, returning the
/// indices it uses in first-seen order.
///
/// An empty order is legal and means "every trigger" (`:249-267`), which is why it
/// returns `1..=trigger_count` rather than nothing.
pub fn validate(order: &str, trigger_count: usize) -> Result<Vec<u32>, Refusal> {
    if order.trim().is_empty() {
        return Ok((1..=trigger_count as u32).collect());
    }
    let tokens = tokenize(order)?;

    let mut open = 0;
    let mut close = 0;
    let mut operands = 0;
    let mut operators = 0;
    let mut indices: Vec<u32> = Vec::new();
    for (at, token) in tokens.iter().enumerate() {
        match *token {
            Token::Open => open += 1,
            Token::Close => close += 1,
            Token::And | Token::Or => operators += 1,
            Token::Not => return Err(Refusal::NotOperator { at }),
            Token::Index(index) => {
                operands += 1;
                if index < 1 || index as usize > trigger_count {
                    return Err(Refusal::IndexOutOfRange { index, count: trigger_count });
                }
                if !indices.contains(&index) {
                    indices.push(index);
                }
            }
        }
    }
    if open != close {
        return Err(Refusal::UnbalancedBraces { open, close });
    }
    // Port-only, and every shipped order satisfies it.
    if operators + 1 != operands {
        return Err(Refusal::Arity { operands, operators });
    }
    // `:296-300`, the only check Java makes.
    if indices.len() != trigger_count {
        return Err(Refusal::CountMismatch { found: indices.len(), expected: trigger_count });
    }
    Ok(indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four distinct values the 74 shipped quest files use.
    const SHIPPED: &[(&str, usize)] = &[
        ("1 || 2", 2),
        ("1 && (2 || 3)", 3),
        ("(1 || 2) && 3", 3),
        ("6 && ((1 && 4) || (2 && 4 && 1) || (1 && 3 && 4)) && 5", 6),
    ];

    #[test]
    fn the_four_shipped_orders_all_validate() {
        for (order, triggers) in SHIPPED {
            let indices = validate(order, *triggers).unwrap_or_else(|error| {
                panic!("{order} is shipped but refused: {}", error.name())
            });
            assert_eq!(indices.len(), *triggers, "{order}");
        }
    }

    #[test]
    fn whitespace_is_irrelevant_and_the_indices_come_out_in_first_seen_order() {
        assert_eq!(validate("  1||2  ", 2), Ok(vec![1, 2]));
        assert_eq!(validate("3 && (1 || 2)", 3), Ok(vec![3, 1, 2]));
        assert_eq!(validate("1 || 2", 2), validate("1||2", 2));
    }

    #[test]
    fn an_empty_order_means_every_trigger() {
        // `:249-267`: the AND branch, no tokenising.
        assert_eq!(validate("", 3), Ok(vec![1, 2, 3]));
        assert_eq!(validate("   ", 1), Ok(vec![1]));
        assert_eq!(validate("", 0), Ok(vec![]));
    }

    #[test]
    fn an_index_past_the_last_trigger_is_refused_where_java_passes_its_only_check() {
        // Two distinct indices, two triggers: Java's count check says yes and
        // `triggerMap.get("3")` returns null at `:445`.
        let refusal = validate("1 || 3", 2).expect_err("3 is not a trigger");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.quest_order_index_out_of_range(3.2)"
        );
        // And index 0, which `types.size()` also counts as fine.
        assert!(validate("0 || 1", 2).is_err());
    }

    #[test]
    fn the_not_operator_is_refused_because_no_evaluation_case_exists() {
        let refusal = validate("!1", 1).expect_err("`!` has no case in evaluate");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.quest_order_not_operator(0)"
        );
    }

    #[test]
    fn a_mismatch_between_indices_and_triggers_is_refused() {
        // `:296-300` logs and stores `null`, so the quest can never fire.
        let refusal = validate("1 || 1", 2).expect_err("one distinct index for two triggers");
        assert_eq!(
            refusal.name(),
            "data_listeners.refusal.quest_order_count_mismatch(1.2)"
        );
        // The same order against one trigger is fine: `1 && 1` is the same trigger
        // twice, which Java's tree evaluates twice.
        assert_eq!(validate("1 && 1", 1), Ok(vec![1]));
    }

    #[test]
    fn braces_must_balance_and_every_operator_needs_a_second_operand() {
        let unbalanced = validate("(1 || 2", 2).expect_err("one `(` and no `)`");
        assert_eq!(
            unbalanced.name(),
            "data_listeners.refusal.quest_order_unbalanced_braces(1.0)"
        );
        let arity = validate("1 &&", 1).expect_err("a trailing operator");
        assert_eq!(arity.name(), "data_listeners.refusal.quest_order_arity(1.1)");
        assert!(validate("1 || 2 ||", 2).is_err());
        assert!(validate("& 1", 1).is_err());
    }
}
