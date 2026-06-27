// The one place the webview's persisted read-model cache touches tauri-plugin-store. Every
// other module reaches the cache through `readSnapshot`/`writeSnapshot` here, so the plugin
// import and the on-disk shape live in a single file. The cache is a last-known projection
// for instant cold-start paint; the core stays authoritative and overwrites it on reconcile.
import { LazyStore } from "@tauri-apps/plugin-store";

// All cached snapshots share one file in the OS app-data dir (separate from the core's SQLite
// — this is display cache, not durable state).
const CACHE_FILE = "ui-cache.json";

// Coalesce a burst of writes (a launch can revalidate several snapshots at once) into one
// disk flush: the store auto-saves this many milliseconds after the last `set`.
const AUTOSAVE_DEBOUNCE_MS = 200;

// Bump when any cached value's shape changes, so a snapshot written by an older build is read
// back as a miss instead of being rendered against the new shape.
const SCHEMA_VERSION = 1;

// The cache keys — one named const per cached snapshot; never a bare string at a call site.
export const CacheKey = {
  projects: "projects",
  appInfo: "app-info",
  agents: "agents",
} as const;
export type CacheKey = (typeof CacheKey)[keyof typeof CacheKey];

// A stored value tagged with the schema it was written under, so a shape change across builds
// invalidates old blobs (version mismatch ⇒ miss) rather than mis-rendering them.
interface Envelope<T> {
  version: number;
  value: T;
}

// Lazy: the store file is loaded on first read/write, not at module import, so nothing touches
// disk until a snapshot is actually used.
const store = new LazyStore(CACHE_FILE, { defaults: {}, autoSave: AUTOSAVE_DEBOUNCE_MS });

// The last-known value for `key`, or null when absent or written under a different schema
// version. Never throws — a read failure (no Tauri host under test, unreadable file) is a
// miss, so the caller simply falls back to the live fetch.
export async function readSnapshot<T>(key: CacheKey): Promise<T | null> {
  try {
    const envelope = await store.get<Envelope<T>>(key);
    if (!envelope || envelope.version !== SCHEMA_VERSION) return null;
    return envelope.value;
  } catch {
    return null;
  }
}

// Persists `value` as the last-known snapshot for `key` (write-through, debounce-saved). Never
// throws — a failed write just leaves the cache cold, never breaking the live path.
export async function writeSnapshot<T>(key: CacheKey, value: T): Promise<void> {
  try {
    await store.set(key, { version: SCHEMA_VERSION, value } satisfies Envelope<T>);
  } catch {
    // Storage unavailable; the live read stays the source of truth for this session.
  }
}
