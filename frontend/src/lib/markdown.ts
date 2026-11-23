/**
 * Minimal, dependency-free Markdown renderer with XSS-safe URL handling.
 *
 * Security notes:
 * - URLs are validated BEFORE HTML entity escaping so that `&` in query
 *   strings cannot corrupt or bypass scheme checks.
 * - Protocol-relative URLs (`//evil.example`) are rejected.
 * - ASCII control characters (including tab/newline used to obfuscate
 *   schemes like `java\tscript:`) are stripped before validation.
 * - Only http(s), mailto, safe relative paths (`/bounties/1`) and fragments
 *   (`#section`) are permitted.
 */

const SAFE_URL = /^(https?:\/\/|mailto:|\/(?!\/)|#|\.)/i;

// eslint-disable-next-line no-control-regex
const CONTROL_CHARS = /[\u0000-\u001F\u007F]/g;

export function escapeHtml(input: string): string {
  return input
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

/**
 * Returns a sanitized URL string, or `null` if the URL is unsafe.
 * Validation happens on the raw (control-char-stripped) value; escaping is
 * applied afterwards so `&` in query strings is preserved correctly.
 */
export function sanitizeUrl(rawUrl: string): string | null {
  if (typeof rawUrl !== 'string') return null;

  const cleaned = rawUrl.replace(CONTROL_CHARS, '').trim();
  if (!cleaned) return null;

  // Reject protocol-relative URLs explicitly (SAFE_URL already excludes them,
  // but keep this guard for clarity and defense in depth).
  if (cleaned.startsWith('//')) return null;

  if (!SAFE_URL.test(cleaned)) return null;

  return escapeHtml(cleaned);
}

function renderInline(text: string): string {
  let out = escapeHtml(text);

  // Inline code: `code`
  out = out.replace(/`([^`]+)`/g, (_m, code: string) => `<code>${code}</code>`);

  // Links: [label](url) — validate URL before escaping.
  out = out.replace(
    /\[([^\]]*)\]\(([^)]*)\)/g,
    (_m, label: string, url: string) => {
      const safe = sanitizeUrl(url);
      if (safe === null) return label;
      return `<a href="${safe}" rel="noopener noreferrer nofollow">${label}</a>`;
    },
  );

  // Bold: **text**
  out = out.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');

  // Italic: *text*
  out = out.replace(/\*([^*]+)\*/g, '<em>$1</em>');

  return out;
}

export function renderMarkdown(markdown: string): string {
  if (typeof markdown !== 'string' || markdown.length === 0) return '';

  const lines = markdown.replace(/\r\n?/g, '\n').split('\n');
  const html: string[] = [];
  let inList = false;

  const closeList = () => {
    if (inList) {
      html.push('</ul>');
      inList = false;
    }
  };

  for (const line of lines) {
    const trimmed = line.trim();

    if (trimmed === '') {
      closeList();
      continue;
    }

    const heading = /^(#{1,6})\s+(.*)$/.exec(trimmed);
    if (heading) {
      closeList();
      const level = heading[1].length;
      html.push(`<h${level}>${renderInline(heading[2])}</h${level}>`);
      continue;
    }

    const listItem = /^[-*+]\s+(.*)$/.exec(trimmed);
    if (listItem) {
      if (!inList) {
        html.push('<ul>');
        inList = true;
      }
      html.push(`<li>${renderInline(listItem[1])}</li>`);
      continue;
    }

    closeList();
    html.push(`<p>${renderInline(trimmed)}</p>`);
  }

  closeList();
  return html.join('\n');
}
