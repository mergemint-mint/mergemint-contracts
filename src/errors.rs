// SPDX-License-Identifier: MIT

//! Canonical contract error conditions.
//!
//! Errors are surfaced as a typed [`ContractError`] with stable numeric
//! discriminants via Soroban's `#[contracterror]` convention. Entrypoints
//! return `Result<_, ContractError>` so the SDK can decode a deterministic
//! `u32` code instead of parsing an opaque panic string. This keeps the
//! release WASM free of the ~60 static error messages and lets the backend
//! match on numeric codes.
//!
//! Discriminants are part of the public ABI: never renumber an existing
//! variant. Append new variants with the next unused value.
use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    BountyNotFound = 1,
    BountyAlreadyAssigned = 2,
    AlreadyClaimed = 3,
    BountyNotOpen = 4,
    BountyNotInProgress = 5,
    BountyHasNoAssignee = 6,
    RewardMustBePositive = 7,
    RewardBelowMinimum = 8,
    NotBountyCreator = 9,
    VerifierCannotBeAssignee = 10,
    CreatorCannotClaim = 11,
    ContributorHasActiveClaim = 12,
    BountyIsDisputed = 13,
    BountyDeadlinePassed = 14,
    BountyNoDeadline = 15,
    DeadlineNotPassed = 16,
    ReputationTooLow = 17,
    TooManyTags = 18,
    MaxAssigneesMustBePositive = 19,
    OnlyCreatorOrAssigneeCanDispute = 20,
    VerifierNotAuthorized = 21,
    AlreadyApproved = 22,
    BountyNotDisputed = 23,
    NotArbitrator = 24,
    ApprovalThresholdExceedsVerifiers = 25,
    InvalidRewardToken = 26,
    InvalidStatus = 27,
    InvalidTag = 28,
    InvalidResolution = 29,
    MilestoneAlreadyCompleted = 30,
    NotAllMilestonesCompleted = 31,
    InvalidMilestoneIndex = 32,
    MilestoneRewardsMismatch = 33,
    MilestoneRewardMustBePositive = 34,
    TooManyMilestones = 35,
    RewardAmountOverflow = 36,
    MetadataEmpty = 37,
    NotAssignee = 38,
    AmountMustBePositive = 39,
    SubmissionRequired = 40,
    DuplicateTag = 41,
    CreatorLimitReached = 42,
    NotAdmin = 43,
    ClaimCooldownActive = 44,
    OnlyPartyCanAddEvidence = 45,
    EvidenceLimitReached = 46,
    DisputeAlreadyResolved = 47,
    NoPendingResolution = 48,
    AppealWindowClosed = 49,
    AlreadyAppealed = 50,
    FinalizeTooEarly = 51,
    ClaimWindowNotElapsed = 52,
    NoActiveClaim = 53,
    MilestoneDeadlineInvalid = 54,
    MilestoneDeadlinePassed = 55,
    AdminAlreadySet = 56,
    RewardTokenNotAllowed = 57,
    /// Internal storage invariant violated (e.g. a status index page that
    /// should contain the bounty being removed turned out to be empty).
    /// Surfacing this as a typed error rather than panicking on an unwrap
    /// keeps the failure mode a catchable contract error instead of an
    /// opaque host trap.
    CorruptedIndex = 58,
    NotPendingAdmin = 59,
    NotPendingOwner = 60,
    OwnerCannotBeAssignee = 61,
    /// A state-mutating entrypoint was called while the admin has the
    /// contract paused.
    ContractPaused = 62,
    /// The designated arbitrator is the bounty creator or one of its
    /// assignees, i.e. an interested party in the dispute (#1195).
    ArbitratorConflict = 63,
}

/// Convert a `ContractError` to its canonical panic message and panic.
// The one sanctioned panic: every typed contract error funnels through here.
#[allow(clippy::panic)]
#[inline(never)]
pub fn fail(e: ContractError) -> ! {
    panic!("{}", message(e))
}

/// Convert a `ContractError` to its canonical human-readable message.
///
/// Retained for tests and documentation only; contract logic must return the
/// typed error rather than trapping with this string.
pub const fn message(e: ContractError) -> &'static str {
    match e {
        ContractError::BountyNotFound => "bounty not found",
        ContractError::BountyAlreadyAssigned => "bounty already assigned",
        ContractError::AlreadyClaimed => "bounty already claimed by contributor",
        ContractError::BountyNotOpen => "bounty not open",
        ContractError::BountyNotInProgress => "bounty is not in progress",
        ContractError::BountyHasNoAssignee => "bounty has no assignee",
        ContractError::RewardMustBePositive => "reward_amount must be positive",
        ContractError::RewardBelowMinimum => "reward_amount is below the minimum allowed",
        ContractError::NotBountyCreator => "not bounty creator",
        ContractError::VerifierCannotBeAssignee => "verifier cannot be the assignee",
        ContractError::CreatorCannotClaim => "creator cannot claim",
        ContractError::ContributorHasActiveClaim => "contributor already has an active claim",
        ContractError::BountyIsDisputed => "bounty is disputed",
        ContractError::BountyDeadlinePassed => "bounty deadline passed",
        ContractError::BountyNoDeadline => "bounty has no deadline",
        ContractError::DeadlineNotPassed => "deadline has not passed",
        ContractError::ReputationTooLow => "contributor reputation is too low",
        ContractError::TooManyTags => "too many tags",
        ContractError::MaxAssigneesMustBePositive => "max_assignees must be at least 1",
        ContractError::OnlyCreatorOrAssigneeCanDispute => {
            "only creator or assignee can raise dispute"
        }
        ContractError::VerifierNotAuthorized => "verifier is not in the required verifiers list",
        ContractError::AlreadyApproved => "verifier has already approved this bounty",
        ContractError::BountyNotDisputed => "bounty is not in disputed status",
        ContractError::NotArbitrator => "caller is not authorized to resolve this dispute",
        ContractError::ApprovalThresholdExceedsVerifiers => {
            "approval_threshold cannot exceed the number of required_verifiers"
        }
        ContractError::InvalidRewardToken => "invalid reward_token address",
        ContractError::InvalidStatus => "invalid bounty status",
        ContractError::InvalidTag => "invalid bounty tag",
        ContractError::InvalidResolution => "resolution must be 'complete' or 'cancel'",
        ContractError::MilestoneAlreadyCompleted => "milestone is already completed",
        ContractError::NotAllMilestonesCompleted => "not all milestones are completed",
        ContractError::InvalidMilestoneIndex => "invalid milestone index",
        ContractError::MilestoneRewardsMismatch => "milestone rewards do not sum to reward_amount",
        ContractError::MilestoneRewardMustBePositive => "milestone reward must be positive",
        ContractError::TooManyMilestones => "too many milestones",
        ContractError::RewardAmountOverflow => "reward amount arithmetic overflow",
        ContractError::MetadataEmpty => "metadata must not be empty",
        ContractError::NotAssignee => "caller is not an assignee of this bounty",
        ContractError::AmountMustBePositive => "amount must be positive",
        ContractError::SubmissionRequired => "a work submission is required before approval",
        ContractError::DuplicateTag => "duplicate tag",
        ContractError::CreatorLimitReached => "creator has reached the open bounty limit",
        ContractError::NotAdmin => "caller is not the contract admin",
        ContractError::ClaimCooldownActive => "claim cooldown is still active for this contributor",
        ContractError::OnlyPartyCanAddEvidence => {
            "only the creator or assignee may add dispute evidence"
        }
        ContractError::EvidenceLimitReached => "dispute evidence limit reached",
        ContractError::DisputeAlreadyResolved => "dispute has already been resolved",
        ContractError::NoPendingResolution => "no pending dispute decision for this bounty",
        ContractError::AppealWindowClosed => "the appeal window has closed",
        ContractError::AlreadyAppealed => "this dispute has already been appealed",
        ContractError::FinalizeTooEarly => "the appeal window has not yet passed",
        ContractError::ClaimWindowNotElapsed => "claim window has not elapsed yet",
        ContractError::NoActiveClaim => "bounty has no active claim to release",
        ContractError::MilestoneDeadlineInvalid => {
            "milestone deadlines must be non-decreasing and not exceed the bounty deadline"
        }
        ContractError::MilestoneDeadlinePassed => "milestone deadline has passed",
        ContractError::MilestoneDeadlineExpired => {
            "an uncompleted milestone deadline has already expired"
        }
        ContractError::TooManySponsors => "maximum number of sponsors per bounty reached",
        ContractError::SponsorAmountTooLow => "sponsor amount is below the minimum allowed",
        ContractError::AdminAlreadySet => "admin has already been initialized",
        ContractError::RewardTokenNotAllowed => "reward_token is not on the admin allowlist",
        ContractError::CorruptedIndex => "internal storage index invariant violated",
        ContractError::NotPendingAdmin => "caller is not the pending admin",
        ContractError::NotPendingOwner => "caller is not the pending bounty owner",
        ContractError::OwnerCannotBeAssignee => "new owner cannot be an assignee of the bounty",
        ContractError::ContractPaused => "contract is paused",
        ContractError::ArbitratorConflict => {
            "arbitrator cannot be the bounty creator or an assignee"
        }
    }
}
