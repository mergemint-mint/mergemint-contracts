import { describe, expect, it } from 'vitest';
import { escapeHtml, renderMarkdown, sanitizeUrl } from './markdown';

describe('escapeHtml', () => {
  it('escapes HTML metacharacters', () => {
    expect(escapeHtml('<script>alert("x")</script>')).toBe(
      '&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;',
    );
  });
});

describe('sanitizeUrl', () => {
  it('allows https URLs and preserves ampersands in query strings', () => {
    expect(sanitizeUrl('https://example.com/a?x=1&y=2')).toBe(
      'https://example.com/a?x=1&amp;y=2',
    );
  });

  it('allows safe relative paths and fragments', () => {
    expect(sanitizeUrl('/bounties/1')).toBe('/bounties/1');
    expect(sanitizeUrl('#section')).toBe('#section');
  });

  it('rejects protocol-relative URLs', () => {
    expect(sanitizeUrl('//evil.example/phish')).toBeNull();
  });

  it('rejects javascript: and obfuscated schemes', () => {
    expect(sanitizeUrl('javascript:alert(1)')).toBeNull();
    expect(sanitizeUrl('java\tscript:alert(1)')).toBeNull();
    expect(sanitizeUrl('java\nscript:alert(1)')).toBeNull();
  });

  it('rejects entity-encoded schemes', () => {
    expect(sanitizeUrl('&#106;avascript:alert(1)')).toBeNull();
  });

  it('rejects empty and control-only input', () => {
    expect(sanitizeUrl('')).toBeNull();
    expect(sanitizeUrl('\u0000\u0001')).toBeNull();
  });
});

describe('renderMarkdown', () => {
  it('renders safe links', () => {
    expect(renderMarkdown('[docs](https://example.com/a?x=1&y=2)')).toBe(
      '<p><a href="https://example.com/a?x=1&amp;y=2" rel="noopener noreferrer nofollow">docs</a></p>',
    );
  });

  it('drops unsafe link targets but keeps label text', () => {
    expect(renderMarkdown('[click](//evil.example/phish)')).toBe('<p>click</p>');
    expect(renderMarkdown('[click](javascript:alert(1))')).toBe('<p>click</p>');
  });

  it('escapes raw HTML in text', () => {
    expect(renderMarkdown('<img src=x onerror=alert(1)>')).toBe(
      '<p>&lt;img src=x onerror=alert(1)&gt;</p>',
    );
  });

  it('handles malformed brackets without throwing', () => {
    expect(() => renderMarkdown('[unclosed](https://example.com')).not.toThrow();
    expect(renderMarkdown('[[nested]](https://example.com)')).toContain('href=');
  });
});
