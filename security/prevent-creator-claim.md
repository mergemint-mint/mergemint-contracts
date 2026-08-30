# Security: Prevent Creator Self-Claim

**Status**: Implemented in contract (see [docs/security.md](../docs/security.md) Threat #2).

## Summary
Creator self-claiming is strictly prevented in `claim_bounty` to eliminate wash trading and self-rewarding exploits.

## Regression tests

- `test_creator_cannot_claim_own_bounty` — single-assignee bounty
- `test_prevent_creator_claim_guard_multi_assignee_bounty` — creator blocked with open slots
- `test_prevent_creator_claim_guard_before_first_assignee` — creator blocked after another assignee claims
