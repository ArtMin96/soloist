import { Component, type ErrorInfo, type ReactNode } from "react";
import { RecoveryNotice } from "@/components/common/RecoveryNotice";

interface PaneErrorBoundaryProps {
  children: ReactNode;
  /** Names the failed pane in the recovery message ("Diff view ran into a problem"). */
  label?: string;
}

interface PaneErrorBoundaryState {
  hasError: boolean;
}

/**
 * Catches a render error thrown anywhere in its subtree — including a failed lazy-chunk import,
 * when it wraps that `Suspense` boundary — and shows a contained recovery notice in its place
 * instead of taking down the rest of the app. "Try again" clears the caught error so the subtree
 * mounts fresh; a pane that keeps failing just shows the notice again.
 */
export class PaneErrorBoundary extends Component<PaneErrorBoundaryProps, PaneErrorBoundaryState> {
  state: PaneErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): PaneErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[PaneErrorBoundary]", this.props.label ?? "pane", error, info.componentStack);
  }

  private retry = () => this.setState({ hasError: false });

  render() {
    if (!this.state.hasError) return this.props.children;
    const name = this.props.label ?? "This pane";
    return <RecoveryNotice message={`${name} ran into a problem.`} onRetry={this.retry} />;
  }
}
