const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("BountyRefresh view bounds and pagination", function () {
  const sig = "getContributorTasks(address,uint256,uint256)";
  let c, user;

  beforeEach(async function () {
    const F = await ethers.getContractFactory("BountyRefresh");
    c = await F.deploy();
    user = ethers.Wallet.createRandom().address;
    await c.createBatch([user, user, user], [1, 2, 3]);
    await c.processBatchParallel(0);
  });

  it("reverts getTask for unknown id", async function () {
    await expect(c.getTask(3)).to.be.revertedWithCustomError(c, "TaskDoesNotExist");
    expect((await c.getTask(2)).id).to.equal(2);
  });

  it("paginates: partial, full and out-of-range slices", async function () {
    expect(await c.getContributorTaskCount(user)).to.equal(3);
    expect((await c[sig](user, 1, 1)).length).to.equal(1);
    expect((await c[sig](user, 0, 100)).length).to.equal(3);
    expect((await c[sig](user, 3, 10)).length).to.equal(0);
    expect((await c[sig](ethers.Wallet.createRandom().address, 0, 10)).length).to.equal(0);
  });
});
