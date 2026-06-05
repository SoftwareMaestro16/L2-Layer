import { create } from "zustand";

const DEFAULT_API_BASE =
  process.env.NEXT_PUBLIC_ENTROPIS_API_BASE ?? "http://127.0.0.1:8080";

type ApiStore = {
  apiBase: string;
  setApiBase: (value: string) => void;
};

export const useApiStore = create<ApiStore>((set) => ({
  apiBase: DEFAULT_API_BASE.replace(/\/+$/u, ""),
  setApiBase: (value) => set({ apiBase: value.trim().replace(/\/+$/u, "") })
}));
