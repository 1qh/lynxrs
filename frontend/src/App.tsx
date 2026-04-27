import { useEffect, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from './api/client.js'
import { useAuth, type User } from './state/auth.js'
import { AuthForm } from './screens/Auth.js'
import { Home } from './screens/Home.js'
import { Toasts } from './screens/Toasts.js'
import { CrossRouter } from './lib/Router.js'
import { LangSwitcher } from './screens/LangSwitcher.js'
import './i18n/index.js'
import './App.css'

export function App() {
  const user = useAuth((s) => s.user)
  const setUser = useAuth((s) => s.setUser)
  const [boot, setBoot] = useState(true)
  const { t } = useTranslation()

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
        <text className="Subtitle">{t('app.loading')}</text>
      </view>
    )
  }

  return (
    <CrossRouter>
      <view className="Screen">
        <view className="Frame">
          <view className="HeaderRow">
            <text className="Title">{t('app.title')}</text>
            <LangSwitcher />
          </view>
          {user ? <Home /> : <AuthForm />}
        </view>
        <Toasts />
      </view>
    </CrossRouter>
  )
}
