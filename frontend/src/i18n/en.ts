const en = {
  // Nav
  nav_bounties: 'Bounties',
  nav_create_bounty: 'Create Bounty',
  nav_leaderboard: 'Leaderboard',
  nav_stats: 'Stats',
  nav_my_bounties: 'My Bounties',
  nav_admin: 'Admin',

  // BountyList
  filter_all: 'All',
  filter_created_by_me: 'Created by me',
  filter_assigned_to_me: 'Assigned to me',
  status_all: 'all',
  status_open: 'open',
  status_in_progress: 'in progress',
  status_claimed: 'claimed',
  status_submitted: 'submitted',
  status_disputed: 'disputed',
  status_completed: 'completed',
  status_cancelled: 'cancelled',
  status_expired: 'expired',
  status_archived: 'archived',
  load_more: 'Load more',
  loading: 'Loading...',

  // StatusBadge
  status_badge_label: 'Status: {status}',

  // Paused banner
  paused_notice_title: 'Contracts paused',
  paused_notice_body:
    'Contract actions are temporarily paused. You can still browse bounties; new claims and submissions will resume once the pause is lifted.',

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
  connect_wallet: 'Connect wallet',
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
