import { useState } from 'react';
import { CopyButton } from './CopyButton';
import { useTranslation } from '../i18n';

interface ShareButtonProps {
  /** The URL to share. Defaults to window.location.href. */
  url?: string;
  /** The title passed to the Web Share API. */
  title?: string;
  /** The text passed to the Web Share API. */
  text?: string;
}

/**
 * ShareButton — uses the Web Share API where available, falls back to
 * copying the URL to the clipboard via CopyButton.
 *
 * Issue #913.
 */
export function ShareButton({ url, title, text }: ShareButtonProps) {
  const { t } = useTranslation();
  const shareUrl = url ?? (typeof window !== 'undefined' ? window.location.href : '');
  const canShare =
    typeof navigator !== 'undefined' &&
    typeof navigator.share === 'function' &&
    typeof navigator.canShare === 'function' &&
    navigator.canShare({ url: shareUrl });

  const [sharing, setSharing] = useState(false);
  const [shareError, setShareError] = useState(false);

  async function handleShare() {
    setShareError(false);
    setSharing(true);
    try {
      await navigator.share({ url: shareUrl, title, text });
    } catch (err) {
      // AbortError means the user dismissed the share sheet — not a real error.
      if (err instanceof Error && err.name !== 'AbortError') {
        setShareError(true);
        setTimeout(() => setShareError(false), 2000);
      }
    } finally {
      setSharing(false);
    }
  }

  if (canShare) {
    return (
      <button
        type="button"
        className="share-button"
        onClick={handleShare}
        disabled={sharing}
        aria-label={t('share')}
      >
        {shareError ? t('copy_failed') : t('share')}
      </button>
    );
  }

  // Fallback: reuse CopyButton to copy the URL.
  return <CopyButton value={shareUrl} />;
}
