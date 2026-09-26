# Milestone Tracker - Developer Guide

## Quick Start

### Setup Branch

```bash
# Clone the repo if you haven't already
git clone https://github.com/mergemint-mint/mergemint-contracts.git
cd mergemint-contracts

# Create and checkout the feature branch
git checkout -b feature/milestone-tracker

# Install dependencies
npm install
cd frontend && npm install
```

### File Structure

```
frontend/src/
├── components/
│   ├── MilestoneTracker.tsx          ← Core component
│   ├── MilestoneTracker.test.tsx      ← Tests
│   └── ...existing components
├── pages/
│   ├── BountyDetail.tsx               ← Integration point
│   └── ...
├── styles/
│   └── theme.css                      ← New styling
└── types.ts
```

### Run Tests

```bash
cd frontend
npm test MilestoneTracker
```

Expected output:
```
✓ MilestoneTracker (11)
  ✓ renders nothing when milestones array is empty
  ✓ renders nothing when milestones is undefined
  ✓ renders 0% completion for no completed milestones
  ✓ renders partial completion correctly
  ✓ renders 100% completion when all milestones are done
  ✓ displays per-milestone rewards
  ✓ displays total reward when provided
  ✓ does not display total reward section when totalReward is not provided
  ✓ renders checkboxes as disabled for read-only display
  ✓ includes proper accessibility labels and roles
  ✓ calculates progress percentage correctly

Test Files  1 passed (1)
     Tests  11 passed (11)
```

## Implementation Details

### Component Structure

The `MilestoneTracker` component accepts two props:

```typescript
interface MilestoneTrackerProps {
  milestones: Milestone[];
  totalReward?: string;
}

interface Milestone {
  description: string;
  reward: string;
  completed: boolean;
}
```

### Rendering Logic

1. **Check for Empty State**: Returns `null` if no milestones
2. **Calculate Progress**: `completedCount / totalCount * 100`
3. **Render Header**: Title and summary count
4. **Render Progress Bar**: Visual indicator with dynamic width
5. **Render List**: Milestone items with checkboxes
6. **Render Footer**: Optional total reward

### CSS Naming Convention

Uses BEM (Block Element Modifier):

```css
.milestone-tracker                    /* Block */
.milestone-tracker__header            /* Element */
.milestone-tracker__progress-bar      /* Element */
.milestone-item                       /* Block */
.milestone-item__checkbox             /* Element */
.milestone-item--completed            /* Modifier */
.milestone-item--pending              /* Modifier */
```

## Testing Strategy

### Test Framework

Uses Vitest with `renderToStaticMarkup` for snapshot testing:

```typescript
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
```

### Test Categories

1. **Edge Cases** (empty, undefined)
2. **States** (0%, partial, 100%)
3. **Display** (rewards, total)
4. **Accessibility** (ARIA, labels)
5. **Calculation** (progress percentage)

### Adding New Tests

```typescript
it('should do something specific', () => {
  const milestones = [
    { description: 'Test', reward: '100 XLM', completed: false }
  ];
  
  const markup = renderToStaticMarkup(
    <MilestoneTracker milestones={milestones} />
  );
  
  expect(markup).toContain('expected-text');
});
```

## Styling Guide

### Theme Variables

Located in `frontend/src/styles/theme.css`, these are available everywhere:

```css
:root {
  --mm-bg: #ffffff;           /* Background */
  --mm-fg: #1a1d21;           /* Foreground/text */
  --mm-fg-muted: #5a6270;     /* Muted text */
  --mm-border: #d7dbe0;       /* Borders */
  --mm-success-fg: #1f7a3d;   /* Success/completed color */
}

@media (prefers-color-scheme: dark) {
  :root {
    --mm-bg: #17191c;
    --mm-fg: #e7e9ec;
    --mm-fg-muted: #9aa2ad;
    --mm-border: #3a3f46;
    --mm-success-fg: #8fd6a6;
  }
}
```

### Adding Styles

Add to `theme.css` with the pattern:

```css
/* ComponentName.tsx */
.component-name {
  background: var(--mm-bg);
  color: var(--mm-fg);
}

.component-name__element {
  /* styles */
}

.component-name--modifier {
  /* styles */
}
```

## Accessibility Checklist

✅ **Semantic HTML**
- Uses `<h2>` for milestone title
- Uses `<ul>` and `<li>` for list
- Uses `<input type="checkbox">` for indicators

✅ **ARIA Attributes**
- `role="region"` on container
- `role="progressbar"` on progress bar
- `aria-label` for descriptive text
- `aria-valuenow`, `aria-valuemin`, `aria-valuemax` on progress
- `aria-live="polite"` on updates
- `aria-label` on each milestone checkbox

✅ **Keyboard Navigation**
- All elements are focusable
- Tab order is logical
- Focus indicators are visible

✅ **Screen Reader**
- All content is announced
- Status updates are live regions
- Descriptions are clear and concise

## Integration Points

### In BountyDetail.tsx

Import the component:
```typescript
import { MilestoneTracker } from '../components/MilestoneTracker';
```

Render in JSX:
```jsx
<MilestoneTracker 
  milestones={bounty.milestones} 
  totalReward={bounty.reward} 
/>
```

### Data Flow

```
API Response (Bounty)
    ↓
BountyDetail Component
    ├── Bounty data
    └── Pass to MilestoneTracker
            ├── milestones prop
            └── totalReward prop
```

## Common Issues & Solutions

### Issue: Component doesn't render
**Solution**: Check that `bounty.milestones` exists and has items

```typescript
// Debug
console.log('Milestones:', bounty.milestones);
console.log('Has milestones:', bounty.milestones?.length > 0);
```

### Issue: Progress bar not visible
**Solution**: Ensure CSS is imported in `theme.css`

```bash
# Verify CSS is present
grep -n "milestone-tracker" frontend/src/styles/theme.css
```

### Issue: Dark mode colors wrong
**Solution**: Check `prefers-color-scheme` media query

```css
@media (prefers-color-scheme: dark) {
  /* Dark mode values */
}
```

### Issue: Tests failing
**Solution**: Run with verbose output

```bash
npm test MilestoneTracker -- --reporter=verbose
```

## Performance Considerations

### Component Optimization

- No useState/useEffect (pure component)
- Single pass through milestones array (O(n))
- Inline style only for dynamic width (CSS variable alternative available)

```typescript
// Current approach (efficient)
style={{ width: `${progressPercent}%` }}

// Alternative with CSS custom properties
<div 
  style={{ '--progress': progressPercent } as React.CSSProperties}
  className="milestone-tracker__progress-bar"
/>
```

### Rendering Performance

- Component re-renders only when props change
- No unnecessary DOM nodes created
- CSS transitions are hardware-accelerated

## Type Safety

### TypeScript Interfaces

```typescript
interface Milestone {
  description: string;  // Milestone title
  reward: string;       // e.g., "100 XLM"
  completed: boolean;   // true if done
}

interface MilestoneTrackerProps {
  milestones: Milestone[];
  totalReward?: string;  // Optional, e.g., "450 XLM"
}
```

### Existing Types

MilestoneTracker works with the existing `Bounty` type:

```typescript
interface Bounty {
  id: string;
  title: string;
  description: string;
  reward: string;  // ← Pass as totalReward
  status: BountyStatus;
  milestones: Array<{   // ← Pass directly
    description: string;
    reward: string;
    completed: boolean;
  }>;
  // ... other fields
}
```

## Deployment Checklist

Before submitting PR:

- [ ] All tests passing: `npm test`
- [ ] No TypeScript errors: `npm run type-check`
- [ ] No ESLint errors: `npm run lint`
- [ ] Prettier formatting applied: `npm run format`
- [ ] Files added to git
- [ ] Commit message follows guidelines
- [ ] Branch is up-to-date with main

## Useful Commands

```bash
# Run tests
npm test MilestoneTracker

# Run all tests
npm test

# Type checking
npx tsc --noEmit

# Linting
npx eslint frontend/src

# Format code
npx prettier --write frontend/src

# Build
npm run build
```

## Resources

- [Implementation Details](./docs/milestone-tracker-implementation.md)
- [Visual Reference](./docs/milestone-tracker-visual-reference.md)
- [ARIA Authoring Practices](https://www.w3.org/WAI/ARIA/apg/)
- [React Accessibility](https://reactjs.org/docs/accessibility.html)
- [BEM Naming Convention](https://getbem.com/)

## Questions?

- Check the implementation documentation
- Review test cases for usage examples
- Look at existing components for patterns
- Refer to the visual reference guide
