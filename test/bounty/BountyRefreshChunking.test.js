const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("BountyRefresh chunked processing", function () {
  async function setup(n) {
    const [owner, other] = await ethers.getSigners();
    const F = await ethers.getContractFactory("BountyRefresh");
    const c = await F.deploy();
    const addrs = Array.from({ length: n }, () => ethers.Wallet.createRandom().address);
    await c.createBatch(addrs, addrs.map((_, i) => i + 1));
    return { c, owner, other };
  }

  it("processes first chunk, then resumes with cursor", async function () {
    const { c } = await setup(25);
    await c.processBatchParallel(0);
    expect((await c.getBatch(0)).nextIndex).to.equal(10);
    await expect(c.finalizeBatch(0)).to.be.revertedWithCustomError(c, "BatchNotFullyProcessed");
    await expect(c.processBatchChunk(0, 0, 10)).to.be.revertedWithCustomError(c, "InvalidChunk");
    await c.processBatchChunk(0, 10, 10);
    await c.processBatchChunk(0, 20, 10);
    await c.finalizeBatch(0);
    expect((await c.getBatch(0)).successCount).to.equal(25);
  });

  it("keeps a 100-item batch under a safe per-tx gas ceiling", async function () {
    const { c } = await setup(100);
    let tx = await c.processBatchParallel(0);
    expect((await tx.wait()).gasUsed).to.be.lt(5_000_000n);
    for (let i = 10; i < 100; i += 10) {
      tx = await c.processBatchChunk(0, i, 10);
      expect((await tx.wait()).gasUsed).to.be.lt(5_000_000n);
    }
  });
});
