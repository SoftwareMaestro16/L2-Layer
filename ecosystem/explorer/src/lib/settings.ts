"use client";

import { create } from "zustand";

const defaultApiBase =
  process.env.NEXT_PUBLIC_ENTROPIS_API_BASE ?? "http://127.0.0.1:8080";

type ExplorerSettings = {
  apiBase: string;
  setApiBase: (apiBase: string) => void;
};

export const useExplorerSettings = create<ExplorerSettings>((set) => ({
  apiBase: defaultApiBase,
  setApiBase: (apiBase) =>
    set({ apiBase: apiBase.trim() || defaultApiBase }),
}));
