import { useState } from "react";

export function CopyButton({ value }: { value: string }) {
  const [copied, setCopied] = useState(false);
  const [failed, setFailed] = useState(false);

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      setFailed(true);
      setTimeout(() => setFailed(false), 1500);
    }
  }

  return (
    <button
      type="button"
      className="copy-button"
      title={value}
      onClick={handleCopy}
      aria-label="Copy to clipboard"
    >
      {copied ? "Copied!" : failed ? "Copy failed" : "⧉"}
    </button>
  );
}
