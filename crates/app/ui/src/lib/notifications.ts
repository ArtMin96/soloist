import type { Notifications } from "@/domain";

// The master switch defaults on (mirrors soloist_core::Notifications::default); the facade's stored
// value supersedes this the moment it loads.
export const DEFAULT_NOTIFICATIONS: Notifications = { enabled: true };
