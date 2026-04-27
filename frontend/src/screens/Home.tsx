import { useCallback, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import {
  Routes,
  Route,
  Outlet,
  useLocation,
  useNavigate,
} from 'react-router-dom'
import { api } from '../api/client.js'
import { useAuth } from '../state/auth.js'
import { useFilesState } from '../state/files.js'
import { useEvents } from '../lib/useEvents.js'
import { useKeyboardShortcuts } from '../lib/useKeyboardShortcuts.js'
import { OrgContextSwitcher } from './OrgContextSwitcher.js'
import { AdminPanel } from './panels/AdminPanel.js'
import { FilesPanel } from './panels/FilesPanel.js'
import { SettingsPanel } from './panels/SettingsPanel.js'
import { OrgsPanel } from './panels/OrgsPanel.js'
import { TrashPanel } from './panels/TrashPanel.js'
import { AuditPanel } from './panels/AuditPanel.js'

type TabSpec = { path: string; labelKey: string; glyph: string; adminOnly?: boolean }

const TABS: ReadonlyArray<TabSpec> = [
  { path: '/files', labelKey: 'tabs.files', glyph: '📁' },
  { path: '/orgs', labelKey: 'tabs.orgs', glyph: '🏢' },
  { path: '/audit', labelKey: 'tabs.audit', glyph: '📊' },
  { path: '/settings', labelKey: 'tabs.settings', glyph: '⚙' },
]

const SECONDARY_TABS: ReadonlyArray<TabSpec> = [
  { path: '/trash', labelKey: 'tabs.trash', glyph: '🗑' },
  { path: '/admin', labelKey: 'tabs.admin', glyph: '🛡', adminOnly: true },
]

function Layout() {
  const { t } = useTranslation()
  const user = useAuth((s) => s.user)!
  const setUser = useAuth((s) => s.setUser)
  const loc = useLocation()
  const navigate = useNavigate()
  const bump = useFilesState((s) => s.bump)
  const [moreOpen, setMoreOpen] = useState(false)

  useEvents(['file_created', 'file_deleted'], bump)

  const [gPrefix, setG] = useState(false)
  useKeyboardShortcuts([
    { key: 'g', handler: () => { setG(true); setTimeout(() => setG(false), 1500) } },
    { key: 'f', handler: () => gPrefix && navigate('/files') },
    { key: 'o', handler: () => gPrefix && navigate('/orgs') },
    { key: 't', handler: () => gPrefix && navigate('/trash') },
    { key: 'a', handler: () => gPrefix && navigate('/audit') },
    { key: 's', handler: () => gPrefix && navigate('/settings') },
    { key: 'd', handler: () => gPrefix && user.role === 'admin' && navigate('/admin') },
  ])

  const logout = useCallback(async () => {
    await api.POST('/auth/logout', {})
    setUser(null)
  }, [setUser])

  const resend = useCallback(async () => {
    await api.POST('/auth/email/resend', {})
  }, [])

  const visibleSecondary = SECONDARY_TABS.filter(
    (tb) => !tb.adminOnly || user.role === 'admin',
  )

  return (
    <view className="h-full">
      {/* Sub-header: identity + org switcher */}
      <view className="flex-row items-center justify-between px-4 py-3 gap-2 border-b border-border">
        <view className="flex-1 gap-0.5">
          <text className="text-sm font-medium text-foreground" aria-label={`signed in as ${user.email}`}>
            {user.display_name ?? user.email}
          </text>
          {!user.email_verified ? (
            <text className="text-[11px] text-warn" bindtap={resend} aria-label={t('home.email_not_verified')}>
              ⚠ {t('home.email_not_verified')}
            </text>
          ) : null}
        </view>
        <OrgContextSwitcher />
      </view>

      {/* Scrollable content */}
      <view className="flex-1 overflow-auto px-4 py-3 gap-3">
        <Outlet />
        {/* Secondary actions tucked at bottom of body */}
        {moreOpen ? (
          <view className="rounded-md bg-card border border-border p-3 gap-2">
            {visibleSecondary.map((tb) => {
              const a = loc.pathname.startsWith(tb.path)
              return (
                <view
                  key={tb.path}
                  className={
                    a
                      ? 'flex-row items-center gap-3 rounded-md bg-primary px-3 py-2'
                      : 'flex-row items-center gap-3 rounded-md bg-transparent px-3 py-2'
                  }
                  bindtap={() => { navigate(tb.path); setMoreOpen(false) }}
                  aria-label={t(tb.labelKey)}
                >
                  <text className="text-base">{tb.glyph}</text>
                  <text
                    className={
                      a
                        ? 'text-primary-foreground text-sm font-medium'
                        : 'text-foreground text-sm font-medium'
                    }
                  >
                    {t(tb.labelKey)}
                  </text>
                </view>
              )
            })}
            <view
              className="flex-row items-center gap-3 rounded-md bg-transparent px-3 py-2"
              bindtap={() => void logout()}
              aria-label={t('home.log_out')}
            >
              <text className="text-base">🚪</text>
              <text className="text-foreground text-sm font-medium">{t('home.log_out')}</text>
            </view>
          </view>
        ) : null}
      </view>

      {/* Bottom tab bar — iOS/Android-style */}
      <view className="flex-row items-stretch border-t border-border bg-background">
        {TABS.map((tb) => {
          const active = loc.pathname.startsWith(tb.path)
          return (
            <view
              key={tb.path}
              className="flex-1 items-center justify-center py-2 gap-0.5"
              bindtap={() => { navigate(tb.path); setMoreOpen(false) }}
              aria-label={t(tb.labelKey)}
            >
              <text className={active ? 'text-lg' : 'text-lg opacity-60'}>{tb.glyph}</text>
              <text
                className={
                  active
                    ? 'text-[11px] font-medium text-primary'
                    : 'text-[11px] font-medium text-muted-foreground'
                }
              >
                {t(tb.labelKey)}
              </text>
            </view>
          )
        })}
        <view
          className="flex-1 items-center justify-center py-2 gap-0.5"
          bindtap={() => setMoreOpen((v) => !v)}
          aria-label="more"
        >
          <text className={moreOpen ? 'text-lg' : 'text-lg opacity-60'}>···</text>
          <text
            className={
              moreOpen
                ? 'text-[11px] font-medium text-primary'
                : 'text-[11px] font-medium text-muted-foreground'
            }
          >
            More
          </text>
        </view>
      </view>
    </view>
  )
}

function FilesView() {
  const refreshKey = useFilesState((s) => s.refreshKey)
  return <FilesPanel refreshKey={refreshKey} />
}

function TrashView() {
  const bump = useFilesState((s) => s.bump)
  return <TrashPanel onRestore={bump} />
}

export function Home() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<FilesView />} />
        <Route path="files" element={<FilesView />} />
        <Route path="files/:id" element={<FilesView />} />
        <Route path="orgs" element={<OrgsPanel />} />
        <Route path="orgs/:slug" element={<OrgsPanel />} />
        <Route path="trash" element={<TrashView />} />
        <Route path="audit" element={<AuditPanel />} />
        <Route path="settings/*" element={<SettingsPanel />} />
        <Route path="admin" element={<AdminPanel />} />
        <Route path="*" element={<FilesView />} />
      </Route>
    </Routes>
  )
}
