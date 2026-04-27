import { useCallback, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../../api/client.js'
import { useAuth } from '../../state/auth.js'
import { reportError } from '../../state/toast.js'

export function MfaPanel() {
  const { t } = useTranslation()
  const user = useAuth((s) => s.user)!
  const setUser = useAuth((s) => s.setUser)
  const [mfaSecret, setMfaSecret] = useState<string | null>(null)
  const [mfaCode, setMfaCode] = useState('')
  const [recoveryCodes, setRecoveryCodes] = useState<string[]>([])

  const enroll = useCallback(async () => {
    const { data, error } = await api.POST('/mfa/enroll', {})
    if (error) { reportError(error, 'MFA enroll failed'); return }
    setMfaSecret((data as { secret: string }).secret)
  }, [])

  const activate = useCallback(async () => {
    const code = mfaCode.trim()
    if (!code) return
    const { error } = await api.POST('/mfa/activate', { body: { code } })
    if (error) { reportError(error, 'MFA activate failed'); return }
    const { data } = await api.GET('/auth/me', {})
    if (data) setUser(data as typeof user)
    setMfaCode('')
    setMfaSecret(null)
  }, [mfaCode, setUser, user])

  const generateRecovery = useCallback(async () => {
    const { data, error } = await api.POST('/mfa/recovery-codes', {})
    if (error) { reportError(error, 'Recovery codes failed'); return }
    setRecoveryCodes((data as { codes: string[] }).codes)
  }, [])

  if (!user.totp_enabled) {
    return (
      <view className="gap-3">
        <view
          className="h-10 rounded-md bg-primary items-center justify-center"
          bindtap={enroll}
        >
          <text className="text-primary-foreground text-sm font-medium">
            {t('mfa.enable')}
          </text>
        </view>
        {mfaSecret ? (
          <view className="rounded-md bg-card border border-border p-3 gap-2">
            <text className="text-sm text-muted-foreground">{t('mfa.scan')}</text>
            <text className="text-foreground text-sm font-mono">{mfaSecret}</text>
            <input
              className="h-10 rounded-md bg-background text-foreground px-3 text-sm border border-input"
              placeholder={t('mfa.code_placeholder')}
              type="text"
              bindinput={(e: { detail: { value: string } }) => setMfaCode(e.detail.value)}
            />
            <view
              className="h-10 rounded-md bg-primary items-center justify-center"
              bindtap={activate}
            >
              <text className="text-primary-foreground text-sm font-medium">
                {t('mfa.activate')}
              </text>
            </view>
          </view>
        ) : null}
      </view>
    )
  }

  return (
    <view className="gap-3">
      <text className="text-sm text-muted-foreground">{t('mfa.enabled')}</text>
      <view
        className="h-10 rounded-md bg-background border border-input items-center justify-center"
        bindtap={generateRecovery}
      >
        <text className="text-foreground text-sm font-medium">
          {t('mfa.generate_recovery')}
        </text>
      </view>
      {recoveryCodes.length > 0 ? (
        <view className="rounded-md bg-card border border-border p-3 gap-1">
          <text className="text-sm text-muted-foreground">{t('mfa.recovery_save')}</text>
          {recoveryCodes.map((c, i) => (
            <text key={i} className="text-foreground text-sm font-mono">{c}</text>
          ))}
        </view>
      ) : null}
    </view>
  )
}
