/* eslint-disable no-console */
import React, { Component, ErrorInfo, ReactNode } from 'react';

interface Props {
  children: ReactNode;
  appName: string;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  public state: State = {
    hasError: false,
    error: null,
  };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Uncaught error:', error, errorInfo);

    try {
      fetch('/api/logs/client', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          message: error.message,
          stack: error.stack,
          componentStack: errorInfo.componentStack,
          url: window.location.href,
          userAgent: navigator.userAgent,
          appName: this.props.appName,
        }),
      }).catch(err => {
        console.error('Failed to send error log to server', err);
      });
    } catch (e) {
      console.error('Failed to serialize error log', e);
    }
  }

  public render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-screen flex items-center justify-center p-4" style={{ background: 'var(--color-bg)' }}>
          <div className="max-w-md w-full text-center space-y-6 card" style={{ padding: 32 }}>
            <div className="w-16 h-16 mx-auto rounded-full flex items-center justify-center" style={{ background: 'var(--color-danger-bg)', color: 'var(--color-danger)' }}>
              <svg xmlns="http://www.w3.org/2000/svg" className="h-8 w-8" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </div>
            <div className="space-y-2">
              <h2 className="text-xl font-bold" style={{ color: 'var(--color-text)' }}>Something went wrong</h2>
              <p style={{ color: 'var(--color-muted)' }}>
                An unexpected error occurred. The error has been reported.
              </p>
            </div>
            <button onClick={() => window.location.reload()} className="btn btn-primary btn-block">
              Reload Page
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
