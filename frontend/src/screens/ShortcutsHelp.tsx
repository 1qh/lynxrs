import { useState } from '@lynx-js/react'
import { useTranslation } from 'react-i18next'
import { useKeyboardShortcuts } from '../lib/useKeyboardShortcuts.js'

const ROWS: ReadonlyArray<{ keys: string; labelKey: string }> = [
  { keys: 'g f', labelKey: 'shortcuts.go_files' },
  { keys: 'g o', labelKey: 'shortcuts.go_orgs' },
  { keys: 'g t', labelKey: 'shortcuts.go_trash' },
  { keys: 'g a', labelKey: 'shortcuts.go_audit' },
  { keys: 'g s', labelKey: 'shortcuts.go_settings' },
  { keys: 'g d', labelKey: 'shortcuts.go_admin' },
  { keys: '?', labelKey: 'shortcuts.show_help' },
  { keys: 'esc', labelKey: 'shortcuts.close' },
]

export function ShortcutsHelp() {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)

  useKeyboardShortcuts([
    { key: '?', shift: true, handler: () => setOpen(true) },
    { key: 'escape', handler: () => setOpen(false) },
  ])

  if (!open) return null
  return (
    <view
      className="fixed inset-0 bg-background/95 items-center justify-center p-6 z-[9200]"
      bindtap={() => setOpen(false)}
    >
      <view className="rounded-md bg-card border border-border p-5 gap-3 min-w-[320px] max-w-[90%]">
        <text className="text-base font-semibold text-foreground">{t('shortcuts.title')}</text>
        <view className="gap-1">
          {ROWS.map((r) => (
            <view key={r.keys} className="flex-row items-center justify-between py-1">
              <text className="text-sm text-foreground">{t(r.labelKey)}</text>
              <text className="text-xs font-mono text-muted-foreground bg-secondary rounded px-2 py-0.5">
                {r.keys}
              </text>
            </view>
          ))}
        </view>
        <view
          className="h-9 rounded-md bg-background border border-input items-center justify-center mt-2"
          bindtap={() => setOpen(false)}
          aria-label={t('shortcuts.close')}
        >
          <text className="text-foreground text-sm font-medium">{t('shortcuts.close')}</text>
        </view>
      </view>
    </view>
  )
}
