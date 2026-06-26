import { appInfo } from "@/api";
import { CacheKey } from "@/store/cache/persistentCache";
import { usePersistentSnapshot } from "@/store/cache/usePersistentSnapshot";
import type { AppInfo } from "@/domain";

// The app identity (name/version). Paints the last-known value instantly from the persisted
// cache on launch, then reconciles to the live core value.
export function useAppInfo(): AppInfo | null {
  const { value } = usePersistentSnapshot(CacheKey.appInfo, () => appInfo());
  return value;
}
