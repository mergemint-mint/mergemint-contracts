# Milestone Tracker - Visual Reference

## Component Structure

```
┌─────────────────────────────────────────────────┐
│ Milestones                          2 of 3 done │
├─────────────────────────────────────────────────┤
│ [██████████████████░░░░░░░░░░░░░░░]  66%       │
├─────────────────────────────────────────────────┤
│ ☑ Design Review              100 XLM           │
├─────────────────────────────────────────────────┤
│ ☑ Implementation              200 XLM           │
├─────────────────────────────────────────────────┤
│ ☐ Testing & QA                150 XLM           │
├─────────────────────────────────────────────────┤
│ Total Reward:                  450 XLM          │
└─────────────────────────────────────────────────┘
```

## States

### No Milestones
- Component returns null (hidden from view)
- No empty state message shown

### 0% Complete (0 of N)
```
Milestones                          0 of 3 done
[░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 0%

☐ Design Review              100 XLM
☐ Implementation              200 XLM
☐ Testing & QA                150 XLM
```

### Partial (X of N)
```
Milestones                          2 of 3 done
[██████████████████░░░░░░░░░░░░░░░] 66%

☑ Design Review              100 XLM
☑ Implementation              200 XLM
☐ Testing & QA                150 XLM
```

(Completed items have strikethrough text)

### 100% Complete (N of N)
```
Milestones                          3 of 3 done
[████████████████████████████████] 100%

☑ Design Review              100 XLM
☑ Implementation              200 XLM
☑ Testing & QA                150 XLM

Total Reward:                  450 XLM
```

## Dark Mode vs Light Mode

### Light Mode (Default)
- Background: White (#ffffff)
- Border: Light gray (#d7dbe0)
- Text: Dark (#1a1d21)
- Progress bar: Green (#1f7a3d)

### Dark Mode
- Background: Dark gray (#17191c)
- Border: Dark (#3a3f46)
- Text: Light (#e7e9ec)
- Progress bar: Light green (#8fd6a6)

## Responsive Behavior

### Desktop (>600px)
```
┌──────────────────────────────────────────────────┐
│ Milestones                      2 of 3 completed │
│ [████████████░░░░░░░░░░░░░░░░░] 66%             │
│                                                  │
│ ☑ First Milestone Name                  100 XLM │
│ ☑ Second Milestone Name                 200 XLM │
│ ☐ Third Milestone Name                  150 XLM │
│                                                  │
│ Total Reward:                           450 XLM │
└──────────────────────────────────────────────────┘
```

### Mobile (<600px)
```
┌────────────────────────────────┐
│ Milestones        2 of 3 done  │
│ [████████░░░░░] 66%            │
│                                │
│ ☑ First Milestone              │
│    100 XLM                      │
│ ☑ Second Milestone             │
│    200 XLM                      │
│ ☐ Third Milestone              │
│    150 XLM                      │
│                                │
│ Total: 450 XLM                 │
└────────────────────────────────┘
```

## Integration in BountyDetail Page

### Page Layout
```
┌─────────────────────────────────────────────┐
│ BOUNTY TITLE                        [OPEN]   │
├─────────────────────────────────────────────┤
│ Bounty description goes here...             │
│                                             │
│ Lorem ipsum dolor sit amet...               │
├─────────────────────────────────────────────┤
│ Milestones                    2 of 3 done  │
│ [████████████░░░░░░░░] 66%                 │
│                                             │
│ ☑ Design Review           100 XLM          │
│ ☑ Development             200 XLM          │
│ ☐ Testing                 150 XLM          │
│                                             │
│ Total Reward:              450 XLM         │
├─────────────────────────────────────────────┤
│ [ Claim Bounty ]                            │
└─────────────────────────────────────────────┘
```

## Accessibility Indicators

### Screen Reader Announcement Order
1. Region label: "Milestone progress"
2. Title: "Milestones"
3. Summary: "2 of 3 completed" (aria-live polite)
4. Progress bar: "Milestone progress: 2 of 3 completed"
5. List items with labels: "Milestone 1: Design Review"
6. Total reward section

### Focus Order
When tabbing through the page:
1. Bounty title (h1)
2. Status badge
3. Milestone tracker section (can be focused)
4. Each milestone checkbox (skippable with Tab)
5. Claim button

### Keyboard Navigation
- Tab: Navigate to milestone tracker region
- Shift+Tab: Navigate back
- All interactive elements have visible focus indicators

## Color Semantics

- ✅ **Completed** (Green): #1f7a3d (light), #8fd6a6 (dark)
  - With strikethrough text
  - Checked checkbox indicator

- ⏳ **Pending** (Default text color): #1a1d21 (light), #e7e9ec (dark)
  - No strikethrough
  - Unchecked checkbox indicator

- **Progress Fill** (Success green): Same as completed state
  - Smooth animation (0.3s ease)

## Testing Scenarios

### Test Case: 0% Completion
```
Input: 
  milestones: [
    { description: 'Design', reward: '100 XLM', completed: false },
    { description: 'Dev', reward: '200 XLM', completed: false }
  ]

Expected Output:
  - "0 of 2 completed"
  - Progress bar width: 0%
  - Both items marked pending
  - All checkboxes unchecked
```

### Test Case: 50% Completion
```
Input:
  milestones: [
    { description: 'Design', reward: '100 XLM', completed: true },
    { description: 'Dev', reward: '200 XLM', completed: false }
  ]

Expected Output:
  - "1 of 2 completed"
  - Progress bar width: 50%
  - First item completed, second pending
  - First checkbox checked, second unchecked
```

### Test Case: 100% Completion
```
Input:
  milestones: [
    { description: 'Design', reward: '100 XLM', completed: true },
    { description: 'Dev', reward: '200 XLM', completed: true }
  ]

Expected Output:
  - "2 of 2 completed"
  - Progress bar width: 100%
  - All items completed
  - All checkboxes checked
  - Total reward displayed
```

## Interaction Patterns

### User Viewing Milestone Progress
1. User visits bounty detail page
2. Sees milestone tracker immediately below description
3. Glances at progress bar to see overall completion
4. Reads "2 of 3 completed" for quick summary
5. Scans list to see which specific milestones are done
6. Sees individual reward amounts for transparency
7. Sees total to understand full bounty value

### Creator Monitoring Progress
1. Creator sees progress tracker on their own bounties
2. Can determine which phases are complete
3. Can see reward distribution
4. Can verify payments match completed milestones

### Smart Contract Interaction
- Milestones are marked complete by the contract, not the UI
- UI only reflects the read-only state from the blockchain
- Checkboxes cannot be interacted with (disabled)
- Progress is updated when page data refreshes from API
