import { useCallback, useState } from '@lynx-js/react'
import { api } from '../../api/client.js'
import { useAuth } from '../../state/auth.js'

export function MfaPanel() {
  const user = useAuth((s) => s.user)!
  const setUser = useAuth((s) => s.setUser)
  const [mfaSecret, setMfaSecret] = useState<string | null>(null)
  const [mfaCode, setMfaCode] = useState('')
  const [recoveryCodes, setRecoveryCodes] = useState<string[]>([])

  const enroll = useCallback(async () => {
    const { data, error } = await api.POST('/mfa/enroll', {})
    if (error) { console.error('[mfa]', error); return }
    setMfaSecret((data as { secret: string }).secret)
  }, [])

  const activate = useCallback(async () => {
    const code = mfaCode.trim()
    if (!code) return
    const { error } = await api.POST('/mfa/activate', { body: { code } })
    if (error) { console.error('[mfa activate]', error); return }
    const { data } = await api.GET('/auth/me', {})
    if (data) setUser(data as typeof user)
    setMfaCode('')
    setMfaSecret(null)
  }, [mfaCode, setUser, user])

  const generateRecovery = useCallback(async () => {
    const { data, error } = await api.POST('/mfa/recovery-codes', {})
    if (error) { console.error('[mfa recovery]', error); return }
    setRecoveryCodes((data as { codes: string[] }).codes)
  }, [])

  if (!user.totp_enabled) {
    return (
      <view>
        <view className="Button ButtonGhost" bindtap={enroll}>
          <text className="ButtonText">Enable MFA (TOTP)</text>
        </view>
        {mfaSecret ? (
          <view>
            <text className="Muted">Scan this secret into your authenticator:</text>
            <text className="FileName">{mfaSecret}</text>
            <input
              className="Input"
              placeholder="6-digit code"
              type="text"
              bindinput={(e: { detail: { value: string } }) => setMfaCode(e.detail.value)}
            />
            <view className="Button" bindtap={activate}>
              <text className="ButtonText">Activate MFA</text>
            </view>
          </view>
        ) : null}
      </view>
    )
  }

  return (
    <view>
      <text className="Muted">MFA enabled ✓</text>
      <view className="Button ButtonGhost" bindtap={generateRecovery}>
        <text className="ButtonText">Generate recovery codes</text>
      </view>
      {recoveryCodes.length > 0 ? (
        <view className="RecoveryCodes">
          <text className="Muted">Save these codes now — each works once:</text>
          {recoveryCodes.map((c, i) => (
            <text key={i} className="FileName">{c}</text>
          ))}
        </view>
      ) : null}
    </view>
  )
}
