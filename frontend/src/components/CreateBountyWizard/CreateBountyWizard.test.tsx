import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { MemoryRouter } from "react-router-dom";
import { CreateBountyWizard, CREATE_BOUNTY_DRAFT_KEY } from "./CreateBountyWizard";

describe("CreateBountyWizard", () => {
  beforeEach(() => {
    sessionStorage.clear();
    vi.clearAllMocks();
  });

  it("renders Step 1 (Details) initially", () => {
    render(
      <MemoryRouter>
        <CreateBountyWizard />
      </MemoryRouter>
    );

    expect(screen.getByTestId("step-details")).toBeTruthy();
    expect(screen.getByLabelText("Title")).toBeTruthy();
    expect(screen.getByLabelText("Description")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Next" })).toBeTruthy();
  });

  it("blocks Step 1 advancement when inputs are invalid", async () => {
    render(
      <MemoryRouter>
        <CreateBountyWizard />
      </MemoryRouter>
    );

    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    expect(screen.getByText("Title is required.")).toBeTruthy();
    expect(screen.getByText("Description is required.")).toBeTruthy();
    expect(screen.getByTestId("step-details")).toBeTruthy();
  });

  it("advances from Step 1 to Step 2 upon valid input", async () => {
    render(
      <MemoryRouter>
        <CreateBountyWizard />
      </MemoryRouter>
    );

    fireEvent.change(screen.getByLabelText("Title"), {
      target: { value: "Test Bounty" },
    });
    fireEvent.change(screen.getByLabelText("Description"), {
      target: { value: "Test Description" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    expect(screen.getByTestId("step-reward")).toBeTruthy();
    expect(screen.getByLabelText("Reward Amount")).toBeTruthy();
  });

  it("supports back navigation preserving user input", async () => {
    render(
      <MemoryRouter>
        <CreateBountyWizard />
      </MemoryRouter>
    );

    fireEvent.change(screen.getByLabelText("Title"), {
      target: { value: "Saved Title" },
    });
    fireEvent.change(screen.getByLabelText("Description"), {
      target: { value: "Saved Desc" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByTestId("step-reward")).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Reward Amount"), {
      target: { value: "100" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByTestId("step-details")).toBeTruthy();

    expect((screen.getByLabelText("Title") as HTMLInputElement).value).toBe("Saved Title");
    expect((screen.getByLabelText("Description") as HTMLTextAreaElement).value).toBe("Saved Desc");

    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByTestId("step-reward")).toBeTruthy();
    expect((screen.getByLabelText("Reward Amount") as HTMLInputElement).value).toBe("100");
  });

  it("validates reward in Step 2 and advances to Step 3", async () => {
    render(
      <MemoryRouter>
        <CreateBountyWizard />
      </MemoryRouter>
    );

    fireEvent.change(screen.getByLabelText("Title"), {
      target: { value: "Bounty Title" },
    });
    fireEvent.change(screen.getByLabelText("Description"), {
      target: { value: "Bounty Desc" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByText("Reward amount is required.")).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Reward Amount"), {
      target: { value: "not-a-number" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(
      screen.getByText("Enter a positive number with up to 7 decimal places.")
    ).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Reward Amount"), {
      target: { value: "50" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    expect(screen.getByTestId("step-milestones")).toBeTruthy();
  });

  it("validates milestones in Step 3 before advancing to Step 4", async () => {
    render(
      <MemoryRouter>
        <CreateBountyWizard />
      </MemoryRouter>
    );

    fireEvent.change(screen.getByLabelText("Title"), { target: { value: "Title" } });
    fireEvent.change(screen.getByLabelText("Description"), { target: { value: "Desc" } });
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    fireEvent.change(screen.getByLabelText("Reward Amount"), { target: { value: "50" } });
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    expect(screen.getByTestId("step-milestones")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Add Milestone" }));
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByText(/requires a description/)).toBeTruthy();

    fireEvent.change(screen.getByPlaceholderText("Milestone description"), {
      target: { value: "Milestone 1" },
    });
    fireEvent.change(screen.getByPlaceholderText("Reward"), {
      target: { value: "20" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByText(/Sum of milestone rewards must equal/)).toBeTruthy();

    fireEvent.change(screen.getByPlaceholderText("Reward"), {
      target: { value: "50" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    expect(screen.getByTestId("step-review")).toBeTruthy();
  });

  it("displays review summary and invokes onSubmit", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);

    render(
      <MemoryRouter>
        <CreateBountyWizard onSubmit={onSubmit} />
      </MemoryRouter>
    );

    fireEvent.change(screen.getByLabelText("Title"), { target: { value: "Final Title" } });
    fireEvent.change(screen.getByLabelText("Description"), { target: { value: "Final Desc" } });
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    fireEvent.change(screen.getByLabelText("Reward Amount"), { target: { value: "75" } });
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    expect(screen.getByTestId("step-review")).toBeTruthy();
    expect(screen.getByText("Final Title")).toBeTruthy();
    expect(screen.getByText("Final Desc")).toBeTruthy();
    expect(screen.getByText("75 XLM")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalledOnce();
    });

    expect(sessionStorage.getItem(CREATE_BOUNTY_DRAFT_KEY)).toBeNull();
  });

  it("saves draft in sessionStorage and restores on refresh", () => {
    const { unmount } = render(
      <MemoryRouter>
        <CreateBountyWizard />
      </MemoryRouter>
    );

    fireEvent.change(screen.getByLabelText("Title"), {
      target: { value: "Persisted Title" },
    });
    fireEvent.change(screen.getByLabelText("Description"), {
      target: { value: "Persisted Desc" },
    });

    unmount();

    render(
      <MemoryRouter>
        <CreateBountyWizard />
      </MemoryRouter>
    );

    expect((screen.getByLabelText("Title") as HTMLInputElement).value).toBe("Persisted Title");
    expect((screen.getByLabelText("Description") as HTMLTextAreaElement).value).toBe(
      "Persisted Desc"
    );
  });
});
