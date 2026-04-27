import { useCallback, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../../api/client.js'
import { useAuth } from '../../state/auth.js'
import { reportError } from '../../state/toast.js'

export function ProfilePanel() {
  const { t } = useTranslation()
  const user = useAuth((s) => s.user)!
  const setUser = useAuth((s) => s.setUser)
  const [displayName, setDisplayName] = useState<string>(user.display_name ?? '')
  const [avatar, setAvatar] = useState<string | null>(user.avatar_url ?? null)

  const save = useCallback(async () => {
    const name = displayName.trim()
    if (!name) return
    const { data } = await api.PATCH('/auth/me', { body: { display_name: name } })
    if (data) setUser(data as typeof user)
  }, [displayName, setUser, user])

  const pickAvatar = useCallback(() => {
    const doc = (globalThis as { document?: Document }).document
    if (!doc) return
    const el = doc.createElement('input')
    el.type = 'file'
    el.accept = 'image/*'
    el.onchange = async () => {
      const f = el.files?.[0]
      if (!f) return
      try {
        const ab = await f.arrayBuffer()
        const bytes = new Uint8Array(ab)
        let bin = ''
        for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]!)
        const dataUrl = `data:${f.type || 'image/png'};base64,${btoa(bin)}`
        const { data, error } = await api.PATCH('/auth/me', {
          body: { avatar_url: dataUrl },
        })
        if (error) {
          reportError(error, 'Avatar upload failed')
          return
        }
        if (data) {
          setUser(data as typeof user)
          setAvatar(dataUrl)
        }
      } catch (e) {
        reportError(e, 'Avatar upload threw')
      }
    }
    el.click()
  }, [setUser, user])

  return (
    <view className="gap-3">
      <view className="flex-row items-center gap-3">
        <view
          className="w-16 h-16 rounded-full bg-muted overflow-hidden border border-border items-center justify-center"
          bindtap={pickAvatar}
        >
          {avatar ? (
            <image src={avatar} className="w-16 h-16 rounded-full" />
          ) : (
            <text className="text-muted-foreground text-xl">
              {(user.display_name ?? user.email).charAt(0).toUpperCase()}
            </text>
          )}
        </view>
        <view className="flex-1 gap-1">
          <text className="text-foreground text-sm font-medium">
            {user.display_name ?? user.email}
          </text>
          <text className="text-xs text-muted-foreground">{user.email}</text>
        </view>
      </view>
      <input
        className="h-10 rounded-md bg-background text-foreground px-3 text-sm border border-input"
        placeholder={t('profile.display_name_placeholder')}
        type="text"
        bindinput={(e: { detail: { value: string } }) => setDisplayName(e.detail.value)}
      />
      <view
        className="h-10 rounded-md bg-primary items-center justify-center"
        bindtap={save}
      >
        <text className="text-primary-foreground text-sm font-medium">
          {t('profile.save_profile')}
        </text>
      </view>
    </view>
  )
}
