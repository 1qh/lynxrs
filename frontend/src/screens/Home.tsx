import { useCallback, useEffect, useState } from '@lynx-js/react'
import { api } from '../api/client.js'
import { useAuth } from '../state/auth.js'
import { useEvents } from '../lib/useEvents.js'
import { AdminPanel } from './panels/AdminPanel.js'
import { FilesPanel } from './panels/FilesPanel.js'
import { ProfilePanel } from './panels/ProfilePanel.js'
import { OrgsPanel } from './panels/OrgsPanel.js'
import { TrashPanel } from './panels/TrashPanel.js'
import { WebhooksPanel } from './panels/WebhooksPanel.js'
import { AuditPanel } from './panels/AuditPanel.js'
import { MfaPanel } from './panels/MfaPanel.js'

type Tab = 'files' | 'profile' | 'orgs' | 'trash' | 'webhooks' | 'audit' | 'mfa' | 'admin'

const TABS: ReadonlyArray<{ id: Tab; label: string; adminOnly?: boolean }> = [
  { id: 'files', label: 'Files' },
  { id: 'profile', label: 'Profile' },
  { id: 'orgs', label: 'Orgs' },
  { id: 'trash', label: 'Trash' },
  { id: 'webhooks', label: 'Webhooks' },
  { id: 'audit', label: 'Audit' },
  { id: 'mfa', label: 'MFA' },
  { id: 'admin', label: 'Admin', adminOnly: true },
]

export function Home() {
  const user = useAuth((s) => s.user)!
  const setUser = useAuth((s) => s.setUser)
  const [refreshKey, setRefreshKey] = useState(0)
  const [tab, setTab] = useState<Tab>(() => {
    try {
      const loc = (globalThis as { location?: Location }).location
      const hash = loc?.hash?.replace(/^#/, '')
      if (hash && TABS.some((t) => t.id === hash)) return hash as Tab
      const v = (globalThis as { localStorage?: Storage }).localStorage?.getItem('simu.tab')
      if (v && TABS.some((t) => t.id === v)) return v as Tab
    } catch {}
    return 'files'
  })

  useEffect(() => {
    try {
      ;(globalThis as { localStorage?: Storage }).localStorage?.setItem('simu.tab', tab)
      const loc = (globalThis as { location?: Location; history?: History }).location
      const hist = (globalThis as { history?: History }).history
      if (loc && hist && loc.hash !== `#${tab}`) {
        hist.replaceState(null, '', `#${tab}`)
      }
    } catch {}
  }, [tab])

  useEffect(() => {
    const w = globalThis as {
      addEventListener?: (e: string, fn: () => void) => void
      removeEventListener?: (e: string, fn: () => void) => void
      location?: Location
    }
    if (!w.addEventListener || !w.location) return
    const onHash = () => {
      const h = w.location?.hash?.replace(/^#/, '')
      if (h && TABS.some((t) => t.id === h)) setTab(h as Tab)
    }
    w.addEventListener('hashchange', onHash)
    return () => w.removeEventListener?.('hashchange', onHash)
  }, [])

  useEvents(['file_created', 'file_deleted'], () => {
    setRefreshKey((k) => k + 1)
  })

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

  const bumpFiles = useCallback(() => setRefreshKey((k) => k + 1), [])

  const visibleTabs = TABS.filter((t) => !t.adminOnly || user.role === 'admin')

  return (
    <view className="Card">
      <text className="H2">Hello {user.email}</text>
      {!user.email_verified ? (
        <view className="Banner" bindtap={resend}>
          <text className="BannerText">Email not verified · tap to resend</text>
        </view>
      ) : null}
      <view className="TabBar">
        {visibleTabs.map((t) => (
          <view
            key={t.id}
            className={tab === t.id ? 'Tab TabActive' : 'Tab'}
            bindtap={() => setTab(t.id)}
          >
            <text className={tab === t.id ? 'TabText TabTextActive' : 'TabText'}>{t.label}</text>
          </view>
        ))}
      </view>
      <view className="TabPanel">
        {tab === 'files' ? <FilesPanel refreshKey={refreshKey} /> : null}
        {tab === 'profile' ? <ProfilePanel /> : null}
        {tab === 'orgs' ? <OrgsPanel /> : null}
        {tab === 'trash' ? <TrashPanel onRestore={bumpFiles} /> : null}
        {tab === 'webhooks' ? <WebhooksPanel /> : null}
        {tab === 'audit' ? <AuditPanel /> : null}
        {tab === 'mfa' ? <MfaPanel /> : null}
        {tab === 'admin' && user.role === 'admin' ? <AdminPanel /> : null}
      </view>
      <view className="Button ButtonGhost" bindtap={logout}>
        <text className="ButtonText">Log out</text>
      </view>
      <view className="Button ButtonGhost" bindtap={logoutAll}>
        <text className="ButtonText">Log out all devices</text>
      </view>
    </view>
  )
}
