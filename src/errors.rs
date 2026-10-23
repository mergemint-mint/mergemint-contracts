// SPDX-License-Identifier: MIT

//! Canonical contract error conditions.
//!
//! Each variant must be referenced from contract logic (`fail(ContractError::…)`)
//! and have a unique panic message. `InvalidRewardToken` is enforced during
//! `create_bounty` via a non-trapping token `balance` probe.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    BountyNotFound,
    BountyAlreadyAssigned,
    AlreadyClaimed,
    BountyNotOpen,
    BountyNotInProgress,
    BountyHasNoAssignee,
    RewardMustBePositive,
    RewardBelowMinimum,
    NotBountyCreator,
    VerifierCannotBeAssignee,
    CreatorCannotClaim,
    ContributorHasActiveClaim,
    BountyIsDisputed,
    BountyDeadlinePassed,
    BountyNoDeadline,
    DeadlineNotPassed,
    ReputationTooLow,
    TooManyTags,
    MaxAssigneesMustBePositive,
    OnlyCreatorOrAssigneeCanDispute,
    VerifierNotAuthorized,
    AlreadyApproved,
    BountyNotDisputed,
    NotArbitrator,
    ApprovalThresholdExceedsVerifiers,
    InvalidRewardToken,
    InvalidStatus,
    InvalidTag,
    InvalidResolution,
    MilestoneAlreadyCompleted,
    NotAllMilestonesCompleted,
    InvalidMilestoneIndex,
    MilestoneRewardsMismatch,
    RewardAmountOverflow,
    MetadataEmpty,
    NotAssignee,
    AmountMustBePositive,
    SubmissionRequired,
    DuplicateTag,
    CreatorLimitReached,
    NotAdmin,
    ClaimCooldownActive,
    OnlyPartyCanAddEvidence,
    EvidenceLimitReached,
    DisputeAlreadyResolved,
    NoPendingResolution,
    AppealWindowClosed,
    AlreadyAppealed,
    FinalizeTooEarly,
    ClaimWindowNotElapsed,
    NoActiveClaim,
    MilestoneDeadlineInvalid,
    MilestoneDeadlinePassed,
    AdminAlreadySet,
    RewardTokenNotAllowed,
    /// Internal storage invariant violated (e.g. a status index page that
    /// should contain the bounty being removed turned out to be empty).
    /// Surfacing this as a typed error rather than panicking on an unwrap
    /// keeps the failure mode a catchable contract error instead of an
    /// opaque host trap.
    CorruptedIndex,
    NotPendingAdmin,
    NotPendingOwner,
    OwnerCannotBeAssignee,
    /// A state-mutating entrypoint was called while the admin has the
    /// contract paused.
    ContractPaused,
}

/// Convert a `ContractError` to its canonical panic message and panic.
// The one sanctioned panic: every typed contract error funnels through here.
#[allow(clippy::panic)]
#[inline(never)]
pub fn fail(e: ContractError) -> ! {
    panic!("{}", message(e))
}

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
        ContractError::AdminAlreadySet => "admin has already been initialized",
        ContractError::RewardTokenNotAllowed => "reward_token is not on the admin allowlist",
        ContractError::CorruptedIndex => "internal storage index invariant violated",
        ContractError::NotPendingAdmin => "caller is not the pending admin",
        ContractError::NotPendingOwner => "caller is not the pending bounty owner",
        ContractError::OwnerCannotBeAssignee => "new owner cannot be an assignee of the bounty",
        ContractError::ContractPaused => "contract is paused",
    }
}
