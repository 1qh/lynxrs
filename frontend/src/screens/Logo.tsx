/**
 * Wordmark logo. Inline SVG via data-URL for the Lynx <image> primitive
 * (Lynx has no native <svg>). The mark is two overlapping circles in the
 * primary color — minimal, theme-agnostic, scales without rasterizing.
 */
export function Logo({ size = 28 }: { size?: number }) {
  const svg = `
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="${size}" height="${size}">
      <circle cx="12" cy="16" r="9" fill="hsl(240,5.9%,10%)"/>
      <circle cx="20" cy="16" r="9" fill="hsl(240,5.9%,10%)" fill-opacity="0.6"/>
    </svg>`
  const src = `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`
  return (
    <image
      src={src}
      style={{ width: `${size}px`, height: `${size}px` }}
      aria-label="simu logo"
    />
  )
}
