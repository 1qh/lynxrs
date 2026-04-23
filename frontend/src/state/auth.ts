import { create } from 'zustand'
import type { components } from '../api/schema.js'

export type User = components['schemas']['UserDto']

interface AuthState {
  user: User | null
  loading: boolean
  setUser: (u: User | null) => void
  setLoading: (b: boolean) => void
}

export const useAuth = create<AuthState>((set) => ({
  user: null,
  loading: false,
  setUser: (u) => set({ user: u }),
  setLoading: (b) => set({ loading: b }),
}))
