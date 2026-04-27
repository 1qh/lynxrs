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

type TabSpec = { path: string; labelKey: string; adminOnly?: boolean }

const TABS: ReadonlyArray<TabSpec> = [
  { path: '/files', labelKey: 'tabs.files' },
  { path: '/orgs', labelKey: 'tabs.orgs' },
  { path: '/trash', labelKey: 'tabs.trash' },
  { path: '/audit', labelKey: 'tabs.audit' },
  { path: '/settings', labelKey: 'tabs.settings' },
  { path: '/admin', labelKey: 'tabs.admin', adminOnly: true },
]

function Layout() {
  const { t } = useTranslation()
  const user = useAuth((s) => s.user)!
  const setUser = useAuth((s) => s.setUser)
  const loc = useLocation()
  const navigate = useNavigate()
  const bump = useFilesState((s) => s.bump)

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

  const logoutAll = useCallback(async () => {
    await api.POST('/auth/logout-all', {})
    setUser(null)
  }, [setUser])

  const resend = useCallback(async () => {
    await api.POST('/auth/email/resend', {})
  }, [])

  const visibleTabs = TABS.filter((tb) => !tb.adminOnly || user.role === 'admin')

  return (
    <view className="gap-4">
      <view className="flex-row items-center justify-between gap-2">
        <text className="flex-1 text-base font-medium text-foreground" aria-label={`signed in as ${user.email}`}>
          {t('home.hello', { email: user.email })}
        </text>
        <OrgContextSwitcher />
      </view>
      {!user.email_verified ? (
        <view
          className="rounded-md bg-secondary border border-border px-3 py-2"
          bindtap={resend}
          aria-label={t('home.email_not_verified')}
        >
          <text className="text-secondary-foreground text-[13px]">
            {t('home.email_not_verified')}
          </text>
        </view>
      ) : null}
      <view className="flex-row flex-wrap gap-1 py-2 border-b border-border">
        {visibleTabs.map((tb) => {
          const active = loc.pathname.startsWith(tb.path)
          return (
            <view
              key={tb.path}
              className={
                active
                  ? 'rounded-md px-3 py-1.5 bg-primary'
                  : 'rounded-md px-3 py-1.5 bg-transparent'
              }
              bindtap={() => navigate(tb.path)}
              aria-label={t(tb.labelKey)}
            >
              <text
                className={
                  active
                    ? 'text-primary-foreground text-sm font-medium'
                    : 'text-muted-foreground text-sm font-medium'
                }
              >
                {t(tb.labelKey)}
              </text>
            </view>
          )
        })}
      </view>
      <view className="py-2 gap-3">
        <Outlet />
      </view>
      <view className="flex-row gap-2 pt-4 border-t border-border">
        <view
          className="h-9 flex-1 rounded-md bg-background border border-input items-center justify-center"
          bindtap={logout}
          aria-label={t('home.log_out')}
        >
          <text className="text-foreground text-sm font-medium">{t('home.log_out')}</text>
        </view>
        <view
          className="h-9 flex-1 rounded-md bg-background border border-input items-center justify-center"
          bindtap={logoutAll}
          aria-label={t('home.log_out_all')}
        >
          <text className="text-foreground text-sm font-medium">{t('home.log_out_all')}</text>
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
