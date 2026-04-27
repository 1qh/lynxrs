import { useEffect, useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { api } from './api/client.js'
import { useAuth, type User } from './state/auth.js'
import { AuthForm } from './screens/Auth.js'
import { Home } from './screens/Home.js'
import { Toasts } from './screens/Toasts.js'
import { CrossRouter } from './lib/Router.js'
import { LangSwitcher } from './screens/LangSwitcher.js'
import { ThemeSwitcher } from './screens/ThemeSwitcher.js'
import { Logo } from './screens/Logo.js'
import './i18n/index.js'
import './state/theme.js'
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
      <view className="w-full h-full bg-background items-center justify-center p-5">
        <text className="text-muted-foreground text-sm">{t('app.loading')}</text>
      </view>
    )
  }

  return (
    <CrossRouter>
      <view className="w-full min-h-full bg-background items-center justify-start p-5 sm:p-8">
        <view className="w-full max-w-[640px] bg-card rounded-lg p-6 sm:p-8 border border-border">
          <view className="flex-row items-center justify-between mb-6">
            <view className="flex-row items-center gap-2">
              <Logo size={28} />
              <text className="text-[28px] font-semibold text-foreground tracking-tight">
                {t('app.title')}
              </text>
            </view>
            <view className="flex-row gap-2">
              <ThemeSwitcher />
              <LangSwitcher />
            </view>
          </view>
          {user ? <Home /> : <AuthForm />}
        </view>
        <Toasts />
      </view>
    </CrossRouter>
  )
}
