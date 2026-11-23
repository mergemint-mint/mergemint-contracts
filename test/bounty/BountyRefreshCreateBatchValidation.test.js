const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("BountyRefresh.createBatch upfront validation", function () {
    let bountyRefresh, addr1, addr2;

    beforeEach(async function () {
        [, addr1, addr2] = await ethers.getSigners();
        const BountyRefresh = await ethers.getContractFactory("BountyRefresh");
        bountyRefresh = await BountyRefresh.deploy();
        await bountyRefresh.deployed();
    });

    it("reverts with InvalidContributor for a zero address", async function () {
        await expect(
            bountyRefresh.createBatch([addr1.address, ethers.constants.AddressZero], [1, 2])
        ).to.be.revertedWithCustomError(bountyRefresh, "InvalidContributor");
    });

    it("reverts with InvalidBountyId for a zero bounty id", async function () {
        await expect(
            bountyRefresh.createBatch([addr1.address, addr2.address], [1, 0])
        ).to.be.revertedWithCustomError(bountyRefresh, "InvalidBountyId");
    });

    it("reverts with DuplicateRefreshPair for a repeated pair", async function () {
        await expect(
            bountyRefresh.createBatch([addr1.address, addr1.address], [7, 7])
        ).to.be.revertedWithCustomError(bountyRefresh, "DuplicateRefreshPair");
    });

    it("does not persist a batch when validation fails", async function () {
        await expect(bountyRefresh.createBatch([ethers.constants.AddressZero], [1])).to.be.reverted;
        expect(await bountyRefresh.batchCounter()).to.equal(0);
    });

    it("accepts the same contributor with different bounty ids", async function () {
        await bountyRefresh.createBatch([addr1.address, addr1.address], [1, 2]);
        expect(await bountyRefresh.batchCounter()).to.equal(1);
    });
});
