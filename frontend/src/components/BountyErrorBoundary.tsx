import React, { Component, ErrorInfo, ReactNode } from 'react';
import { t } from '../i18n';

interface Props {
  children: ReactNode;
  /** Optional custom fallback title shown in the error UI. */
  heading?: string;
}

interface State {
  hasError: boolean;
  message: string | null;
}

/**
 * Route-level error boundary.
 *
 * - Catches render/lifecycle errors in any child tree.
 * - Shows a retry button that resets the boundary.
 * - Displays error details only in development (import.meta.env.DEV).
 *
 * Issues #915, #911.
 */
export class BountyErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, message: null };
  }

  static getDerivedStateFromError(error: unknown): State {
    const message =
      error instanceof Error ? error.message : String(error);
    return { hasError: true, message };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Preserve existing error-logging conventions (console only; no external sink yet).
    console.error('[BountyErrorBoundary]', error, info.componentStack);
  }

  handleRetry = () => {
    this.setState({ hasError: false, message: null });
  };

  render() {
    if (this.state.hasError) {
      const isDev =
        typeof import.meta !== 'undefined' &&
        (import.meta as { env?: { DEV?: boolean } }).env?.DEV === true;

      return (
        <div role="alert" className="error-boundary">
          <p>{this.props.heading ?? t('error_boundary_message')}</p>
          {isDev && this.state.message && (
            <details className="error-boundary__details">
              <summary>Error details (development only)</summary>
              <pre>{this.state.message}</pre>
            </details>
          )}
          <button onClick={this.handleRetry}>{t('retry')}</button>
        </div>
      );
    }
    return this.props.children;
  }
}
