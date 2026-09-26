import type { FormMilestone } from "../../lib/validation";

export type WizardStep = 1 | 2 | 3 | 4;

export interface BountyDraftData {
  title: string;
  description: string;
  reward: string;
  maxAssignees: number;
  milestones: FormMilestone[];
  verifiers: string[];
  threshold: number;
}

export interface CreateBountyWizardProps {
  /** Optional custom submit callback. Defaults to calling api.createBounty. */
  onSubmit?: (data: BountyDraftData) => Promise<void>;
  /** Optional initial data to seed the wizard with. */
  initialData?: Partial<BountyDraftData>;
}
