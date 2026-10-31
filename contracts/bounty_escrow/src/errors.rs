use soroban_sdk::contracterror;

/// Contract errors. Numbering is stable: never renumber or reuse a value after deploy.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    Paused = 4,
    TokenNotAllowed = 5,
    AmountTooSmall = 6,
    AmountTooLarge = 7,
    InvalidDeadline = 8,
    IssueAlreadyFunded = 9,
    BountyNotFound = 10,
    InvalidStatus = 11,
    DeadlineNotReached = 12,
    SelfPayout = 13,
    InvalidSplit = 14,
    InvalidFee = 15,
    Overflow = 16,
    RelayerNotAllowed = 17,
}
