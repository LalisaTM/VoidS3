import { create } from "zustand";
import { persist } from "zustand/middleware";
interface Bookmark {
  label: string;
  profile: string;
  prefix: string;
  local: string;
}
interface Preferences {
  theme: "dark" | "light";
  view: "dual" | "remote" | "local";
  hidden: boolean;
  queueOpen: boolean;
  checkUpdatesAutomatically: boolean;
  bookmarks: Bookmark[];
  set: (patch: Partial<Omit<Preferences, "set">>) => void;
}
export const usePreferences = create<Preferences>()(
  persist(
    (set) => ({
      theme: "dark",
      view: "dual",
      hidden: false,
      queueOpen: true,
      checkUpdatesAutomatically: true,
      bookmarks: [],
      set,
    }),
    { name: "voids3-ui-v1", version: 1 },
  ),
);
