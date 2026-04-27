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
      <view>
        <view
          className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
          bindtap={enroll}
        >
          <text className="text-white text-base font-semibold">{t('mfa.enable')}</text>
        </view>
        {mfaSecret ? (
          <view>
            <text className="text-muted text-sm py-2.5">{t('mfa.scan')}</text>
            <text className="text-white text-[15px] font-medium">{mfaSecret}</text>
            <input
              className="h-11 rounded-[10px] bg-card text-white px-3.5 text-base border border-border"
              placeholder={t('mfa.code_placeholder')}
              type="text"
              bindinput={(e: { detail: { value: string } }) => setMfaCode(e.detail.value)}
            />
            <view
              className="h-11 rounded-[10px] bg-accent items-center justify-center mt-1"
              bindtap={activate}
            >
              <text className="text-white text-base font-semibold">{t('mfa.activate')}</text>
            </view>
          </view>
        ) : null}
      </view>
    )
  }

  return (
    <view>
      <text className="text-muted text-sm py-2.5">{t('mfa.enabled')}</text>
      <view
        className="h-11 rounded-[10px] items-center justify-center mt-1 bg-transparent border border-border"
        bindtap={generateRecovery}
      >
        <text className="text-white text-base font-semibold">{t('mfa.generate_recovery')}</text>
      </view>
      {recoveryCodes.length > 0 ? (
        <view className="mt-3">
          <text className="text-muted text-sm py-2.5">{t('mfa.recovery_save')}</text>
          {recoveryCodes.map((c, i) => (
            <text key={i} className="text-white text-[15px] font-medium">{c}</text>
          ))}
        </view>
      ) : null}
    </view>
  )
}
