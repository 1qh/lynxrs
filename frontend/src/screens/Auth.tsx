import { useCallback, useRef, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../api/client.js'
import { useAuth, type User } from '../state/auth.js'

export function AuthForm() {
  const { t } = useTranslation()
  const setUser = useAuth((s) => s.setUser)
  const [mode, setMode] = useState<'login' | 'signup'>('signup')
  const emailRef = useRef('demo@simu.dev')
  const passwordRef = useRef('hunter2hunter2')
  const [err, setErr] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const submit = useCallback(async () => {
    setErr(null)
    const email = emailRef.current.trim()
    const password = passwordRef.current
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
      setErr(t('auth.email_invalid'))
      return
    }
    if (mode === 'signup' && password.length < 12) {
      setErr(t('auth.password_too_short'))
      return
    }
    setBusy(true)
    try {
      const body = { email, password }
      if (mode === 'signup') {
        const { data, error } = await api.POST('/auth/signup', { body })
        if (error) setErr((error as { message?: string }).message ?? t('auth.failed'))
        else if (data) setUser(data as User)
      } else {
        const { data, error } = await api.POST('/auth/login', { body })
        if (error) setErr((error as { message?: string }).message ?? t('auth.failed'))
        else if (data) setUser(data as User)
      }
    } catch (e) {
      setErr(String(e))
    } finally {
      setBusy(false)
    }
  }, [mode, setUser, t])

  return (
    <view className="Card">
      <text className="H2">{mode === 'signup' ? t('auth.create_account') : t('auth.login')}</text>
      <input
        className="Input"
        placeholder={t('auth.email')}
        type="email"
        bindinput={(e: { detail: { value: string } }) => { emailRef.current = e.detail.value }}
      />
      <input
        className="Input"
        placeholder={t('auth.password')}
        type="password"
        bindinput={(e: { detail: { value: string } }) => { passwordRef.current = e.detail.value }}
      />
      {err ? <text className="Error">{err}</text> : null}
      <view className="Button" bindtap={busy ? undefined : submit}>
        <text className="ButtonText">{busy ? '…' : mode === 'signup' ? t('auth.sign_up') : t('auth.log_in')}</text>
      </view>
      <view
        className="SwitchRow"
        bindtap={() => setMode(mode === 'signup' ? 'login' : 'signup')}
      >
        <text className="SwitchText">
          {mode === 'signup' ? t('auth.have_account') : t('auth.new_here')}
        </text>
      </view>
    </view>
  )
}
