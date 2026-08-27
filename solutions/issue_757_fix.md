**Patch**

```diff
--- a/src/contract/mutations.rs
+++ b/src/contract/mutations.rs
@@
 use crate::contract::errors::ContractError;
 use crate::contract::state::{Bounty, BountyId, BountyStore, ClaimStore};
 use crate::msg::{ClaimBountyResponse, ExecuteMsg};
 use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdResult};
 
+/// Guard that prevents the bounty creator from claiming their own bounty.
+/// The guard is intentionally placed at the very beginning of the
+/// `claim_bounty` mutation so that it short‑circuits any further logic
+/// and returns a clear, deterministic error.  The error type is defined
+/// in `errors.rs` as `CreatorCannotClaim`.
+fn guard_creator_cannot_claim(
+    info: &MessageInfo,
+    bounty: &Bounty,
+) -> Result<(), ContractError> {
+    if info.sender == bounty.creator {
+        return Err(ContractError::CreatorCannotClaim {
+            creator: bounty.creator.clone(),
+        });
+    }
+    Ok(())
+}
+
 pub fn claim_bounty(
     deps: DepsMut,
     env: Env,
     info: MessageInfo,
     bounty_id: BountyId,
 ) -> Result<Response, ContractError> {
-    // TODO: Implement claim logic
-    Ok(Response::default())
+    // Load the bounty from storage
+    let mut bounty = BountyStore::new(deps.storage).load(bounty_id)?;
+
+    // Guard: the creator cannot claim their own bounty
+    guard_creator_cannot_claim(&info, &bounty)?;
+
+    // Ensure the bounty is still open
+    if !bounty.is_open() {
+        return Err(ContractError::BountyClosed { bounty_id });
+    }
+
+    // Record the claim
+    let claim = ClaimStore::new(deps.storage).save(bounty_id, &info.sender)?;
+
+    // Emit a response
+    Ok(Response::new()
+        .add_attribute("action", "claim_bounty")
+        .add_attribute("bounty_id", bounty_id.to_string())
+        .add_attribute("claimer", info.sender.to_string())
+        .add_attribute("claim_id", claim.to_string()))
 }
```

```diff
--- a/src/contract/errors.rs
+++ b/src/contract/errors.rs
@@
 use schemars::JsonSchema;
 use serde::{Deserialize, Serialize};
 use thiserror::Error;
 
 #[derive(Error, Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
 pub enum ContractError {
@@
     #[error("Bounty {bounty_id} is closed")]
     BountyClosed { bounty_id: u64 },
+
+    /// Returned when the bounty creator attempts to claim their own bounty.
+    #[error("Creator {creator} cannot claim their own bounty")]
+    CreatorCannotClaim { creator: String },
 }
```

```diff
--- a/src/test.rs
+++ b/src/test.rs
@@
 use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
 use cosmwasm_std::{coins, Addr, Uint128};
 use crate::contract::{execute, instantiate, query};
 use crate::msg::{InstantiateMsg, ExecuteMsg, QueryMsg, ClaimBountyResponse};
 use crate::state::{BountyStore, BountyId};
 
+/// Helper that creates a bounty and returns its