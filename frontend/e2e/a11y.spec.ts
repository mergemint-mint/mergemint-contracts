import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

const CRITICAL_IMPACTS = ['critical', 'serious'];

async function assertNoCriticalViolations(page: import('@playwright/test').Page) {
  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();

  const violations = results.violations.filter((v) =>
    CRITICAL_IMPACTS.includes(v.impact ?? ''),
  );

  expect(
    violations,
    `Critical/serious axe violations: ${violations
      .map((v) => `${v.id} (${v.impact})`)
      .join(', ')}`,
  ).toEqual([]);
}

test.describe('Accessibility (WCAG 2.1 AA)', () => {
  test('home page has no critical/serious violations', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await assertNoCriticalViolations(page);
  });

  test('submitted contract state is accessible', async ({ page }) => {
    await page.goto('/contracts?status=submitted');
    await page.waitForLoadState('networkidle');
    await expect(page.getByTestId('status-badge-submitted').first()).toBeVisible();
    await assertNoCriticalViolations(page);
  });

  test('archived contract state is accessible', async ({ page }) => {
    await page.goto('/contracts?status=archived');
    await page.waitForLoadState('networkidle');
    await expect(page.getByTestId('status-badge-archived').first()).toBeVisible();
    await assertNoCriticalViolations(page);
  });

  test('paused banner is accessible', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    const banner = page.getByRole('status').filter({ hasText: /paused/i });
    if (await banner.count()) {
      await expect(banner.first()).toBeVisible();
    }
    await assertNoCriticalViolations(page);
  });
});

test.describe('Accessibility — dark mode contrast', () => {
  test.use({ colorScheme: 'dark' });

  test('submitted, archived, and paused states pass contrast in dark mode', async ({
    page,
  }) => {
    await page.goto('/contracts?status=submitted');
    await page.waitForLoadState('networkidle');
    await assertNoCriticalViolations(page);

    await page.goto('/contracts?status=archived');
    await page.waitForLoadState('networkidle');
    await assertNoCriticalViolations(page);

    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await assertNoCriticalViolations(page);
  });
});
