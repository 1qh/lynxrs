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
    <view className="gap-3 p-1">
      <text className="text-xl font-semibold text-white mb-2">
        {mode === 'signup' ? t('auth.create_account') : t('auth.login')}
      </text>
      <input
        className="h-11 rounded-[10px] bg-card text-white px-3.5 text-base border border-border"
        placeholder={t('auth.email')}
        type="email"
        bindinput={(e: { detail: { value: string } }) => { emailRef.current = e.detail.value }}
      />
      <input
        className="h-11 rounded-[10px] bg-card text-white px-3.5 text-base border border-border"
        placeholder={t('auth.password')}
        type="password"
        bindinput={(e: { detail: { value: string } }) => { passwordRef.current = e.detail.value }}
      />
      {err ? <text className="text-danger text-sm">{err}</text> : null}
      <view
        className="h-11 rounded-[10px] bg-accent items-center justify-center mt-1"
        bindtap={busy ? undefined : submit}
      >
        <text className="text-white text-base font-semibold">
          {busy ? '…' : mode === 'signup' ? t('auth.sign_up') : t('auth.log_in')}
        </text>
      </view>
      <view
        className="items-center p-2.5"
        bindtap={() => setMode(mode === 'signup' ? 'login' : 'signup')}
      >
        <text className="text-[#8aa2ff] text-sm">
          {mode === 'signup' ? t('auth.have_account') : t('auth.new_here')}
        </text>
      </view>
    </view>
  )
}
