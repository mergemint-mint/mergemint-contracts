use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    BountyNotFound = 4,
    BountyNotOpen = 5,
    BountyNotInProgress = 6,
    BountyNotCompleted = 7,
    BountyAlreadyCompleted = 8,
    InvalidDeadline = 9,
    InvalidApprovalThreshold = 10,
    ApprovalThresholdExceedsVerifiers = 11,
    VerifierCannotBeAssignee = 12,
    VerifierAlreadyExists = 13,
    VerifierNotFound = 14,
    AssigneeNotFound = 15,
    SubmissionNotFound = 16,
    AlreadyApproved = 17,
    NotAVerifier = 18,
    InsufficientApprovals = 19,
    InvalidAmount = 20,
    InsufficientFunds = 21,
    TransferFailed = 22,
    InvalidFeeRecipient = 23,
    FeeExceedsMaximum = 24,
}
