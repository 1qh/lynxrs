import { useCallback, useRef, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from '../api/client.js'
import { useAuth, type User } from '../state/auth.js'
import { passwordStrength } from '../lib/passwordStrength.js'

export function AuthForm() {
  const { t } = useTranslation()
  const setUser = useAuth((s) => s.setUser)
  const [mode, setMode] = useState<'login' | 'signup'>('signup')
  const emailRef = useRef('demo@simu.dev')
  const passwordRef = useRef('hunter2hunter2')
  const [pwLive, setPwLive] = useState('hunter2hunter2')
  const [err, setErr] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const strength = passwordStrength(pwLive)

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
    <view className="gap-3">
      <text className="text-xl font-semibold text-foreground">
        {mode === 'signup' ? t('auth.create_account') : t('auth.login')}
      </text>
      <input
        className="h-10 rounded-md bg-background text-foreground px-3 text-sm border border-input"
        placeholder={t('auth.email')}
        type="email"
        bindinput={(e: { detail: { value: string } }) => { emailRef.current = e.detail.value }}
      />
      <input
        className="h-10 rounded-md bg-background text-foreground px-3 text-sm border border-input"
        placeholder={t('auth.password')}
        type="password"
        bindinput={(e: { detail: { value: string } }) => {
          passwordRef.current = e.detail.value
          setPwLive(e.detail.value)
        }}
      />
      {mode === 'signup' ? (
        <view className="flex-row gap-1 h-1">
          {[0, 1, 2, 3].map((i) => (
            <view
              key={i}
              className={
                i < strength.score
                  ? 'flex-1 rounded-full bg-primary'
                  : 'flex-1 rounded-full bg-muted'
              }
            />
          ))}
        </view>
      ) : null}
      {mode === 'signup' && pwLive.length > 0 ? (
        <text className="text-xs text-muted-foreground">{strength.label}</text>
      ) : null}
      {err ? <text className="text-destructive text-sm">{err}</text> : null}
      <view
        className="h-10 rounded-md bg-primary items-center justify-center"
        bindtap={busy ? undefined : submit}
      >
        <text className="text-primary-foreground text-sm font-medium">
          {busy ? '…' : mode === 'signup' ? t('auth.sign_up') : t('auth.log_in')}
        </text>
      </view>
      <view
        className="items-center py-2"
        bindtap={() => setMode(mode === 'signup' ? 'login' : 'signup')}
      >
        <text className="text-muted-foreground text-sm">
          {mode === 'signup' ? t('auth.have_account') : t('auth.new_here')}
        </text>
      </view>
    </view>
  )
}
