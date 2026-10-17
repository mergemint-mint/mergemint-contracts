import React, { useMemo } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize';

interface BountyDescriptionProps {
  description?: string | null;
  className?: string;
  /**
   * When true, renders a compact preview (used by the create wizard) while
   * keeping the exact same Markdown pipeline as the detail view.
   */
  preview?: boolean;
}

// Strict sanitization schema: no raw HTML, no script tags, no javascript: URLs.
const sanitizeSchema = {
  ...defaultSchema,
  tagNames: [
    'p',
    'br',
    'strong',
    'em',
    'del',
    'blockquote',
    'ul',
    'ol',
    'li',
    'a',
    'code',
    'pre',
    'h1',
    'h2',
    'h3',
    'h4',
    'h5',
    'h6',
    'hr',
    'table',
    'thead',
    'tbody',
    'tr',
    'th',
    'td',
  ],
  attributes: {
    ...defaultSchema.attributes,
    a: [['href'], ['title']],
    code: [['className']],
  },
  protocols: {
    ...defaultSchema.protocols,
    href: ['http', 'https', 'mailto'],
  },
};

const isSafeHref = (href?: string): boolean => {
  if (!href) return false;
  const trimmed = href.trim().toLowerCase();
  return (
    trimmed.startsWith('http://') ||
    trimmed.startsWith('https://') ||
    trimmed.startsWith('mailto:') ||
    trimmed.startsWith('#') ||
    trimmed.startsWith('/')
  );
};

const BountyDescription: React.FC<BountyDescriptionProps> = ({
  description,
  className = '',
  preview = false,
}) => {
  const content = useMemo(() => (description ?? '').trim(), [description]);

  if (!content) {
    return (
      <p className={`text-sm italic text-gray-500 dark:text-gray-400 ${className}`}>
        No description provided.
      </p>
    );
  }

  return (
    <div
      className={`bounty-markdown text-sm leading-relaxed text-gray-800 dark:text-gray-200 ${
        preview ? 'max-h-64 overflow-y-auto' : ''
      } ${className}`}
    >
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[[rehypeSanitize, sanitizeSchema]]}
        skipHtml
        components={{
          a: ({ href, children, ...props }) => {
            if (!isSafeHref(href)) {
              return <span>{children}</span>;
            }
            return (
              <a
                {...props}
                href={href}
                target="_blank"
                rel="noopener noreferrer"
                className="text-indigo-600 underline hover:text-indigo-500 dark:text-indigo-400 dark:hover:text-indigo-300"
              >
                {children}
              </a>
            );
          },
          code: ({ className: codeClassName, children, ...props }) => {
            const isBlock = /language-/.test(codeClassName ?? '');
            if (isBlock) {
              return (
                <code
                  {...props}
                  className={`${codeClassName ?? ''} block`}
                >
                  {children}
                </code>
              );
            }
            return (
              <code
                {...props}
                className="rounded bg-gray-100 px-1.5 py-0.5 font-mono text-[0.85em] text-pink-600 dark:bg-gray-800 dark:text-pink-400"
              >
                {children}
              </code>
            );
          },
          pre: ({ children, ...props }) => (
            <pre
              {...props}
              className="my-3 overflow-x-auto rounded-lg border border-gray-200 bg-gray-50 p-3 font-mono text-xs text-gray-800 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-100"
            >
              {children}
            </pre>
          ),
          ul: ({ children, ...props }) => (
            <ul {...props} className="my-2 list-disc space-y-1 pl-5">
              {children}
            </ul>
          ),
          ol: ({ children, ...props }) => (
            <ol {...props} className="my-2 list-decimal space-y-1 pl-5">
              {children}
            </ol>
          ),
          blockquote: ({ children, ...props }) => (
            <blockquote
              {...props}
              className="my-3 border-l-4 border-gray-300 pl-4 italic text-gray-600 dark:border-gray-600 dark:text-gray-400"
            >
              {children}
            </blockquote>
          ),
          p: ({ children, ...props }) => (
            <p {...props} className="my-2">
              {children}
            </p>
          ),
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
};

export default BountyDescription;
