// Apply an optimistic settings write: on success adopt the facade's echoed value; on failure fall
// back to disk truth by re-reading, so the UI never silently diverges from what was actually
// persisted. The one place the load-once providers and hooks share this reconcile (mirrors the
// behaviour AppearanceProvider already had inline).
export function persistThenReconcile<T>(
  write: Promise<T>,
  read: () => Promise<T>,
  apply: (value: T) => void,
): void {
  void write.then(apply).catch(() => {
    void read()
      .then(apply)
      .catch(() => {});
  });
}
