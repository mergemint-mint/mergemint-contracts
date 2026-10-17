// Lightweight, dependency-free Markdown renderer for bounty descriptions.
// HTML is disabled (raw HTML is escaped, never rendered) and links are
// sanitized so `javascript:`/`data:` URLs are stripped and external links
// get `rel="noopener noreferrer"` + `target="_blank"`.

const SAFE_URL = /^(https?:\/\/|mailto:|\/|#|\.)/i;

function escapeHtml(input: string): string {
  return input
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function sanitizeUrl(raw: string): string | null {
  const url = raw.trim();
  if (!url) return null;
  // Reject anything with a scheme that isn't explicitly allowed.
  if (/^[a-z][a-z0-9+.-]*:/i.test(url) && !SAFE_URL.test(url)) return null;
  if (!SAFE_URL.test(url)) return null;
  return url;
}

function renderInline(text: string): string {
  let out = escapeHtml(text);

  // Inline code first so its contents aren't further processed.
  const codeSlots: string[] = [];
  out = out.replace(/`([^`]+)`/g, (_m, code: string) => {
    const idx = codeSlots.push(`<code class="md-code-inline">${code}</code>`) - 1;
    return `\u0000CODE${idx}\u0000`;
  });

  // Links: [label](url)
  out = out.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_m, label: string, url: string) => {
    const safe = sanitizeUrl(url);
    if (!safe) return label;
    return `<a href="${safe}" target="_blank" rel="noopener noreferrer">${label}</a>`;
  });

  // Bold then italic.
  out = out.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  out = out.replace(/(^|[^*])\*([^*]+)\*/g, '$1<em>$2</em>');

  // Restore inline code.
  out = out.replace(/\u0000CODE(\d+)\u0000/g, (_m, i: string) => codeSlots[Number(i)] ?? '');

  return out;
}

/**
 * Render a Markdown string to sanitized HTML.
 * Raw HTML in the source is escaped, so `<script>` tags are shown as text.
 */
export function renderMarkdown(source: string): string {
  if (!source) return '';
  const lines = source.replace(/\r\n/g, '\n').split('\n');
  const html: string[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // Fenced code blocks.
    const fence = line.match(/^```(\w*)\s*$/);
    if (fence) {
      const lang = fence[1];
      const body: string[] = [];
      i++;
      while (i < lines.length && !/^```\s*$/.test(lines[i])) {
        body.push(lines[i]);
        i++;
      }
      i++; // skip closing fence
      const langClass = lang ? ` class="language-${escapeHtml(lang)}"` : '';
      html.push(`<pre class="md-code-block"><code${langClass}>${escapeHtml(body.join('\n'))}</code></pre>`);
      continue;
    }

    // Headings.
    const heading = line.match(/^(#{1,6})\s+(.*)$/);
    if (heading) {
      const level = heading[1].length;
      html.push(`<h${level} class="md-heading">${renderInline(heading[2])}</h${level}>`);
      i++;
      continue;
    }

    // Unordered lists.
    if (/^\s*[-*+]\s+/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^\s*[-*+]\s+/.test(lines[i])) {
        items.push(`<li>${renderInline(lines[i].replace(/^\s*[-*+]\s+/, ''))}</li>`);
        i++;
      }
      html.push(`<ul class="md-list">${items.join('')}</ul>`);
      continue;
    }

    // Ordered lists.
    if (/^\s*\d+\.\s+/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^\s*\d+\.\s+/.test(lines[i])) {
        items.push(`<li>${renderInline(lines[i].replace(/^\s*\d+\.\s+/, ''))}</li>`);
        i++;
      }
      html.push(`<ol class="md-list">${items.join('')}</ol>`);
      continue;
    }

    // Blank line.
    if (!line.trim()) {
      i++;
      continue;
    }

    // Paragraph: gather consecutive non-blank, non-block lines.
    const para: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() &&
      !/^```/.test(lines[i]) &&
      !/^#{1,6}\s+/.test(lines[i]) &&
      !/^\s*[-*+]\s+/.test(lines[i]) &&
      !/^\s*\d+\.\s+/.test(lines[i])
    ) {
      para.push(lines[i]);
      i++;
    }
    html.push(`<p class="md-paragraph">${renderInline(para.join(' '))}</p>`);
  }

  return html.join('\n');
}
