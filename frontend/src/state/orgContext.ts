import { create } from 'zustand'

interface OrgContextState {
  /** null = personal scope; otherwise filter files/uploads to this org. */
  activeOrgId: string | null
  setActive: (id: string | null) => void
}

const KEY = 'simu.activeOrg'

function readInitial(): string | null {
  try {
    const v = (globalThis as { localStorage?: Storage }).localStorage?.getItem(KEY)
    return v && v !== 'null' ? v : null
  } catch {
    return null
  }
}

export const useOrgContext = create<OrgContextState>((set) => ({
  activeOrgId: readInitial(),
  setActive: (id) => {
    try {
      ;(globalThis as { localStorage?: Storage }).localStorage?.setItem(KEY, id ?? 'null')
    } catch {}
    set({ activeOrgId: id })
  },
}))
