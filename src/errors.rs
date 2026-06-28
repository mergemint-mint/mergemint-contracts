// SPDX-License-Identifier: MIT

/// All contract error conditions in one place.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    BountyNotFound,
    BountyAlreadyAssigned,
    BountyNotOpen,
    BountyNotInProgress,
    BountyHasNoAssignee,
    RewardMustBePositive,
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
    OnlyCreatorOrAssigneeCanDispute,
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
        ContractError::BountyNotOpen => "bounty not open",
        ContractError::BountyNotInProgress => "bounty not in progress",
        ContractError::BountyHasNoAssignee => "bounty has no assignee",
        ContractError::RewardMustBePositive => "reward must be positive",
        ContractError::NotBountyCreator => "not bounty creator",
        ContractError::VerifierCannotBeAssignee => "verifier cannot be assignee",
        ContractError::CreatorCannotClaim => "creator cannot claim",
        ContractError::ContributorHasActiveClaim => "contributor already has an active claim",
        ContractError::BountyIsDisputed => "bounty is disputed",
        ContractError::BountyDeadlinePassed => "bounty deadline passed",
        ContractError::BountyNoDeadline => "bounty has no deadline",
        ContractError::DeadlineNotPassed => "deadline has not passed",
        ContractError::ReputationTooLow => "contributor reputation is too low",
        ContractError::TooManyTags => "too many tags",
        ContractError::OnlyCreatorOrAssigneeCanDispute => "only creator or assignee can raise dispute",
    }
}

// Legacy string constants kept for backward compat references in storage/events
pub const BOUNTY_NOT_FOUND: &str = "bounty not found";
pub const BOUNTY_ALREADY_ASSIGNED: &str = "bounty already assigned";
pub const BOUNTY_NOT_OPEN: &str = "bounty not open";
pub const BOUNTY_NOT_IN_PROGRESS: &str = "bounty not in progress";
pub const BOUNTY_HAS_NO_ASSIGNEE: &str = "bounty has no assignee";
pub const REWARD_MUST_BE_POSITIVE: &str = "reward must be positive";
pub const NOT_BOUNTY_CREATOR: &str = "not bounty creator";
pub const VERIFIER_CANNOT_BE_ASSIGNEE: &str = "verifier cannot be assignee";
pub const CREATOR_CANNOT_CLAIM: &str = "creator cannot claim";
pub const CONTRIBUTOR_HAS_ACTIVE_CLAIM: &str = "contributor already has an active claim";
pub const BOUNTY_DEADLINE_PASSED: &str = "bounty deadline passed";
pub const BOUNTY_NO_DEADLINE: &str = "bounty has no deadline";
pub const DEADLINE_NOT_PASSED: &str = "deadline has not passed";
