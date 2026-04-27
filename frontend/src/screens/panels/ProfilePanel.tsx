import { useCallback, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../../api/client.js'
import { useAuth } from '../../state/auth.js'

export function ProfilePanel() {
  const { t } = useTranslation()
  const user = useAuth((s) => s.user)!
  const setUser = useAuth((s) => s.setUser)
  const [displayName, setDisplayName] = useState<string>(user.display_name ?? '')

  const save = useCallback(async () => {
    const name = displayName.trim()
    if (!name) return
    const { data } = await api.PATCH('/auth/me', { body: { display_name: name } })
    if (data) setUser(data as typeof user)
  }, [displayName, setUser, user])

  return (
    <view>
      <text className="Muted">
        {t('profile.display_name_label', { value: user.display_name ?? t('profile.display_name_empty') })}
      </text>
      <input
        className="Input"
        placeholder={t('profile.display_name_placeholder')}
        type="text"
        bindinput={(e: { detail: { value: string } }) => setDisplayName(e.detail.value)}
      />
      <view className="Button ButtonGhost" bindtap={save}>
        <text className="ButtonText">{t('profile.save_profile')}</text>
      </view>
    </view>
  )
}
