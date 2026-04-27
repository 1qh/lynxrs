import { useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { ProfilePanel } from './ProfilePanel.js'
import { MfaPanel } from './MfaPanel.js'
import { WebhooksPanel } from './WebhooksPanel.js'

type Section = 'profile' | 'mfa' | 'webhooks'

const SECTIONS: ReadonlyArray<{ id: Section; labelKey: string }> = [
  { id: 'profile', labelKey: 'settings.profile' },
  { id: 'mfa', labelKey: 'settings.mfa' },
  { id: 'webhooks', labelKey: 'settings.webhooks' },
]

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
        {section === 'mfa' ? <MfaPanel /> : null}
        {section === 'webhooks' ? <WebhooksPanel /> : null}
      </view>
    </view>
  )
}
