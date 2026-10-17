import { useState } from 'react';
import { marked } from 'marked';
import DOMPurify from 'dompurify';

interface CreateBountyWizardProps {
  onSubmit?: (data: { title: string; description: string; reward: string }) => void;
}

function renderMarkdown(markdown: string): string {
  const rawHtml = marked.parse(markdown, { async: false }) as string;
  return DOMPurify.sanitize(rawHtml, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ['script', 'style', 'iframe', 'object', 'embed'],
    FORBID_ATTR: ['onerror', 'onload', 'onclick'],
  });
}

export default function CreateBountyWizard({ onSubmit }: CreateBountyWizardProps) {
  const [step, setStep] = useState(0);
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [reward, setReward] = useState('');

  const handleSubmit = () => {
    onSubmit?.({ title, description, reward });
  };

  return (
    <div className="mx-auto w-full max-w-2xl p-4 sm:p-6">
      <h2 className="mb-4 text-xl font-semibold text-gray-900 dark:text-gray-100">
        Create Bounty
      </h2>

      {step === 0 && (
        <div className="space-y-4">
          <div>
            <label className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              Title
            </label>
            <input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              className="w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-gray-900 focus:border-blue-500 focus:outline-none dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              placeholder="Bounty title"
            />
          </div>

          <div>
            <label className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              Description (Markdown supported)
            </label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={6}
              className="w-full rounded-md border border-gray-300 bg-white px-3 py-2 font-mono text-sm text-gray-900 focus:border-blue-500 focus:outline-none dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              placeholder="Describe the bounty... Markdown is supported"
            />
          </div>

          {description.trim() && (
            <div>
              <span className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
                Preview
              </span>
              <div
                className="markdown-preview prose prose-sm max-w-none rounded-md border border-gray-200 bg-gray-50 p-3 text-gray-900 dark:prose-invert dark:border-gray-700 dark:bg-gray-900 dark:text-gray-100"
                dangerouslySetInnerHTML={{ __html: renderMarkdown(description) }}
              />
            </div>
          )}

          <div>
            <label className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300">
              Reward
            </label>
            <input
              type="text"
              value={reward}
              onChange={(e) => setReward(e.target.value)}
              className="w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-gray-900 focus:border-blue-500 focus:outline-none dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
              placeholder="e.g. 100 XLM"
            />
          </div>

          <div className="flex justify-end">
            <button
              type="button"
              onClick={() => setStep(1)}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
            >
              Next
            </button>
          </div>
        </div>
      )}

      {step === 1 && (
        <div className="space-y-4">
          <div>
            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">{title}</h3>
            <div
              className="markdown-preview prose prose-sm mt-2 max-w-none text-gray-900 dark:prose-invert dark:text-gray-100"
              dangerouslySetInnerHTML={{ __html: renderMarkdown(description) }}
            />
            <p className="mt-2 text-sm text-gray-600 dark:text-gray-400">Reward: {reward}</p>
          </div>

          <div className="flex justify-between">
            <button
              type="button"
              onClick={() => setStep(0)}
              className="rounded-md border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-800"
            >
              Back
            </button>
            <button
              type="button"
              onClick={handleSubmit}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
            >
              Create Bounty
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
