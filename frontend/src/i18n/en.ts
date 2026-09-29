const en = {
  // Nav
  nav_bounties: 'Bounties',
  nav_create_bounty: 'Create Bounty',

  // BountyList
  filter_all: 'All',
  filter_created_by_me: 'Created by me',
  filter_assigned_to_me: 'Assigned to me',
  status_all: 'all',
  status_open: 'open',
  status_claimed: 'claimed',
  status_disputed: 'disputed',
  status_completed: 'completed',
  status_cancelled: 'cancelled',
  load_more: 'Load more',
  loading: 'Loading...',

  // BountyDetail (page)
  claiming: 'Claiming...',
  claim_bounty: 'Claim Bounty',

  // CreateBounty (page)
  placeholder_title: 'Title',
  placeholder_description: 'Description',
  placeholder_reward: 'Reward (XLM)',
  creating: 'Creating...',
  create_bounty: 'Create Bounty',

  // ContributorProfile
  connect_wallet_prompt: 'Connect your wallet to view contributor profiles.',
  reputation: 'Reputation',
  completed_bounties: 'Completed bounties',

  // BountyCard
  loading_bounty: 'Loading bounty',

  // BountyErrorBoundary
  error_boundary_message: 'Something went wrong.',
  retry: 'Retry',

  // ShareButton
  share: 'Share',
  copy_link: 'Copy link',
  copied: 'Copied!',
  copy_failed: 'Copy failed',

  // CopyButton
  copy_to_clipboard: 'Copy to clipboard',

  // Deadline
  deadline_label: 'Deadline',
  deadline_urgent: 'Less than 24 hours left!',
} as const;

export type TranslationKey = keyof typeof en;
export default en;
