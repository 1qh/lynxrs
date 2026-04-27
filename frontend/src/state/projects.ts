import { create } from 'zustand'

/**
 * Lightweight client-side projects store. Groups conversations under a named
 * "project" so a user can keep separate threads (work / personal / research)
 * without cluttering the global list. Backed by localStorage so it survives
 * reloads; not yet on the server (multi-device sync of project metadata is a
 * follow-up).
 */
export type Project = { id: string; name: string }

interface ProjectsState {
  projects: Project[]
  /** conversation_id → project_id assignment */
  byConv: Record<string, string>
  /** Currently-active project ('all' = no filter). */
  active: string
  add: (name: string) => Project
  rename: (id: string, name: string) => void
  remove: (id: string) => void
  assign: (convId: string, projectId: string | null) => void
  setActive: (id: string) => void
}

const PK = 'simu.projects'
const CK = 'simu.convProj'
const AK = 'simu.activeProject'

function readArr<T>(key: string, fallback: T): T {
  try {
    const raw = (globalThis as { localStorage?: Storage }).localStorage?.getItem(key)
    return raw ? (JSON.parse(raw) as T) : fallback
  } catch {
    return fallback
  }
}
function write(key: string, v: unknown) {
  try {
    ;(globalThis as { localStorage?: Storage }).localStorage?.setItem(key, JSON.stringify(v))
  } catch {}
}

export const useProjects = create<ProjectsState>((set, get) => ({
  projects: readArr<Project[]>(PK, []),
  byConv: readArr<Record<string, string>>(CK, {}),
  active: (() => {
    try {
      return (globalThis as { localStorage?: Storage }).localStorage?.getItem(AK) ?? 'all'
    } catch {
      return 'all'
    }
  })(),
  add: (name) => {
    const p: Project = { id: `p-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`, name }
    const next = [...get().projects, p]
    write(PK, next)
    set({ projects: next })
    return p
  },
  rename: (id, name) => {
    const next = get().projects.map((p) => (p.id === id ? { ...p, name } : p))
    write(PK, next)
    set({ projects: next })
  },
  remove: (id) => {
    const next = get().projects.filter((p) => p.id !== id)
    const byConv = { ...get().byConv }
    for (const k of Object.keys(byConv)) if (byConv[k] === id) delete byConv[k]
    write(PK, next)
    write(CK, byConv)
    set({ projects: next, byConv, active: get().active === id ? 'all' : get().active })
  },
  assign: (convId, projectId) => {
    const byConv = { ...get().byConv }
    if (projectId) byConv[convId] = projectId
    else delete byConv[convId]
    write(CK, byConv)
    set({ byConv })
  },
  setActive: (id) => {
    try {
      ;(globalThis as { localStorage?: Storage }).localStorage?.setItem(AK, id)
    } catch {}
    set({ active: id })
  },
}))
