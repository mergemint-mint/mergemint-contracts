import React from 'react';
import { CreateBountyWizard } from '../components/CreateBountyWizard';

/**
 * Page component for creating a new bounty via the multi-step wizard.
 * @returns The rendered CreateBounty page
 */
export function CreateBounty() {
  return (
    <div className="create-bounty-page">
      <h1>Create a Bounty</h1>
      <CreateBountyWizard />
    </div>
  );
}

