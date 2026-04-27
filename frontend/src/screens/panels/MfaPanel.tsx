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
        <view className="Button ButtonGhost" bindtap={enroll}>
          <text className="ButtonText">{t('mfa.enable')}</text>
        </view>
        {mfaSecret ? (
          <view>
            <text className="Muted">{t('mfa.scan')}</text>
            <text className="FileName">{mfaSecret}</text>
            <input
              className="Input"
              placeholder={t('mfa.code_placeholder')}
              type="text"
              bindinput={(e: { detail: { value: string } }) => setMfaCode(e.detail.value)}
            />
            <view className="Button" bindtap={activate}>
              <text className="ButtonText">{t('mfa.activate')}</text>
            </view>
          </view>
        ) : null}
      </view>
    )
  }

  return (
    <view>
      <text className="Muted">{t('mfa.enabled')}</text>
      <view className="Button ButtonGhost" bindtap={generateRecovery}>
        <text className="ButtonText">{t('mfa.generate_recovery')}</text>
      </view>
      {recoveryCodes.length > 0 ? (
        <view className="RecoveryCodes">
          <text className="Muted">{t('mfa.recovery_save')}</text>
          {recoveryCodes.map((c, i) => (
            <text key={i} className="FileName">{c}</text>
          ))}
        </view>
      ) : null}
    </view>
  )
}
