import { create } from 'zustand'

interface FilesState {
  refreshKey: number
  bump: () => void
}

export const useFilesState = create<FilesState>((set) => ({
  refreshKey: 0,
  bump: () => set((s) => ({ refreshKey: s.refreshKey + 1 })),
}))
