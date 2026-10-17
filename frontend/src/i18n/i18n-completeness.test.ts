import { describe, it, expect } from 'vitest';
import en from './en';

describe('i18n key completeness', () => {
  const enKeys = Object.keys(en) as Array<keyof typeof en>;

  it('en.ts has non-empty keys and string values', () => {
    expect(enKeys.length).toBeGreaterThan(0);
    for (const key of enKeys) {
      expect(typeof en[key]).toBe('string');
      expect((en[key] as string).trim().length).toBeGreaterThan(0);
    }
  });

  const localeModules = import.meta.glob<{ default?: Record<string, string> }>('./*.ts', {
    eager: true,
  });

  const candidateLocales = Object.entries(localeModules).filter(([filePath]) => {
    return (
      !filePath.endsWith('.test.ts') &&
      !filePath.endsWith('.d.ts') &&
      !filePath.endsWith('/en.ts') &&
      !filePath.endsWith('/index.ts')
    );
  });

  it('verifies that every other locale has all keys present in en.ts', () => {
    expect(candidateLocales.length).toBeGreaterThanOrEqual(1);

    for (const [filePath, moduleExports] of candidateLocales) {
      const catalog = moduleExports.default || (moduleExports as unknown as Record<string, string>);
      const catalogKeys = new Set(Object.keys(catalog));

      const missingKeys = enKeys.filter((key) => !catalogKeys.has(key));

      if (missingKeys.length > 0) {
        throw new Error(
          `Locale "${filePath}" is missing ${missingKeys.length} key(s):\n  - ${missingKeys.join('\n  - ')}`
        );
      }
      expect(missingKeys).toEqual([]);
    }
  });

  it('fails with a descriptive list of missing key names when a catalog is incomplete', () => {
    const incompleteCatalog = {
      nav_bounties: 'Bounties',
      nav_create_bounty: 'Create Bounty',
      // missing all other keys
    };

    const catalogKeys = new Set(Object.keys(incompleteCatalog));
    const missingKeys = enKeys.filter((key) => !catalogKeys.has(key));

    expect(missingKeys.length).toBeGreaterThan(0);
    expect(missingKeys).toContain('claim_bounty');
    expect(missingKeys).toContain('create_bounty');
    expect(missingKeys).toContain('error_boundary_message');
    expect(missingKeys).toContain('deadline_urgent');

    const errorFormat = `Missing ${missingKeys.length} keys: ${missingKeys.join(', ')}`;
    expect(errorFormat).toContain('claim_bounty');
  });
});
