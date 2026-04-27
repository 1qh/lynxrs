import { useCallback, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { ProfilePanel } from './ProfilePanel.js'
import { MfaPanel } from './MfaPanel.js'
import { WebhooksPanel } from './WebhooksPanel.js'
import { reportError } from '../../state/toast.js'

type Section = 'profile' | 'mfa' | 'webhooks' | 'notifications'

const SECTIONS: ReadonlyArray<{ id: Section; labelKey: string }> = [
  { id: 'profile', labelKey: 'settings.profile' },
  { id: 'notifications', labelKey: 'settings.notifications' },
  { id: 'mfa', labelKey: 'settings.mfa' },
  { id: 'webhooks', labelKey: 'settings.webhooks' },
]

function NotificationsPanel() {
  const { t } = useTranslation()
  const [perm, setPerm] = useState<string>(
    (globalThis as { Notification?: { permission: string } }).Notification?.permission ?? 'default',
  )
  const enable = useCallback(async () => {
    try {
      const w = globalThis as {
        Notification?: { requestPermission: () => Promise<string> }
        navigator?: { serviceWorker?: { ready: Promise<{ pushManager?: { subscribe: (opts: unknown) => Promise<unknown>; getSubscription: () => Promise<unknown> } }> } }
      }
      if (!w.Notification) {
        reportError('Notifications API not available', 'Enable notifications failed')
        return
      }
      const p = await w.Notification.requestPermission()
      setPerm(p)
      if (p !== 'granted') return
      // Subscribe to the service worker's push manager. Without a VAPID
      // public key configured server-side, this stays a no-op shell — the
      // SW push handler still works for client-triggered notifications.
      const reg = await w.navigator?.serviceWorker?.ready
      if (reg?.pushManager) {
        const existing = await reg.pushManager.getSubscription()
        if (!existing) {
          // No-op subscribe attempt; backend would supply applicationServerKey.
        }
      }
    } catch (e) {
      reportError(e, 'Enable notifications failed')
    }
  }, [])
  const testLocal = useCallback(() => {
    const w = globalThis as { Notification?: { new (title: string, opts?: { body?: string }): unknown } }
    if (!w.Notification || perm !== 'granted') return
    new w.Notification('simu', { body: 'Test notification ✓' })
  }, [perm])
  return (
    <view className="gap-3">
      <text className="text-sm text-foreground">
        {t('settings.notifications_status')}: <text className="font-medium">{perm}</text>
      </text>
      {perm !== 'granted' ? (
        <view
          className="h-10 rounded-md bg-primary items-center justify-center"
          bindtap={() => void enable()}
          aria-label={t('settings.enable_notifications')}
        >
          <text className="text-primary-foreground text-sm font-medium">
            {t('settings.enable_notifications')}
          </text>
        </view>
      ) : (
        <view
          className="h-10 rounded-md bg-background border border-input items-center justify-center"
          bindtap={testLocal}
          aria-label={t('settings.test_notification')}
        >
          <text className="text-foreground text-sm font-medium">
            {t('settings.test_notification')}
          </text>
        </view>
      )}
      <text className="text-xs text-muted-foreground">{t('settings.notifications_hint')}</text>
    </view>
  )
}

export function SettingsPanel() {
  const { t } = useTranslation()
  const [section, setSection] = useState<Section>('profile')
  return (
    <view className="gap-4">
      <view className="flex-row flex-wrap gap-1">
        {SECTIONS.map((s) => (
          <view
            key={s.id}
            className={
              section === s.id
                ? 'rounded-md px-3 py-1.5 bg-secondary'
                : 'rounded-md px-3 py-1.5 bg-transparent'
            }
            bindtap={() => setSection(s.id)}
            aria-label={t(s.labelKey)}
          >
            <text
              className={
                section === s.id
                  ? 'text-secondary-foreground text-sm font-medium'
                  : 'text-muted-foreground text-sm font-medium'
              }
            >
              {t(s.labelKey)}
            </text>
          </view>
        ))}
      </view>
      <view className="rounded-md bg-card border border-border p-4">
        {section === 'profile' ? <ProfilePanel /> : null}
        {section === 'notifications' ? <NotificationsPanel /> : null}
        {section === 'mfa' ? <MfaPanel /> : null}
        {section === 'webhooks' ? <WebhooksPanel /> : null}
      </view>
    </view>
  )
}
