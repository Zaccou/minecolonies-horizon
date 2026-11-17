//! What names a request and what names a resolver.
//!
//! Java reference: `api/colony/requestsystem/token/IToken.java` (one identifier),
//! `token/StandardToken.java:29-62` (a `UUID` and nothing else),
//! `core/colony/requestsystem/management/handlers/TokenHandler.java:33-37` (one
//! fresh token per request), `resolvers/StandardRetryingRequestResolver.java:30`,
//! `:45` (the one resolver whose identity is not fresh), and
//! `api/colony/requestsystem/token/AbstractTokenFactory.java:18-19`, `:42-43`
//! (how a token is saved: two longs).
//!
//! **A token is an identity, not a right.** In Java the token of a request is
//! only how the manager's maps find it; the *right* to move that request is the
//! assignment recorded in `IRequestResolverRequestAssignmentDataStore`, which
//! [`super::Assignment`] owns. That split is why this file has no method called
//! `may`.
//!
//! **128 bits, two halves.** Java's token *is* a `UUID` (`StandardToken.java:33`)
//! and it is saved as `Id_MSB`/`Id_LSB` (`AbstractTokenFactory.java:42-43`), so
//! the port's [`Token`] is one `u128` with both halves addressable — a save
//! written by Java reads back as the same token, which a hash of the debug form
//! would not.
//!
//! **Fresh tokens are a counter here, not a random draw.** `TokenHandler` asks
//! for `UUID.randomUUID()` (`:36`). The port has no randomness in the guest and
//! does not want one: a counter makes a save and the test that reads it
//! reproducible, and nothing in Java's rules depends on the token being
//! unpredictable — only on it being unique (`RequestHandler.java:64-68` refuses a
//! duplicate either way). A restored [`Tokens`] resumes where the save stopped.

/// The identity of a request or of a resolver — Java's `IToken` around a `UUID`
/// (`IToken.java:10-19`, `StandardToken.java:29-35`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token(u128);

impl Token {
    /// A token from its 128 bits.
    pub const fn new(value: u128) -> Token {
        Token(value)
    }

    /// A token from the two halves a save holds: `Id_MSB`, then `Id_LSB`
    /// (`AbstractTokenFactory.java:18-19`, `:42-43`). Most significant half
    /// first, as `UUID` orders them.
    pub const fn from_halves(msb: i64, lsb: i64) -> Token {
        Token(((msb as u128) << 64) | (lsb as u64 as u128))
    }

    /// The value the retrying resolver asks for as its identity:
    /// `colony.getID() * CONST_RETRYING_ID_SCALE`
    /// (`StandardRetryingRequestResolver.java:30`, `:45`), kept with Java's
    /// `int` arithmetic, which **wraps silently** past colony `107_374`. The
    /// port keeps the wrap rather than refusing: a colony that big is a corrupt
    /// save, and a different refusal here would only move the surprise.
    ///
    /// What is *not* ported is the factory step that turns that `int` into a
    /// `UUID` (`StandardTokenFactory.java:30-34` takes a `UUID`, not an `int`);
    /// the number is the part the retry logic compares, so the number is what
    /// this keeps.
    pub const fn retrying(colony: i32) -> Token {
        Token(colony.wrapping_mul(-20_000) as i64 as u128)
    }

    /// The 128 bits.
    pub const fn as_u128(self) -> u128 {
        self.0
    }

    /// The most significant half, what `Id_MSB` holds.
    pub const fn msb(self) -> i64 {
        (self.0 >> 64) as i64
    }

    /// The least significant half, what `Id_LSB` holds.
    pub const fn lsb(self) -> i64 {
        self.0 as u64 as i64
    }
}

impl std::fmt::Display for Token {
    /// Thirty-two hex digits, MSB first: one line in a log, and the same digits
    /// a `UUID` prints once the dashes are out.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

/// The source of fresh tokens — `TokenHandler.generateNewToken`
/// (`TokenHandler.java:33-37`). One per colony, handed out in order.
///
/// Java refuses a duplicate identity at registration
/// (`RequestHandler.java:64-68`); a counter cannot produce one, and
/// [`Tokens::with_next`] is how a reloaded system resumes *after* its highest
/// token instead of starting again at the bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tokens {
    next: u128,
}

impl Tokens {
    /// A source whose first token is `1`. Zero is left out so that "never
    /// handed out" and "the first token" cannot be the same value.
    pub fn new() -> Tokens {
        Tokens { next: 1 }
    }

    /// A source that resumes at `next` — what a loaded save does, with
    /// `highest + 1`.
    pub fn with_next(next: u128) -> Tokens {
        Tokens { next: next.max(1) }
    }

    /// The next identity, and the one after it next time.
    pub fn take(&mut self) -> Token {
        let token = Token(self.next);
        self.next = self.next.saturating_add(1);
        token
    }

    /// What the next [`Self::take`] would answer, without taking it.
    pub fn peek(&self) -> Token {
        Token(self.next)
    }
}

impl Default for Tokens {
    fn default() -> Tokens {
        Tokens::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_its_bits_and_two_tokens_differ_by_them() {
        assert_eq!(Token::new(7), Token::new(7));
        assert_ne!(Token::new(7), Token::new(8));
        assert_eq!(Token::new(7).as_u128(), 7);
    }

    #[test]
    fn the_two_halves_of_a_save_read_back_as_the_same_token() {
        let token = Token::new(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210);
        assert_eq!(token.msb(), 0x0123_4567_89ab_cdef_u64 as i64);
        assert_eq!(token.lsb(), 0xfedc_ba98_7654_3210_u64 as i64);
        assert_eq!(Token::from_halves(token.msb(), token.lsb()), token);
        // A negative half is a half, not a broken token: `Id_MSB` is a long.
        let negative = Token::new(u128::MAX);
        assert_eq!(Token::from_halves(negative.msb(), negative.lsb()), negative);
    }

    #[test]
    fn thirty_two_hex_digits_name_a_token() {
        assert_eq!(Token::new(0).to_string(), "0".repeat(32));
        assert_eq!(Token::new(0xab).to_string(), format!("{}{:02x}", "0".repeat(30), 0xab));
        assert_eq!(Token::new(1).to_string().len(), 32);
    }

    #[test]
    fn the_retrying_resolver_identity_is_the_colony_times_its_scale() {
        // StandardRetryingRequestResolver.java:30, :45 — CONST_RETRYING_ID_SCALE.
        assert_eq!(Token::retrying(0).as_u128(), 0);
        assert_eq!(Token::retrying(1).as_u128(), (-20_000i64) as u128);
        assert_eq!(Token::retrying(3), Token::retrying(3), "derived, so repeatable");
        assert_ne!(Token::retrying(3), Token::retrying(4));
        assert_ne!(Token::retrying(1).as_u128(), 20_000u128, "the sign is part of it");
    }

    #[test]
    fn the_retrying_identity_keeps_javas_silent_overflow() {
        // 107_374 * 20_000 still fits in an int; one more does not.
        assert_eq!(Token::retrying(107_374).as_u128(), (-2_147_480_000i64) as u128);
        let wrapped = 107_375i32.wrapping_mul(-20_000);
        assert!(wrapped > 0, "the wrapped value changed sign, as Java's int does");
        assert_eq!(Token::retrying(107_375).as_u128(), wrapped as i64 as u128);
    }

    #[test]
    fn a_fresh_token_never_repeats_and_a_resumed_source_does_not_start_over() {
        let mut tokens = Tokens::new();
        let first = tokens.take();
        let second = tokens.take();
        assert_ne!(first, second);
        assert_eq!(first.as_u128(), 1, "zero is left out on purpose");
        assert_eq!(tokens.peek(), Token::new(3));
        let mut resumed = Tokens::with_next(3);
        assert_eq!(resumed.take(), Token::new(3), "picks up where the save stopped");
        let mut restored = Tokens::with_next(0);
        assert_eq!(restored.take(), Token::new(1), "and never goes back to zero");
        assert_eq!(Tokens::default(), Tokens::new());
    }
}
