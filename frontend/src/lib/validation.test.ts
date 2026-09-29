import { describe, expect, it } from "vitest";
import {
  SYMBOL_MAX_LENGTH,
  isValidRewardAmount,
  isValidDescriptionLength,
  isValidTitle,
  isValidContractAddress,
  isValidMaxAssignees,
  validateDetailsStep,
  validateRewardStep,
  validateMilestonesStep,
} from "./validation";

describe("validation.ts unit tests", () => {
  describe("SYMBOL_MAX_LENGTH", () => {
    it("is 32 characters", () => {
      expect(SYMBOL_MAX_LENGTH).toBe(32);
    });
  });

  describe("isValidRewardAmount", () => {
    it("accepts valid reward values", () => {
      expect(isValidRewardAmount("10")).toBe(true);
      expect(isValidRewardAmount("10.5")).toBe(true);
      expect(isValidRewardAmount("0.0000001")).toBe(true);
      expect(isValidRewardAmount("1000000")).toBe(true);
    });

    it("rejects invalid reward values", () => {
      expect(isValidRewardAmount("0")).toBe(false);
      expect(isValidRewardAmount("-5")).toBe(false);
      expect(isValidRewardAmount("10.12345678")).toBe(false);
      expect(isValidRewardAmount("")).toBe(false);
      expect(isValidRewardAmount("abc")).toBe(false);
      expect(isValidRewardAmount("1e10")).toBe(false);
      expect(isValidRewardAmount("10.")).toBe(false);
    });
  });

  describe("isValidTitle & isValidDescriptionLength", () => {
    it("validates within SYMBOL_MAX_LENGTH", () => {
      expect(isValidTitle("Short title")).toBe(true);
      expect(isValidTitle("a".repeat(32))).toBe(true);
      expect(isValidTitle("a".repeat(33))).toBe(false);
      expect(isValidTitle("")).toBe(false);
      expect(isValidTitle("   ")).toBe(false);

      expect(isValidDescriptionLength("Valid description")).toBe(true);
      expect(isValidDescriptionLength("a".repeat(32))).toBe(true);
      expect(isValidDescriptionLength("a".repeat(33))).toBe(false);
      expect(isValidDescriptionLength("")).toBe(false);
      expect(isValidDescriptionLength("   ")).toBe(false);
    });
  });

  describe("isValidContractAddress", () => {
    it("validates 56 character Soroban contract addresses starting with C", () => {
      const validAddress = "CA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVOGO";
      expect(isValidContractAddress(validAddress)).toBe(true);
      expect(isValidContractAddress("GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVOGO")).toBe(false);
      expect(isValidContractAddress("short")).toBe(false);
    });
  });

  describe("isValidMaxAssignees", () => {
    it("requires integers >= 1", () => {
      expect(isValidMaxAssignees(1)).toBe(true);
      expect(isValidMaxAssignees(5)).toBe(true);
      expect(isValidMaxAssignees(0)).toBe(false);
      expect(isValidMaxAssignees(-1)).toBe(false);
      expect(isValidMaxAssignees(1.5)).toBe(false);
    });
  });

  describe("validateDetailsStep", () => {
    it("returns errors for missing title and description", () => {
      const res = validateDetailsStep({ title: "", description: "" });
      expect(res.valid).toBe(false);
      expect(res.errors.title).toBe("Title is required.");
      expect(res.errors.description).toBe("Description is required.");
    });

    it("returns errors for values exceeding SYMBOL_MAX_LENGTH", () => {
      const res = validateDetailsStep({
        title: "x".repeat(33),
        description: "y".repeat(35),
      });
      expect(res.valid).toBe(false);
      expect(res.errors.title).toContain("32 characters or less");
      expect(res.errors.description).toContain("32 characters or less");
    });

    it("passes for valid title and description", () => {
      const res = validateDetailsStep({
        title: "My Feature",
        description: "Add wizard support",
      });
      expect(res.valid).toBe(true);
      expect(res.errors).toEqual({});
    });
  });

  describe("validateRewardStep", () => {
    it("returns errors for missing or invalid reward", () => {
      const empty = validateRewardStep({ reward: "" });
      expect(empty.valid).toBe(false);
      expect(empty.errors.reward).toBe("Reward amount is required.");

      const invalid = validateRewardStep({ reward: "abc" });
      expect(invalid.valid).toBe(false);
      expect(invalid.errors.reward).toBe("Enter a positive number with up to 7 decimal places.");
    });

    it("validates maxAssignees when provided", () => {
      const invalidAssignees = validateRewardStep({ reward: "100", maxAssignees: 0 });
      expect(invalidAssignees.valid).toBe(false);
      expect(invalidAssignees.errors.maxAssignees).toBe("Max assignees must be at least 1.");

      const validAssignees = validateRewardStep({ reward: "100", maxAssignees: 3 });
      expect(validAssignees.valid).toBe(true);
      expect(validAssignees.errors).toEqual({});
    });
  });

  describe("validateMilestonesStep", () => {
    it("passes when no milestones or verifiers are provided", () => {
      const res = validateMilestonesStep({});
      expect(res.valid).toBe(true);
      expect(res.errors).toEqual({});
    });

    it("returns errors when milestone fields are empty or invalid", () => {
      const res1 = validateMilestonesStep({
        milestones: [{ description: "", reward: "10" }],
      });
      expect(res1.valid).toBe(false);
      expect(res1.errors.milestones).toContain("requires a description");

      const res2 = validateMilestonesStep({
        milestones: [{ description: "Phase 1", reward: "-5" }],
      });
      expect(res2.valid).toBe(false);
      expect(res2.errors.milestones).toContain("invalid reward amount");
    });

    it("returns error when sum of milestone rewards does not match total reward", () => {
      const res = validateMilestonesStep({
        totalReward: "100",
        milestones: [
          { description: "Phase 1", reward: "40" },
          { description: "Phase 2", reward: "50" },
        ],
      });
      expect(res.valid).toBe(false);
      expect(res.errors.milestones).toContain("Sum of milestone rewards must equal");
    });

    it("passes when milestone rewards sum up to total reward", () => {
      const res = validateMilestonesStep({
        totalReward: "100",
        milestones: [
          { description: "Phase 1", reward: "40" },
          { description: "Phase 2", reward: "60" },
        ],
      });
      expect(res.valid).toBe(true);
      expect(res.errors).toEqual({});
    });

    it("validates threshold against active verifiers count", () => {
      const invalidThreshold = validateMilestonesStep({
        verifiers: ["v1", "v2"],
        threshold: 3,
      });
      expect(invalidThreshold.valid).toBe(false);
      expect(invalidThreshold.errors.verifiers).toContain("must be between 1 and 2");

      const validThreshold = validateMilestonesStep({
        verifiers: ["v1", "v2"],
        threshold: 2,
      });
      expect(validThreshold.valid).toBe(true);
    });
  });
});
