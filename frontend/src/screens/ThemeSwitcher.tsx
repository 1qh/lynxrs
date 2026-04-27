import { useTheme } from '../state/theme.js'

export function ThemeSwitcher() {
  const theme = useTheme((s) => s.theme)
  const toggle = useTheme((s) => s.toggle)
  return (
    <view
      className="bg-secondary rounded-md px-2.5 py-1.5 border border-border"
      bindtap={toggle}
    >
      <text className="text-secondary-foreground text-[13px] font-medium">
        {theme === 'light' ? '🌙' : '☀'}
      </text>
    </view>
  )
}
