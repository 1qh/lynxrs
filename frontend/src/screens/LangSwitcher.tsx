import { useTranslation } from 'react-i18next'
import { setLang } from '../i18n/index.js'

export function LangSwitcher() {
  const { i18n } = useTranslation()
  const current = (i18n.language?.startsWith('vi') ? 'vi' : 'en') as 'en' | 'vi'
  const next = current === 'en' ? 'vi' : 'en'
  return (
    <view className="LangSwitch" bindtap={() => setLang(next)}>
      <text className="LangSwitchText">{next.toUpperCase()}</text>
    </view>
  )
}
