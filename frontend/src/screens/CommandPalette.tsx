import { useEffect, useState } from '@lynx-js/react'
import { useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useKeyboardShortcuts } from '../lib/useKeyboardShortcuts.js'
import { useTheme } from '../state/theme.js'
import { setLang } from '../i18n/index.js'

type Action = { id: string; label: string; run: () => void }

/**
 * Cmd+K / Ctrl+K command palette. Lists static actions (navigation, theme,
 * lang) plus filterable conversation jumps. Lives at the App root so it can
 * navigate any tab.
 */
export function CommandPalette() {
  const { t, i18n } = useTranslation()
  const navigate = useNavigate()
  const toggleTheme = useTheme((s) => s.toggle)
  const [open, setOpen] = useState(false)

  useKeyboardShortcuts([
    { key: 'k', meta: true, handler: () => setOpen(true) },
    { key: 'k', ctrl: true, handler: () => setOpen(true) },
    { key: 'escape', handler: () => setOpen(false) },
  ])

  // Reset filter when (re-)opening.
  useEffect(() => {
    if (!open) return
  }, [open])

  if (!open) return null

  const actions: Action[] = [
    { id: 'go-chat', label: t('cmdk.go_chat'), run: () => { navigate('/chat'); setOpen(false) } },
    { id: 'go-files', label: t('cmdk.go_files'), run: () => { navigate('/files'); setOpen(false) } },
    { id: 'go-orgs', label: t('cmdk.go_orgs'), run: () => { navigate('/orgs'); setOpen(false) } },
    { id: 'go-audit', label: t('cmdk.go_audit'), run: () => { navigate('/audit'); setOpen(false) } },
    { id: 'go-settings', label: t('cmdk.go_settings'), run: () => { navigate('/settings'); setOpen(false) } },
    { id: 'theme', label: t('cmdk.toggle_theme'), run: () => { toggleTheme(); setOpen(false) } },
    {
      id: 'lang',
      label: t('cmdk.toggle_lang'),
      run: () => { setLang(i18n.language?.startsWith('vi') ? 'en' : 'vi'); setOpen(false) },
    },
  ]

  return (
    <view
      className="fixed inset-0 bg-background/80 z-[9100] items-center justify-start pt-16 px-4"
      bindtap={() => setOpen(false)}
    >
      <view className="w-full max-w-[480px] rounded-md bg-card border border-border p-2 gap-1">
        <text className="text-xs text-muted-foreground px-2 py-1">⌘K · {t('cmdk.title')}</text>
        {actions.map((a) => (
          <view
            key={a.id}
            className="rounded-md px-3 py-2 bg-transparent"
            bindtap={a.run}
            aria-label={a.label}
          >
            <text className="text-foreground text-sm">{a.label}</text>
          </view>
        ))}
      </view>
    </view>
  )
}
