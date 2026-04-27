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
      <view className="w-full h-full bg-bg items-center justify-center p-5">
        <text className="text-muted text-sm">{t('app.loading')}</text>
      </view>
    )
  }

  return (
    <CrossRouter>
      <view className="w-full h-full bg-bg items-center justify-center p-5">
        <view className="w-full max-w-[390px] bg-panel rounded-[20px] p-6">
          <view className="flex-row items-center justify-between mb-4">
            <text className="text-[32px] font-bold text-white">{t('app.title')}</text>
            <LangSwitcher />
          </view>
          {user ? <Home /> : <AuthForm />}
        </view>
        <Toasts />
      </view>
    </CrossRouter>
  )
}
