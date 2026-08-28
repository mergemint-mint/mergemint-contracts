# Event Schema

Contract events emitted for indexer integration.

## Events

### bounty_created
- **Topics**: `(Symbol("bounty_created"), creator_address)`
- **Data**: `(bounty_id, reward_amount)`
- **Trigger**: New bounty created
- **Purpose**: Notify indexer of new bounty

### bounty_claimed
- **Topics**: `(Symbol("bounty_claimed"), contributor_address)`
- **Data**: `bounty_id`
- **Trigger**: Contributor claims bounty
- **Purpose**: Notify indexer of bounty assignment

### bounty_disputed
- **Topics**: `(Symbol("bounty_disputed"), caller_address)`
- **Data**: `bounty_id`
- **Trigger**: Dispute raised on bounty
- **Purpose**: Notify indexer of dispute

### bounty_completed
- **Topics**: `(Symbol("bounty_completed"), contributor_address)`
- **Data**: `bounty_id`
- **Trigger**: Bounty completed and reward paid
- **Purpose**: Notify indexer of completion

### reward_paid
- **Topics**: `(Symbol("reward_paid"), contributor_address)`
- **Data**: `(bounty_id, amount)`
- **Trigger**: Token transfer confirmed
- **Purpose**: Confirm payment to indexer

### bounty_cancelled
- **Topics**: `(Symbol("bounty_cancelled"), creator_address)`
- **Data**: `bounty_id`
- **Trigger**: Creator cancels bounty
- **Purpose**: Notify indexer of bounty cancellation

### bounty_expired
- **Topics**: `(Symbol("bounty_expired"), creator_address)`
- **Data**: `bounty_id`
- **Trigger**: Bounty reaches deadline without completion
- **Purpose**: Notify indexer of bounty expiration

### approval_recorded
- **Topics**: `(Symbol("approval_recorded"), verifier_address)`
- **Data**: `(bounty_id, approval_count)`
- **Trigger**: Verifier approves completion
- **Purpose**: Notify indexer of verification progress

### dispute_resolved
- **Topics**: `(Symbol("dispute_resolved"), arbitrator_address)`
- **Data**: `(bounty_id, resolution)`
- **Trigger**: Arbitrator resolves dispute
- **Purpose**: Notify indexer of dispute outcome

### milestone_completed
- **Topics**: `(Symbol("milestone_completed"), milestone_index)`
- **Data**: `(bounty_id, amount)`
- **Trigger**: A milestone is completed and its staged reward paid
- **Purpose**: Notify indexer of milestone-level progress
