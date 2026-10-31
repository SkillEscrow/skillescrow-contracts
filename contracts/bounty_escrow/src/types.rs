use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Open,
    Released,
    Refunded,
    Disputed,
    Resolved,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bounty {
    pub id: u64,
    pub maintainer: Address,
    /// GitHub numeric repository id.
    pub repo_id: u64,
    /// GitHub issue number.
    pub issue: u64,
    pub token: Address,
    /// Gross amount escrowed, in the token's smallest unit.
    pub amount: i128,
    /// Snapshot of `Config::fee_bps` taken at funding time.
    pub fee_bps: u32,
    /// Ledger timestamp in seconds.
    pub deadline: u64,
    /// The maintainer opted in to relayer release on merge.
    pub relayer_allowed: bool,
    pub status: Status,
    pub winner: Option<Address>,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenLimits {
    pub min_amount: i128,
    pub max_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub relayer: Address,
    pub arbiter: Address,
    pub treasury: Address,
    /// Maximum 1_000 (10%).
    pub fee_bps: u32,
    /// Refund is allowed only at `deadline + grace_secs`.
    pub grace_secs: u64,
    pub min_duration_secs: u64,
    pub max_duration_secs: u64,
    pub paused: bool,
}

/// Per-token accounting. After every state change
/// `funded == released_net + fees + refunded + held`.
#[contracttype]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Totals {
    pub funded: i128,
    pub released_net: i128,
    pub fees: i128,
    pub refunded: i128,
    pub held: i128,
}

#[contracttype]
pub enum DataKey {
    Config,
    NextId,
    Bounty(u64),
    /// `(repo_id, issue)` to bounty id while Open or Disputed.
    ActiveByIssue(u64, u64),
    /// Presence means the token is allowlisted.
    Token(Address),
    Totals(Address),
}
