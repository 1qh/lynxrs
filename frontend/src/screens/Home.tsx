import { useCallback, useState } from '@lynx-js/react'
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

export function Home() {
  const user = useAuth((s) => s.user)!
  const setUser = useAuth((s) => s.setUser)
  const [refreshKey, setRefreshKey] = useState(0)

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

  return (
    <view className="Card">
      <text className="H2">Hello {user.email}</text>
      {!user.email_verified ? (
        <view className="Banner" bindtap={resend}>
          <text className="BannerText">Email not verified · tap to resend</text>
        </view>
      ) : null}
      {user.role === 'admin' ? <AdminPanel /> : null}
      <FilesPanel refreshKey={refreshKey} />
      <ProfilePanel />
      <OrgsPanel />
      <TrashPanel onRestore={bumpFiles} />
      <WebhooksPanel />
      <AuditPanel />
      <MfaPanel />
      <view className="Button ButtonGhost" bindtap={logout}>
        <text className="ButtonText">Log out</text>
      </view>
      <view className="Button ButtonGhost" bindtap={logoutAll}>
        <text className="ButtonText">Log out all devices</text>
      </view>
    </view>
  )
}
