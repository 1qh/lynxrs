import { useTranslation } from 'react-i18next'
import { setLang } from '../i18n/index.js'

export function LangSwitcher() {
  const { i18n } = useTranslation()
  const current = (i18n.language?.startsWith('vi') ? 'vi' : 'en') as 'en' | 'vi'
  const next = current === 'en' ? 'vi' : 'en'
  return (
    <view
      className="bg-secondary rounded-md px-2.5 py-1.5 border border-border"
      bindtap={() => setLang(next)}
    >
      <text className="text-secondary-foreground text-[13px] font-medium">
        {next.toUpperCase()}
      </text>
    </view>
  )
}
