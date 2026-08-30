// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract MockBountyManager {
    enum BountyStatus {
        Active,
        Paused,
        Closed
    }

    mapping(address => uint256) public refreshCount;
    mapping(address => BountyStatus) public bountyStatus;
    bool public shouldFail = false;

    modifier onlyActiveBounty(address contributor) {
        require(bountyStatus[contributor] == BountyStatus.Active, "Mock bounty status is not Active");
        _;
    }

    function refreshContributor(address contributor) external onlyActiveBounty(contributor) {
        if (shouldFail) {
            revert("Mock refresh failed");
        }
        refreshCount[contributor]++;
    }

    function batchRefreshContributors(address[] calldata contributors) external {
        if (shouldFail) {
            revert("Mock batch refresh failed");
        }
        for (uint256 i = 0; i < contributors.length; i++) {
            require(
                bountyStatus[contributors[i]] == BountyStatus.Active,
                "Mock bounty status is not Active"
            );
            refreshCount[contributors[i]]++;
        }
    }

    function getContributorBounty(address contributor)
        external
        view
        onlyActiveBounty(contributor)
        returns (
            uint256 totalBounty,
            uint256 claimedBounty,
            uint256 pendingBounty
        )
    {
        return (1000, 500, 500);
    }

    function setShouldFail(bool _shouldFail) external {
        shouldFail = _shouldFail;
    }

    /// @dev Test helper: puts a contributor's bounty into a given status so
    /// tests can exercise the invalid-status revert paths above.
    function setBountyStatus(address contributor, BountyStatus status) external {
        bountyStatus[contributor] = status;
    }

    function getRefreshCount(address contributor) external view returns (uint256) {
        return refreshCount[contributor];
    }
}
