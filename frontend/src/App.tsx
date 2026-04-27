import { useEffect, useState } from '@lynx-js/react'
import { api } from './api/client.js'
import { useAuth, type User } from './state/auth.js'
import { AuthForm } from './screens/Auth.js'
import { Home } from './screens/Home.js'
import { Toasts } from './screens/Toasts.js'
import { CrossRouter } from './lib/Router.js'
import './App.css'

export function App() {
  const user = useAuth((s) => s.user)
  const setUser = useAuth((s) => s.setUser)
  const [boot, setBoot] = useState(true)

  useEffect(() => {
    ;(async () => {
      const { data } = await api.GET('/auth/me', {})
      if (data) setUser(data as User)
      setBoot(false)
    })()
  }, [setUser])

  if (boot) {
    return (
      <view className="Screen">
        <text className="Subtitle">loading…</text>
      </view>
    )
  }

  return (
    <CrossRouter>
      <view className="Screen">
        <view className="Frame">
          <text className="Title">simu</text>
          {user ? <Home /> : <AuthForm />}
        </view>
        <Toasts />
      </view>
    </CrossRouter>
  )
}
