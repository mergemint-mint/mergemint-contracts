// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "../../contracts/bounty/BountyRefresh.sol";

contract PanicRefresh is BountyRefresh {
    uint256 public mode;
    error Custom(uint256 x);

    function setMode(uint256 m) external { mode = m; }

    function _executeRefresh(address, uint256) external view override onlySelf {
        if (mode == 1) { uint256 z = 0; z = 1 / z; }
        if (mode == 2) revert Custom(7);
        if (mode == 3) revert(string(new bytes(500)));
    }
}

contract BountyRefreshErrorIsolationTest is Test {
    PanicRefresh c;

    function setUp() public { c = new PanicRefresh(); }

    function _run(uint256 mode) internal returns (string memory) {
        c.setMode(mode);
        address[] memory a = new address[](1);
        uint256[] memory b = new uint256[](1);
        a[0] = address(0xBEEF); b[0] = 1;
        uint256 id = c.createBatch(a, b);
        c.processBatchParallel(id);
        return c.getTask(c.taskCounter() - 1).errorMessage;
    }

    function testPanicDecoded() public {
        assertEq(_run(1), "Panic(18)");
    }

    function testCustomErrorSelector() public {
        assertEq(_run(2), string.concat("CustomError(0x", "", "", _hex(PanicRefresh.Custom.selector), ")"));
    }

    function testLongReasonBounded() public {
        assertEq(bytes(_run(3)).length, c.MAX_ERROR_LENGTH());
    }

    function _hex(bytes4 v) internal pure returns (string memory) {
        bytes memory h = "0123456789abcdef";
        bytes memory out = new bytes(8);
        for (uint256 i = 0; i < 4; ++i) {
            out[2 * i] = h[uint8(v[i]) >> 4];
            out[2 * i + 1] = h[uint8(v[i]) & 0x0f];
        }
        return string(out);
    }
}
