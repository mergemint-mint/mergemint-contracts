import React, { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api } from "../../lib/api";
import { mapErrorMessage } from "../../utils/format";
import {
  SYMBOL_MAX_LENGTH,
  validateDetailsStep,
  validateMilestonesStep,
  validateRewardStep,
} from "../../lib/validation";
import type { BountyDraftData, CreateBountyWizardProps, WizardStep } from "./types";
import "./CreateBountyWizard.css";

export const CREATE_BOUNTY_DRAFT_KEY = "mergemint_create_bounty_draft";

const defaultDraft: BountyDraftData = {
  title: "",
  description: "",
  reward: "",
  maxAssignees: 1,
  milestones: [],
  verifiers: [],
  threshold: 1,
};

interface StoredDraftState {
  formData: BountyDraftData;
  step: WizardStep;
}

/**
 * Retrieves the persisted bounty draft from sessionStorage if available.
 * @returns Parsed draft state or null
 */
function loadDraftFromStorage(): StoredDraftState | null {
  try {
    const raw = sessionStorage.getItem(CREATE_BOUNTY_DRAFT_KEY);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && parsed.formData) {
      return parsed as StoredDraftState;
    }
    return null;
  } catch {
    return null;
  }
}

/**
 * Multi-step wizard for creating a bounty with per-step validation and session storage draft persistence.
 * @param props Component properties including optional onSubmit and initialData
 * @returns The rendered CreateBountyWizard component
 */
export function CreateBountyWizard({ onSubmit, initialData }: CreateBountyWizardProps) {
  const navigate = useNavigate();

  const [initialLoaded] = useState(() => loadDraftFromStorage());

  const [step, setStep] = useState<WizardStep>(() => {
    if (initialLoaded?.step) {
      return initialLoaded.step;
    }
    return 1;
  });

  const [formData, setFormData] = useState<BountyDraftData>(() => {
    return {
      ...defaultDraft,
      ...(initialLoaded?.formData ?? {}),
      ...(initialData ?? {}),
    };
  });

  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);

  useEffect(() => {
    try {
      const stateToStore: StoredDraftState = { formData, step };
      sessionStorage.setItem(CREATE_BOUNTY_DRAFT_KEY, JSON.stringify(stateToStore));
    } catch {
      return;
    }
  }, [formData, step]);

  const handleDetailsNext = useCallback(() => {
    const validation = validateDetailsStep({
      title: formData.title,
      description: formData.description,
    });
    if (!validation.valid) {
      setFieldErrors(validation.errors as Record<string, string>);
      return;
    }
    setFieldErrors({});
    setStep(2);
  }, [formData.title, formData.description]);

  const handleRewardNext = useCallback(() => {
    const validation = validateRewardStep({
      reward: formData.reward,
      maxAssignees: formData.maxAssignees,
    });
    if (!validation.valid) {
      setFieldErrors(validation.errors as Record<string, string>);
      return;
    }
    setFieldErrors({});
    setStep(3);
  }, [formData.reward, formData.maxAssignees]);

  const handleMilestonesNext = useCallback(() => {
    const validation = validateMilestonesStep({
      milestones: formData.milestones,
      totalReward: formData.reward,
      verifiers: formData.verifiers,
      threshold: formData.threshold,
    });
    if (!validation.valid) {
      setFieldErrors(validation.errors as Record<string, string>);
      return;
    }
    setFieldErrors({});
    setStep(4);
  }, [formData.milestones, formData.reward, formData.verifiers, formData.threshold]);

  const handleBack = useCallback(() => {
    setFieldErrors({});
    setSubmitError(null);
    setStep((prev) => (Math.max(1, prev - 1) as WizardStep));
  }, []);

  const handleAddMilestone = useCallback(() => {
    setFormData((prev) => ({
      ...prev,
      milestones: [...prev.milestones, { description: "", reward: "" }],
    }));
  }, []);

  const handleRemoveMilestone = useCallback((index: number) => {
    setFormData((prev) => ({
      ...prev,
      milestones: prev.milestones.filter((_, i) => i !== index),
    }));
  }, []);

  const handleMilestoneChange = useCallback(
    (index: number, field: "description" | "reward", value: string) => {
      setFormData((prev) => {
        const nextMilestones = [...prev.milestones];
        nextMilestones[index] = { ...nextMilestones[index], [field]: value };
        return { ...prev, milestones: nextMilestones };
      });
    },
    []
  );

  const handleAddVerifier = useCallback(() => {
    setFormData((prev) => ({
      ...prev,
      verifiers: [...prev.verifiers, ""],
    }));
  }, []);

  const handleRemoveVerifier = useCallback((index: number) => {
    setFormData((prev) => ({
      ...prev,
      verifiers: prev.verifiers.filter((_, i) => i !== index),
    }));
  }, []);

  const handleVerifierChange = useCallback((index: number, value: string) => {
    setFormData((prev) => {
      const nextVerifiers = [...prev.verifiers];
      nextVerifiers[index] = value;
      return { ...prev, verifiers: nextVerifiers };
    });
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setSubmitting(true);
      setSubmitError(null);

      try {
        if (onSubmit) {
          await onSubmit(formData);
        } else {
          const payload = {
            title: formData.title,
            description: formData.description,
            reward: formData.reward,
            maxAssignees: formData.maxAssignees,
            milestones: formData.milestones.map((m) => ({
              description: m.description,
              reward: m.reward,
              completed: false,
            })),
          };
          const bounty = await api.createBounty(payload);
          try {
            sessionStorage.removeItem(CREATE_BOUNTY_DRAFT_KEY);
          } catch {
            return;
          }
          navigate(`/bounties/${bounty.id}`);
          return;
        }

        try {
          sessionStorage.removeItem(CREATE_BOUNTY_DRAFT_KEY);
        } catch {
          return;
        }
      } catch (err) {
        setSubmitError(mapErrorMessage(err instanceof Error ? err.message : String(err)));
      } finally {
        setSubmitting(false);
      }
    },
    [formData, onSubmit, navigate]
  );

  const stepsList: Array<{ id: WizardStep; label: string }> = [
    { id: 1, label: "Details" },
    { id: 2, label: "Reward" },
    { id: 3, label: "Milestones" },
    { id: 4, label: "Review" },
  ];

  return (
    <div className="create-bounty-wizard" data-testid="create-bounty-wizard">
      <nav className="wizard-stepper" aria-label="Creation Progress">
        {stepsList.map((s) => (
          <div
            key={s.id}
            className={`wizard-step-indicator ${step === s.id ? "active" : ""} ${
              step > s.id ? "completed" : ""
            }`}
          >
            <div className="wizard-step-circle">{s.id}</div>
            <div className="wizard-step-label">{s.label}</div>
          </div>
        ))}
      </nav>

      {submitError && (
        <div role="alert" className="wizard-error-banner">
          {submitError}
        </div>
      )}

      {step === 1 && (
        <div className="wizard-step-body" data-testid="step-details">
          <div className="wizard-field-group">
            <label htmlFor="title">Title</label>
            <input
              id="title"
              aria-label="Title"
              className="wizard-input"
              value={formData.title}
              maxLength={SYMBOL_MAX_LENGTH}
              onChange={(e) => {
                setFormData((prev) => ({ ...prev, title: e.target.value }));
                if (fieldErrors.title) {
                  setFieldErrors((prev) => ({ ...prev, title: "" }));
                }
              }}
              placeholder="e.g. Build authentication"
            />
            <div className="wizard-field-footer">
              <span>Limit: {SYMBOL_MAX_LENGTH} chars</span>
              <span>
                {formData.title.length}/{SYMBOL_MAX_LENGTH}
              </span>
            </div>
            {fieldErrors.title && (
              <span role="alert" className="wizard-error-text">
                {fieldErrors.title}
              </span>
            )}
          </div>

          <div className="wizard-field-group">
            <label htmlFor="description">Description</label>
            <textarea
              id="description"
              aria-label="Description"
              className="wizard-textarea"
              rows={4}
              value={formData.description}
              maxLength={SYMBOL_MAX_LENGTH}
              onChange={(e) => {
                setFormData((prev) => ({ ...prev, description: e.target.value }));
                if (fieldErrors.description) {
                  setFieldErrors((prev) => ({ ...prev, description: "" }));
                }
              }}
              placeholder="Provide bounty scope"
            />
            <div className="wizard-field-footer">
              <span>Limit: {SYMBOL_MAX_LENGTH} chars</span>
              <span>
                {formData.description.length}/{SYMBOL_MAX_LENGTH}
              </span>
            </div>
            {fieldErrors.description && (
              <span role="alert" className="wizard-error-text">
                {fieldErrors.description}
              </span>
            )}
          </div>

          <div className="wizard-actions">
            <div />
            <button
              type="button"
              className="wizard-button wizard-button-primary"
              onClick={handleDetailsNext}
            >
              Next
            </button>
          </div>
        </div>
      )}

      {step === 2 && (
        <div className="wizard-step-body" data-testid="step-reward">
          <div className="wizard-field-group">
            <label htmlFor="reward-amount">Reward Amount</label>
            <input
              id="reward-amount"
              aria-label="Reward Amount"
              className="wizard-input"
              value={formData.reward}
              onChange={(e) => {
                setFormData((prev) => ({ ...prev, reward: e.target.value }));
                if (fieldErrors.reward) {
                  setFieldErrors((prev) => ({ ...prev, reward: "" }));
                }
              }}
              placeholder="Reward (XLM)"
            />
            {fieldErrors.reward && (
              <span role="alert" className="wizard-error-text">
                {fieldErrors.reward}
              </span>
            )}
          </div>

          <div className="wizard-field-group">
            <label htmlFor="max-assignees">Max Assignees</label>
            <input
              id="max-assignees"
              aria-label="Max Assignees"
              type="number"
              min={1}
              className="wizard-input"
              value={formData.maxAssignees}
              onChange={(e) => {
                const val = parseInt(e.target.value, 10);
                setFormData((prev) => ({
                  ...prev,
                  maxAssignees: Number.isNaN(val) ? 1 : val,
                }));
              }}
            />
            <div className="wizard-field-footer">
              <span>When more than one assignee is allowed, the reward is split across claimants by share.</span>
            </div>
            {fieldErrors.maxAssignees && (
              <span role="alert" className="wizard-error-text">
                {fieldErrors.maxAssignees}
              </span>
            )}
          </div>

          <div className="wizard-actions">
            <button type="button" className="wizard-button" onClick={handleBack}>
              Back
            </button>
            <button
              type="button"
              className="wizard-button wizard-button-primary"
              onClick={handleRewardNext}
            >
              Next
            </button>
          </div>
        </div>
      )}

      {step === 3 && (
        <div className="wizard-step-body" data-testid="step-milestones">
          <div className="wizard-field-group">
            <label>Milestones (Optional)</label>
            <div className="wizard-field-footer">
              <span>Split your bounty into milestones. Milestone rewards must sum to the total reward.</span>
            </div>

            {formData.milestones.map((milestone, idx) => (
              <div key={idx} className="wizard-milestone-item">
                <input
                  aria-label={`Milestone ${idx + 1} Description`}
                  placeholder="Milestone description"
                  className="wizard-input"
                  value={milestone.description}
                  onChange={(e) => handleMilestoneChange(idx, "description", e.target.value)}
                />
                <input
                  aria-label={`Milestone ${idx + 1} Reward`}
                  placeholder="Reward"
                  style={{ width: "120px" }}
                  className="wizard-input"
                  value={milestone.reward}
                  onChange={(e) => handleMilestoneChange(idx, "reward", e.target.value)}
                />
                <button
                  type="button"
                  className="wizard-button"
                  onClick={() => handleRemoveMilestone(idx)}
                >
                  Remove
                </button>
              </div>
            ))}

            <button
              type="button"
              className="wizard-button"
              style={{ width: "fit-content", marginTop: "8px" }}
              onClick={handleAddMilestone}
            >
              Add Milestone
            </button>

            {fieldErrors.milestones && (
              <span role="alert" className="wizard-error-text">
                {fieldErrors.milestones}
              </span>
            )}
          </div>

          <div className="wizard-field-group" style={{ marginTop: "16px" }}>
            <label>Required Verifiers (Optional)</label>
            {formData.verifiers.map((verifier, idx) => (
              <div key={idx} className="wizard-verifier-item">
                <input
                  aria-label={`Verifier ${idx + 1} Address`}
                  placeholder="Verifier public key or address"
                  className="wizard-input"
                  value={verifier}
                  onChange={(e) => handleVerifierChange(idx, e.target.value)}
                />
                <button
                  type="button"
                  className="wizard-button"
                  onClick={() => handleRemoveVerifier(idx)}
                >
                  Remove
                </button>
              </div>
            ))}

            <button
              type="button"
              className="wizard-button"
              style={{ width: "fit-content", marginTop: "8px" }}
              onClick={handleAddVerifier}
            >
              Add Verifier
            </button>

            {formData.verifiers.length > 0 && (
              <div style={{ marginTop: "12px" }}>
                <label htmlFor="approval-threshold">Approval Threshold</label>
                <input
                  id="approval-threshold"
                  aria-label="Approval Threshold"
                  type="number"
                  min={1}
                  max={formData.verifiers.length}
                  className="wizard-input"
                  value={formData.threshold}
                  onChange={(e) => {
                    const val = parseInt(e.target.value, 10);
                    setFormData((prev) => ({
                      ...prev,
                      threshold: Number.isNaN(val) ? 1 : val,
                    }));
                  }}
                />
              </div>
            )}

            {fieldErrors.verifiers && (
              <span role="alert" className="wizard-error-text">
                {fieldErrors.verifiers}
              </span>
            )}
          </div>

          <div className="wizard-actions">
            <button type="button" className="wizard-button" onClick={handleBack}>
              Back
            </button>
            <button
              type="button"
              className="wizard-button wizard-button-primary"
              onClick={handleMilestonesNext}
            >
              Next
            </button>
          </div>
        </div>
      )}

      {step === 4 && (
        <form onSubmit={handleSubmit} className="wizard-step-body" data-testid="step-review">
          <div className="wizard-summary-card">
            <div className="wizard-summary-row">
              <span className="wizard-summary-label">Title</span>
              <span className="wizard-summary-value">{formData.title}</span>
            </div>
            <div className="wizard-summary-row">
              <span className="wizard-summary-label">Description</span>
              <span className="wizard-summary-value">{formData.description}</span>
            </div>
            <div className="wizard-summary-row">
              <span className="wizard-summary-label">Reward</span>
              <span className="wizard-summary-value">{formData.reward} XLM</span>
            </div>
            <div className="wizard-summary-row">
              <span className="wizard-summary-label">Max Assignees</span>
              <span className="wizard-summary-value">{formData.maxAssignees}</span>
            </div>
            <div className="wizard-summary-row">
              <span className="wizard-summary-label">Milestones</span>
              <span className="wizard-summary-value">
                {formData.milestones.length === 0
                  ? "None"
                  : `${formData.milestones.length} milestone(s)`}
              </span>
            </div>
            {formData.verifiers.length > 0 && (
              <div className="wizard-summary-row">
                <span className="wizard-summary-label">Verifiers</span>
                <span className="wizard-summary-value">
                  {formData.verifiers.length} verifier(s), threshold: {formData.threshold}
                </span>
              </div>
            )}
          </div>

          <div className="wizard-actions">
            <button
              type="button"
              className="wizard-button"
              onClick={handleBack}
              disabled={submitting}
            >
              Back
            </button>
            <button
              type="submit"
              className="wizard-button wizard-button-primary"
              disabled={submitting}
            >
              {submitting ? "Creating..." : "Submit"}
            </button>
          </div>
        </form>
      )}
    </div>
  );
}
