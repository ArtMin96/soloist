import { useState } from "react";
import { ArchiveIcon, CheckIcon, GitBranchIcon, PlusIcon, Trash2Icon } from "lucide-react";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { BranchActions } from "@/store/git/branchCluster";
import type { Branches } from "@/domain";

const SEARCH_PLACEHOLDER = "Switch or create a branch";
const NOTHING_MATCHES = "No branch by that name";
const BRANCHES_HEADING = "Branches";
const WORKING_TREE_HEADING = "Working tree";
const CREATE_LABEL = "Create";
const DELETE_LABEL = "Delete branch";
const STASH_LABEL = "Stash changes";
const STASH_HINT = "Set the working tree's changes aside; untracked files stay where they are";
const POP_LABEL = "Restore stashed changes";
const CHECKED_OUT = "Checked out";

/**
 * The branches to move between, and the two things that can be done with what the working tree
 * holds. One list, searched by typing, with the same text creating a branch when nothing matches it
 * — so the two intents share one field rather than hiding "new branch" behind a second control.
 *
 * Presentational: it holds the text being typed and nothing else. Whether a name is usable, whether
 * a switch is possible, and whether a branch may be deleted are all the core's and version
 * control's answers, reported where the action was asked for.
 */
export function BranchMenu({
  branches,
  actions,
  busy,
  onDelete,
}: {
  /** The branches to offer, or null until the read lands. */
  branches: Branches | null;
  actions: BranchActions;
  busy: boolean;
  /** A branch was asked to be removed; the surface confirms before anything happens. */
  onDelete: (name: string) => void;
}) {
  const [typed, setTyped] = useState("");
  const entries = branches?.entries ?? [];
  const wanted = typed.trim();
  const unclaimed = wanted !== "" && !entries.some((branch) => branch.name === wanted);

  return (
    <Command shouldFilter className="w-72">
      <CommandInput placeholder={SEARCH_PLACEHOLDER} value={typed} onValueChange={setTyped} />
      <CommandList>
        {/* A name nothing matches is an intent, not a dead end: the same text makes the branch. */}
        {unclaimed ? (
          <CommandGroup>
            <CommandItem
              value={`create ${wanted}`}
              disabled={busy}
              onSelect={() => {
                void actions.create(wanted).then((created) => created && setTyped(""));
              }}
            >
              <PlusIcon aria-hidden />
              <span className="min-w-0 truncate">
                {CREATE_LABEL} <span className="font-mono">{wanted}</span>
              </span>
            </CommandItem>
          </CommandGroup>
        ) : (
          <CommandEmpty>{NOTHING_MATCHES}</CommandEmpty>
        )}
        {entries.length > 0 && (
          <CommandGroup heading={BRANCHES_HEADING}>
            {entries.map((branch) => (
              <CommandItem
                key={branch.name}
                value={branch.name}
                disabled={busy}
                onSelect={() => !branch.head && actions.switchTo(branch.name)}
                className="group/branch"
              >
                {branch.head ? (
                  <CheckIcon aria-hidden className="text-git-branch-synced" />
                ) : (
                  <GitBranchIcon aria-hidden className="text-muted-foreground" />
                )}
                <span className="min-w-0 flex-1 truncate font-mono">{branch.name}</span>
                {branch.head ? (
                  <span className="type-label shrink-0 text-muted-foreground">{CHECKED_OUT}</span>
                ) : (
                  // The branch that is checked out cannot be removed, so it is not offered — and the
                  // rest reveal it on hover rather than lining the list with a destructive control.
                  //
                  // Taken out of the row's flow and pinned to its trailing edge, so an unhovered row
                  // spends none of its width on a control nobody is looking at: reserving the 24px
                  // was what truncated every branch name. It stays in the document — a control that
                  // is not rendered cannot be tabbed to — and carries the row's own fill so the
                  // revealed button never sits on top of the name behind it.
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`${DELETE_LABEL} ${branch.name}`}
                    className="absolute inset-y-0 end-0.5 my-auto translate-x-1 bg-accent opacity-0 transition-[opacity,translate] duration-[var(--dur-fast)] ease-out-quint motion-reduce:transition-none pointer-events-none group-hover/branch:pointer-events-auto group-hover/branch:translate-x-0 group-hover/branch:opacity-100 group-focus-within/branch:pointer-events-auto group-focus-within/branch:translate-x-0 group-focus-within/branch:opacity-100 focus-visible:pointer-events-auto focus-visible:translate-x-0 focus-visible:opacity-100"
                    onClick={(event) => {
                      event.stopPropagation();
                      onDelete(branch.name);
                    }}
                  >
                    <Trash2Icon />
                  </Button>
                )}
              </CommandItem>
            ))}
          </CommandGroup>
        )}
        {(actions.stash !== null || branches?.stashed === true) && (
          <>
            <CommandSeparator />
            <CommandGroup heading={WORKING_TREE_HEADING}>
              {actions.stash !== null && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <CommandItem value="stash" disabled={busy} onSelect={actions.stash}>
                      <ArchiveIcon aria-hidden className="text-muted-foreground" />
                      <span>{STASH_LABEL}</span>
                    </CommandItem>
                  </TooltipTrigger>
                  <TooltipContent>{STASH_HINT}</TooltipContent>
                </Tooltip>
              )}
              {branches?.stashed === true && (
                <CommandItem value="restore stash" disabled={busy} onSelect={actions.popStash}>
                  <ArchiveIcon aria-hidden className="text-muted-foreground" />
                  <span>{POP_LABEL}</span>
                </CommandItem>
              )}
            </CommandGroup>
          </>
        )}
      </CommandList>
    </Command>
  );
}
