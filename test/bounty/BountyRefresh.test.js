const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("BountyRefresh", function () {
    let bountyRefresh;
    let mockBountyManager;
    let owner;
    let addr1, addr2, addr3, addr4, addr5;
    const MAX_BATCH_SIZE = 100;

    beforeEach(async function () {
        [owner, addr1, addr2, addr3, addr4, addr5] = await ethers.getSigners();

        // Deploy mock bounty manager
        const MockBountyManager = await ethers.getContractFactory("MockBountyManager");
        mockBountyManager = await MockBountyManager.deploy();
        await mockBountyManager.deployed();

        // Deploy BountyRefresh
        const BountyRefresh = await ethers.getContractFactory("BountyRefresh");
        bountyRefresh = await BountyRefresh.deploy(mockBountyManager.address);
        await bountyRefresh.deployed();
    });

    describe("Deployment", function () {
        it("Should set the correct bounty manager", async function () {
            expect(await bountyRefresh.bountyManager()).to.equal(mockBountyManager.address);
        });

        it("Should revert with invalid bounty manager", async function () {
            const BountyRefresh = await ethers.getContractFactory("BountyRefresh");
            await expect(BountyRefresh.deploy(ethers.constants.AddressZero)).to.be.revertedWithCustomError(
                bountyRefresh,
                "InvalidBountyManager"
            );
        });
    });

    describe("refreshBounty", function () {
        it("Should refresh single contributor", async function () {
            const contributors = [addr1.address];
            await expect(bountyRefresh.refreshBounty(contributors))
                .to.emit(bountyRefresh, "BatchRefreshStarted")
                .to.emit(bountyRefresh, "BatchRefreshCompleted");
        });

        it("Should refresh multiple contributors", async function () {
            const contributors = [addr1.address, addr2.address, addr3.address];
            await expect(bountyRefresh.refreshBounty(contributors))
                .to.emit(bountyRefresh, "BatchRefreshStarted")
                .to.emit(bountyRefresh, "BatchRefreshCompleted");
        });

        it("Should revert with empty contributors array", async function () {
            await expect(bountyRefresh.refreshBounty([])).to.be.revertedWithCustomError(
                bountyRefresh,
                "NoContributorsToRefresh"
            );
        });

        it("Should revert with batch size exceeded", async function () {
            const contributors = new Array(MAX_BATCH_SIZE + 1).fill(addr1.address);
            contributors[MAX_BATCH_SIZE] = addr2.address;
            await expect(bountyRefresh.refreshBounty(contributors)).to.be.revertedWithCustomError(
                bountyRefresh,
                "BatchSizeExceeded"
            );
        });

        it("Should revert with duplicate contributors", async function () {
            const contributors = [addr1.address, addr1.address];
            await expect(bountyRefresh.refreshBounty(contributors)).to.be.revertedWithCustomError(
                bountyRefresh,
                "InvalidContributorList"
            );
        });

        it("Should revert with zero address", async function () {
            const contributors = [ethers.constants.AddressZero];
            await expect(bountyRefresh.refreshBounty(contributors)).to.be.revertedWithCustomError(
                bountyRefresh,
                "InvalidContributorList"
            );
        });

        it("Should update last refresh time", async function () {
            const contributors = [addr1.address];
            const blockBefore = await ethers.provider.getBlock("latest");
            await bountyRefresh.refreshBounty(contributors);
            const blockAfter = await ethers.provider.getBlock("latest");

            const lastRefresh = await bountyRefresh.lastRefreshTime(addr1.address);
            expect(lastRefresh).to.be.gte(blockBefore.timestamp);
            expect(lastRefresh).to.be.lte(blockAfter.timestamp);
        });

        it("Should only allow owner", async function () {
            const contributors = [addr1.address];
            await expect(
                bountyRefresh.connect(addr1).refreshBounty(contributors)
            ).to.be.revertedWith("Ownable: caller is not the owner");
        });
    });

    describe("refreshBountyParallel", function () {
        it("Should parallelize batch refresh", async function () {
            const contributors = [addr1.address, addr2.address, addr3.address, addr4.address, addr5.address];
            await expect(bountyRefresh.refreshBountyParallel(contributors, 2))
                .to.emit(bountyRefresh, "BatchRefreshStarted")
                .to.emit(bountyRefresh, "BatchRefreshCompleted");
        });

        it("Should handle single batch", async function () {
            const contributors = [addr1.address, addr2.address];
            await expect(bountyRefresh.refreshBountyParallel(contributors, 10))
                .to.emit(bountyRefresh, "BatchRefreshStarted")
                .to.emit(bountyRefresh, "BatchRefreshCompleted");
        });

        it("Should revert with invalid batch size", async function () {
            const contributors = [addr1.address];
            await expect(bountyRefresh.refreshBountyParallel(contributors, 0)).to.be.revertedWithCustomError(
                bountyRefresh,
                "BatchSizeExceeded"
            );
        });

        it("Should revert with batch size exceeding max", async function () {
            const contributors = [addr1.address];
            await expect(
                bountyRefresh.refreshBountyParallel(contributors, MAX_BATCH_SIZE + 1)
            ).to.be.revertedWithCustomError(bountyRefresh, "BatchSizeExceeded");
        });
    });

    describe("Queue and Process", function () {
        it("Should queue contributors for refresh", async function () {
            const contributors = [addr1.address, addr2.address, addr3.address];
            await bountyRefresh.queueContributorsForRefresh(contributors);
            const count = await bountyRefresh.getPendingContributorsCount();
            expect(count).to.equal(3);
        });

        it("Should process pending batch", async function () {
            const contributors = [addr1.address, addr2.address, addr3.address];
            await bountyRefresh.queueContributorsForRefresh(contributors);
            await expect(bountyRefresh.processPendingBatch(2))
                .to.emit(bountyRefresh, "BatchRefreshStarted")
                .to.emit(bountyRefresh, "BatchRefreshCompleted");
            const count = await bountyRefresh.getPendingContributorsCount();
            expect(count).to.equal(1);
        });

        it("Should get pending contributors with pagination", async function () {
            const contributors = [addr1.address, addr2.address, addr3.address, addr4.address, addr5.address];
            await bountyRefresh.queueContributorsForRefresh(contributors);
            const page1 = await bountyRefresh.getPendingContributors(0, 2);
            expect(page1.length).to.equal(2);
            const page2 = await bountyRefresh.getPendingContributors(2, 2);
            expect(page2.length).to.equal(2);
        });

        it("Should revert processing with no pending contributors", async function () {
            await expect(bountyRefresh.processPendingBatch(10)).to.be.revertedWithCustomError(
                bountyRefresh,
                "NoContributorsToRefresh"
            );
        });
    });

    describe("setBountyManager", function () {
        it("Should update bounty manager", async function () {
            const newManager = addr1.address;
            await expect(bountyRefresh.setBountyManager(newManager))
                .to.emit(bountyRefresh, "BountyManagerUpdated")
                .withArgs(newManager);
            expect(await bountyRefresh.bountyManager()).to.equal(newManager);
        });

        it("Should revert with zero address", async function () {
            await expect(
                bountyRefresh.setBountyManager(ethers.constants.AddressZero)
            ).to.be.revertedWithCustomError(bountyRefresh, "InvalidBountyManager");
        });

        it("Should only allow owner", async function () {
            await expect(
                bountyRefresh.connect(addr1).setBountyManager(addr2.address)
            ).to.be.revertedWith("Ownable: caller is not the owner");
        });
    });

    describe("Access Control Matrix", function () {
        // Table-driven check: every state-changing function restricted to
        // the owner must reject every non-privileged caller role with the
        // same Ownable revert reason. This guards against a future change
        // accidentally loosening (or forgetting) the onlyOwner modifier on
        // any one of these entry points.
        const RESTRICTED_FUNCTIONS = [
            {
                name: "refreshBounty",
                invoke: (contract, caller) => contract.connect(caller).refreshBounty([addr1.address]),
            },
            {
                name: "refreshBountyParallel",
                invoke: (contract, caller) =>
                    contract.connect(caller).refreshBountyParallel([addr1.address, addr2.address], 2),
            },
            {
                name: "queueContributorsForRefresh",
                invoke: (contract, caller) =>
                    contract.connect(caller).queueContributorsForRefresh([addr1.address]),
            },
            {
                name: "processPendingBatch",
                invoke: (contract, caller) => contract.connect(caller).processPendingBatch(1),
            },
            {
                name: "setBountyManager",
                invoke: (contract, caller) => contract.connect(caller).setBountyManager(addr1.address),
            },
        ];

        // Non-privileged caller roles the matrix exercises against each
        // restricted function above.
        const NON_PRIVILEGED_ROLES = [
            { role: "random contributor #1", getSigner: () => addr1 },
            { role: "random contributor #2", getSigner: () => addr2 },
            { role: "random contributor #3", getSigner: () => addr3 },
            { role: "random contributor #4", getSigner: () => addr4 },
            { role: "random contributor #5", getSigner: () => addr5 },
        ];

        RESTRICTED_FUNCTIONS.forEach(({ name, invoke }) => {
            describe(name, function () {
                NON_PRIVILEGED_ROLES.forEach(({ role, getSigner }) => {
                    it(`Should reject call from ${role}`, async function () {
                        await expect(invoke(bountyRefresh, getSigner())).to.be.revertedWith(
                            "Ownable: caller is not the owner"
                        );
                    });
                });
            });
        });

        it("Should allow the owner to call every restricted function", async function () {
            for (const { name, invoke } of RESTRICTED_FUNCTIONS) {
                await expect(invoke(bountyRefresh, owner)).to.not.be.revertedWith(
                    "Ownable: caller is not the owner"
                );
            }
        });
    });

    describe("Reentrancy Protection", function () {
        it("Should prevent reentrancy in refreshBounty", async function () {
            const ReentrancyAttacker = await ethers.getContractFactory("ReentrancyAttacker");
            const attacker = await ReentrancyAttacker.deploy(bountyRefresh.address);
            await attacker.deployed();

            await expect(attacker.attack([addr1.address])).to.be.revertedWith(
                "ReentrancyGuard: reentrant call"
            );
        });
    });

    describe("Gas Usage Regression", function () {
        // Baselines captured against the current implementation of each
        // hot-path function. GAS_TOLERANCE_PCT absorbs small, legitimate
        // fluctuations (compiler/optimizer version bumps, minor refactors)
        // while still catching a regression that meaningfully inflates the
        // gas cost of a frequently-called function (e.g. an accidental
        // storage read/write added to a loop body).
        const GAS_TOLERANCE_PCT = 20;

        const GAS_BASELINES = {
            refreshBountySingle: 90000,
            refreshBountyBatchOfThree: 220000,
            refreshBountyParallel: 260000,
            queueContributorsForRefresh: 150000,
            processPendingBatch: 150000,
        };

        function maxAllowedGas(baseline) {
            return baseline + Math.ceil((baseline * GAS_TOLERANCE_PCT) / 100);
        }

        function expectGasWithinBaseline(gasUsed, baseline, label) {
            const upperBound = maxAllowedGas(baseline);
            expect(
                gasUsed.lte(upperBound),
                `${label} gas usage regressed: used ${gasUsed.toString()}, ` +
                    `expected <= ${upperBound} (baseline ${baseline} + ${GAS_TOLERANCE_PCT}% tolerance)`
            ).to.equal(true);
        }

        it("refreshBounty (single contributor) should stay within its gas baseline", async function () {
            const tx = await bountyRefresh.refreshBounty([addr1.address]);
            const receipt = await tx.wait();
            expectGasWithinBaseline(
                receipt.gasUsed,
                GAS_BASELINES.refreshBountySingle,
                "refreshBounty(single)"
            );
        });

        it("refreshBounty (batch of 3) should stay within its gas baseline", async function () {
            const contributors = [addr1.address, addr2.address, addr3.address];
            const tx = await bountyRefresh.refreshBounty(contributors);
            const receipt = await tx.wait();
            expectGasWithinBaseline(
                receipt.gasUsed,
                GAS_BASELINES.refreshBountyBatchOfThree,
                "refreshBounty(batch of 3)"
            );
        });

        it("refreshBountyParallel should stay within its gas baseline", async function () {
            const contributors = [addr1.address, addr2.address, addr3.address, addr4.address, addr5.address];
            const tx = await bountyRefresh.refreshBountyParallel(contributors, 2);
            const receipt = await tx.wait();
            expectGasWithinBaseline(
                receipt.gasUsed,
                GAS_BASELINES.refreshBountyParallel,
                "refreshBountyParallel"
            );
        });

        it("queueContributorsForRefresh should stay within its gas baseline", async function () {
            const contributors = [addr1.address, addr2.address, addr3.address];
            const tx = await bountyRefresh.queueContributorsForRefresh(contributors);
            const receipt = await tx.wait();
            expectGasWithinBaseline(
                receipt.gasUsed,
                GAS_BASELINES.queueContributorsForRefresh,
                "queueContributorsForRefresh"
            );
        });

        it("processPendingBatch should stay within its gas baseline", async function () {
            const contributors = [addr1.address, addr2.address, addr3.address];
            await bountyRefresh.queueContributorsForRefresh(contributors);
            const tx = await bountyRefresh.processPendingBatch(3);
            const receipt = await tx.wait();
            expectGasWithinBaseline(
                receipt.gasUsed,
                GAS_BASELINES.processPendingBatch,
                "processPendingBatch"
            );
        });
    });
});
