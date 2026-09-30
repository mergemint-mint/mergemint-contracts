import { useState } from 'react';
import { CopyButton } from './CopyButton';
import { useTranslation } from '../i18n';

/**
 * Properties for the ShareButton component.
 */
export interface ShareButtonProps {
  /** The URL to share. Defaults to window.location.href. */
  url?: string;
  /** The title passed to the Web Share API. */
  title?: string;
  /** The text description passed to the Web Share API. */
  text?: string;
  /** Optional additional CSS class names. */
  className?: string;
}

/**
 * ShareButton component that utilizes the Web Share API when supported,
 * falling back to clipboard copying via CopyButton.
 *
 * @param props Component properties.
 * @returns React element for sharing or copying the URL.
 */
export function ShareButton({ url, title, text, className }: ShareButtonProps) {
  const { t } = useTranslation();
  const shareUrl = url ?? (typeof window !== 'undefined' ? window.location.href : '');

  const canShare = (() => {
    if (typeof navigator === 'undefined' || typeof navigator.share !== 'function') {
      return false;
    }
    if (typeof navigator.canShare === 'function') {
      try {
        return navigator.canShare({ url: shareUrl });
      } catch {
        return false;
      }
    }
    return true;
  })();

  const [sharing, setSharing] = useState(false);
  const [shareError, setShareError] = useState(false);

  async function handleShare() {
    setShareError(false);
    setSharing(true);
    try {
      await navigator.share({ url: shareUrl, title, text });
    } catch (err) {
      if (err instanceof Error && err.name !== 'AbortError') {
        setShareError(true);
        setTimeout(() => setShareError(false), 2000);
      }
    } finally {
      setSharing(false);
    }
  }

  if (canShare) {
    const buttonClass = className ? `share-button ${className}` : 'share-button';
    return (
      <button
        type="button"
        className={buttonClass}
        onClick={handleShare}
        disabled={sharing}
        aria-label={t('share')}
      >
        {shareError ? t('copy_failed') : t('share')}
      </button>
    );
  }

  return <CopyButton value={shareUrl} />;
}
