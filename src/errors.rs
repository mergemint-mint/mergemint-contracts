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
    MilestoneAlreadyCompleted,
    NotAllMilestonesCompleted,
    InvalidMilestoneIndex,
    MilestoneRewardsMismatch,
    RewardAmountOverflow,
    MetadataEmpty,
}

/// Convert a `ContractError` to its canonical panic message and panic.
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
        ContractError::MilestoneAlreadyCompleted => "milestone is already completed",
        ContractError::NotAllMilestonesCompleted => "not all milestones are completed",
        ContractError::InvalidMilestoneIndex => "invalid milestone index",
        ContractError::MilestoneRewardsMismatch => "milestone rewards do not sum to reward_amount",
        ContractError::RewardAmountOverflow => "reward amount arithmetic overflow",
        ContractError::MetadataEmpty => "metadata must not be empty",
    }
}
