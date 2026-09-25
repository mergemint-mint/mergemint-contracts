// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "../../../contracts/bounty/BountyRefresh.sol";

/// @dev Test helper: fails the refresh for a bounty until the task has been
/// attempted `failUntilAttempt[bountyId]` times, so tests can exercise the
/// retry path in BountyRefresh.
contract FlakyBountyRefresh is BountyRefresh {
    mapping(uint256 => uint256) public failUntilAttempt;

    function setFailUntilAttempt(uint256 bountyId, uint256 attempts) external {
        failUntilAttempt[bountyId] = attempts;
    }

    function _executeRefresh(address contributor, uint256 bountyId) external view override onlySelf {
        if (refreshTasks[taskCounter - 1].attempts <= failUntilAttempt[bountyId]) {
            revert("Transient refresh failure");
        }
        require(contributor != address(0), "Invalid contributor");
    }
}
