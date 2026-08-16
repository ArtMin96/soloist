import { useCallback, useEffect, useState } from "react";
import { trustGrants, trustRevoke } from "@/api";
import { Button } from "@/components/ui/button";
import { plainReason } from "@/lib/plainText";
import type { TrustGrant } from "@/domain";

// What this project is trusted to run, and the way back out of it.
//
// The list exists because an agent can now cause a grant: if something else can ask for arbitrary
// code execution and the user can say yes, the user must be able to see what they said yes to and
// take it back. So every row leads with the command line — the digest is the key, but a key is not
// something a person can review — and says plainly how the grant came to be: a grant with no
// requester is one the user authored from their own solo.yml, and one that names a process is
// quoted with the words that process used to ask.
export function TrustedCommandsSection({ project }: { project: number }) {
  const [grants, setGrants] = useState<TrustGrant[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(
    () =>
      trustGrants(project)
        .then(setGrants)
        .catch((e) => setError(String(e))),
    [project],
  );

  useEffect(() => {
    void reload();
  }, [reload]);

  const revoke = useCallback(
    (grant: TrustGrant) => {
      setError(null);
      trustRevoke(project, grant.variant_hash)
        .then(reload)
        .catch((e) => setError(String(e)));
    },
    [project, reload],
  );

  return (
    <section className="mb-6">
      <h3 className="px-1 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
        Trusted commands
      </h3>
      <p className="mt-0.5 mb-1 max-w-[52ch] px-1 text-xs text-muted-foreground">
        The exact command lines this project may run. A revoked command is refused again the next
        time anything tries to start it, including an automatic restart.
      </p>
      {error && <p className="mt-1 px-1 text-xs text-destructive">{error}</p>}
      <div className="mt-1.5 divide-y divide-border overflow-hidden rounded-lg border border-border bg-card px-3">
        {grants === null ? (
          <p className="py-3 text-xs text-muted-foreground">Reading trusted commands…</p>
        ) : grants.length === 0 ? (
          <p className="py-3 text-xs text-muted-foreground">
            Nothing is trusted in this project yet.
          </p>
        ) : (
          grants.map((grant) => (
            <GrantRow key={grant.variant_hash} grant={grant} onRevoke={() => revoke(grant)} />
          ))
        )}
      </div>
    </section>
  );
}

// One grant: what it lets run, where it came from, and the control that takes it back.
function GrantRow({ grant, onRevoke }: { grant: TrustGrant; onRevoke: () => void }) {
  const label = grant.command ?? grant.variant_hash;
  return (
    <div className="flex items-start justify-between gap-6 py-3">
      <div className="flex min-w-0 flex-col gap-1">
        <code className="block break-all whitespace-pre-wrap font-mono text-xs text-foreground">
          {label}
        </code>
        {grant.requested_by === null ? (
          <p className="text-xs text-muted-foreground">You approved this from this project.</p>
        ) : (
          <p className="max-w-[52ch] text-xs text-muted-foreground">
            Approved at the asking of process {grant.requested_by}
            {grant.reason ? <>, which said: “{plainReason(grant.reason)}”</> : "."}
          </p>
        )}
      </div>
      <Button
        variant="outline"
        size="xs"
        className="shrink-0"
        aria-label={`Revoke ${label}`}
        onClick={onRevoke}
      >
        Revoke
      </Button>
    </div>
  );
}
