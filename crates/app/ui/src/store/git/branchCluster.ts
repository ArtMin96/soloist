import { useSyncExternalStore } from "react";
import type { BranchInfo, Branches } from "@/domain";

/** What a branch row can be asked to do. */
export interface BranchActions {
  switchTo: (name: string) => void;
  create: (name: string) => Promise<boolean>;
  remove: (name: string) => void;
  stash: () => void;
  popStash: () => void;
}

/** What the remote can be asked for, and the one control that ends a wait on it. */
export interface ExchangeActions {
  fetch: () => void;
  pull: () => void;
  push: () => void;
  stop: () => void;
}

/** What is checked out, how it stands against its upstream, and everything that can be done about
 *  either. */
export interface BranchClusterView {
  branch: BranchInfo;
  /** The branches to offer once the switcher is open, or null until that read lands. */
  branches: Branches | null;
  exchanging: boolean;
  /** Whether a branch action is still running. */
  busy: boolean;
  /** Reaching the remote, or null while nothing may change the repository. */
  exchange: ExchangeActions | null;
  /** Moving between branches and setting the working tree aside, or null while nothing may change
   *  the repository. */
  branchActions: BranchActions | null;
  /** Show the pull-request view, or null while nothing may change the repository. */
  openPullRequest: (() => void) | null;
  /** The switcher opened or closed; the branch list is read only while it is open. */
  onBranchesOpen: (open: boolean) => void;
}

let published: BranchClusterView | null = null;
const listeners = new Set<() => void>();

/**
 * Hands the window chrome the repository projection, or clears it.
 *
 * Called by the surface that already reads the repository, so the branch in the title bar and the
 * rail beside the terminal are one read rather than two. The two sit in different corners of the
 * tree with no common ancestor below the app shell, so a context provider would have to wrap the
 * whole shell — dragging every repository hook into the eager bundle. A module store keeps the read
 * where it was and re-renders only the cluster when what it shows changes.
 */
export function publishBranchCluster(next: BranchClusterView | null): void {
  if (next === published) return;
  published = next;
  for (const listener of listeners) listener();
}

const switcherRequests = new Set<() => void>();

/**
 * Asks whatever is showing the branch switcher to open it.
 *
 * How a surface with no room for the control — the command palette — reaches the same one the badge
 * in the chrome opens, rather than carrying a second way to switch branches. Nothing happens when no
 * repository is in sight, which is exactly when there is no switcher to open.
 */
export function requestBranchSwitcher(): void {
  for (const open of [...switcherRequests]) open();
}

/** Answers those requests for as long as the switcher is on screen. */
export function onBranchSwitcherRequest(open: () => void): () => void {
  switcherRequests.add(open);
  return () => {
    switcherRequests.delete(open);
  };
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function snapshot(): BranchClusterView | null {
  return published;
}

/** The published projection, or null while no repository is in sight. */
export function useBranchCluster(): BranchClusterView | null {
  return useSyncExternalStore(subscribe, snapshot, snapshot);
}
