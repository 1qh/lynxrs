import { useCallback, useEffect, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
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

const TABS: ReadonlyArray<{ id: Tab; labelKey: string; adminOnly?: boolean }> = [
  { id: 'files', labelKey: 'tabs.files' },
  { id: 'profile', labelKey: 'tabs.profile' },
  { id: 'orgs', labelKey: 'tabs.orgs' },
  { id: 'trash', labelKey: 'tabs.trash' },
  { id: 'webhooks', labelKey: 'tabs.webhooks' },
  { id: 'audit', labelKey: 'tabs.audit' },
  { id: 'mfa', labelKey: 'tabs.mfa' },
  { id: 'admin', labelKey: 'tabs.admin', adminOnly: true },
]

export function Home() {
  const { t } = useTranslation()
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

  const visibleTabs = TABS.filter((tb) => !tb.adminOnly || user.role === 'admin')

  return (
    <view className="Card">
      <text className="H2">{t('home.hello', { email: user.email })}</text>
      {!user.email_verified ? (
        <view className="Banner" bindtap={resend}>
          <text className="BannerText">{t('home.email_not_verified')}</text>
        </view>
      ) : null}
      <view className="TabBar">
        {visibleTabs.map((tb) => (
          <view
            key={tb.id}
            className={tab === tb.id ? 'Tab TabActive' : 'Tab'}
            bindtap={() => setTab(tb.id)}
          >
            <text className={tab === tb.id ? 'TabText TabTextActive' : 'TabText'}>{t(tb.labelKey)}</text>
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
        <text className="ButtonText">{t('home.log_out')}</text>
      </view>
      <view className="Button ButtonGhost" bindtap={logoutAll}>
        <text className="ButtonText">{t('home.log_out_all')}</text>
      </view>
    </view>
  )
}
