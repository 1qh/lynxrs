import { useCallback, useRef, useState } from '@lynx-js/react'
import { api } from '../api/client.js'
import { useAuth, type User } from '../state/auth.js'

export function AuthForm() {
  const setUser = useAuth((s) => s.setUser)
  const [mode, setMode] = useState<'login' | 'signup'>('signup')
  const emailRef = useRef('demo@simu.dev')
  const passwordRef = useRef('hunter2hunter2')
  const [err, setErr] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const submit = useCallback(async () => {
    setErr(null); setBusy(true)
    try {
      const body = { email: emailRef.current, password: passwordRef.current }
      if (mode === 'signup') {
        const { data, error } = await api.POST('/auth/signup', { body })
        if (error) setErr((error as { message?: string }).message ?? 'failed')
        else if (data) setUser(data as User)
      } else {
        const { data, error } = await api.POST('/auth/login', { body })
        if (error) setErr((error as { message?: string }).message ?? 'failed')
        else if (data) setUser(data as User)
      }
    } catch (e) {
      setErr(String(e))
    } finally {
      setBusy(false)
    }
  }, [mode, setUser])

  return (
    <view className="Card">
      <text className="H2">{mode === 'signup' ? 'Create account' : 'Login'}</text>
      <input
        className="Input"
        placeholder="email"
        type="email"
        bindinput={(e: { detail: { value: string } }) => { emailRef.current = e.detail.value }}
      />
      <input
        className="Input"
        placeholder="password"
        type="password"
        bindinput={(e: { detail: { value: string } }) => { passwordRef.current = e.detail.value }}
      />
      {err ? <text className="Error">{err}</text> : null}
      <view className="Button" bindtap={busy ? undefined : submit}>
        <text className="ButtonText">{busy ? '…' : mode === 'signup' ? 'Sign up' : 'Log in'}</text>
      </view>
      <view
        className="SwitchRow"
        bindtap={() => setMode(mode === 'signup' ? 'login' : 'signup')}
      >
        <text className="SwitchText">
          {mode === 'signup' ? 'Have an account? Log in' : 'New here? Sign up'}
        </text>
      </view>
    </view>
  )
}
