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
import { ShortcutsHelp } from './screens/ShortcutsHelp.js'
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
      <view className="w-full h-full bg-background items-center justify-center">
        <text className="text-muted-foreground text-sm">{t('app.loading')}</text>
      </view>
    )
  }

  return (
    <CrossRouter>
      {/* Vertical mobile-app shell. Header, scrolling body, optional bottom
          tab bar (rendered inside Home). Full-bleed within the phone frame. */}
      <view className="w-full h-full bg-background">
        <view className="flex-row items-center justify-between px-4 py-3 border-b border-border bg-background">
          <view className="flex-row items-center gap-2">
            <Logo size={22} />
            <text className="text-base font-semibold text-foreground tracking-tight">
              {t('app.title')}
            </text>
          </view>
          <view className="flex-row gap-2">
            <ThemeSwitcher />
            <LangSwitcher />
          </view>
        </view>
        <view className="flex-1 overflow-auto">
          {user ? <Home /> : (
            <view className="px-4 py-6">
              <AuthForm />
            </view>
          )}
        </view>
        <Toasts />
        <ShortcutsHelp />
      </view>
    </CrossRouter>
  )
}
