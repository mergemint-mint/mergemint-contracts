// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/**
 * @title IBountyManager
 * @notice Interface exposed by the bounty manager contract that consumers
 *         such as BountyRefresh rely on to read and update contributor
 *         bounty data.
 */
interface IBountyManager {
    /**
     * @notice Updates the on-chain metrics for a contributor's participation
     *         in a specific bounty.
     * @dev Intended to be called after an off-chain or on-chain event that
     *      changes a contributor's standing on the given bounty (e.g. a
     *      refresh cycle). Implementations should handle repeated calls for
     *      the same contributor/bounty pair idempotently.
     * @param contributor The contributor address whose metrics are updated.
     * @param bountyId The ID of the bounty the metrics update relates to.
     */
    function updateContributorMetrics(address contributor, uint256 bountyId) external;

    /**
     * @notice Returns every contributor address associated with a bounty.
     * @param bountyId The ID of the bounty to query.
     * @return Array of contributor addresses linked to the bounty.
     */
    function getBountyContributors(uint256 bountyId) external view returns (address[] memory);

    /**
     * @notice Returns the number of contributors associated with a bounty.
     * @param bountyId The ID of the bounty to query.
     * @return Number of contributors linked to the bounty.
     */
    function getContributorCount(uint256 bountyId) external view returns (uint256);
}
