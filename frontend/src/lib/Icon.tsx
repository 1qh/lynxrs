import * as lucide from 'lucide-static'
import { useTheme } from '../state/theme.js'

/**
 * Render a lucide-static SVG icon as a Lynx `<image>` via data-URL.
 * Lynx doesn't have a native `<svg>` element, so we inline the SVG
 * markup, swap `currentColor` for the active theme's foreground HSL,
 * and pass the URL-encoded result to `<image src>`.
 */
export function Icon({
  name,
  size = 16,
  className,
}: {
  name: keyof typeof lucide
  size?: number
  className?: string
}) {
  const theme = useTheme((s) => s.theme)
  const fg = theme === 'dark' ? '#fafafa' : '#0a0a0a'
  const raw = (lucide as Record<string, string>)[name as string] ?? ''
  const colored = raw.replaceAll('currentColor', fg)
  const src = `data:image/svg+xml;utf8,${encodeURIComponent(colored)}`
  return className ? (
    <image
      src={src}
      className={className}
      style={{ width: `${size}px`, height: `${size}px` }}
      aria-label={String(name)}
    />
  ) : (
    <image
      src={src}
      style={{ width: `${size}px`, height: `${size}px` }}
      aria-label={String(name)}
    />
  )
}
