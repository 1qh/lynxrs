import { test, expect } from '@playwright/test'

test('debug: capture console + screenshot', async ({ page }) => {
  const logs: string[] = []
  page.on('console', (msg) => logs.push(`[${msg.type()}] ${msg.text()}`))
  page.on('pageerror', (err) => logs.push(`[pageerror] ${err.message}`))
  page.on('requestfailed', (req) => logs.push(`[req-fail] ${req.url()} ${req.failure()?.errorText}`))

  await page.goto('/__web_preview?casename=main.web.bundle', { waitUntil: 'networkidle' })
  await page.waitForTimeout(4000)

  await page.screenshot({ path: 'test-results/debug-boot.png', fullPage: true })

  const html = await page.content()
  console.log('=== HTML ===')
  console.log(html.slice(0, 2000))
  console.log('=== LOGS ===')
  for (const l of logs) console.log(l)

  // Inspect the lynx-view element shadow
  const lv = page.locator('lynx-view')
  await expect(lv).toBeVisible()
  const shadowHtml = await lv.evaluate((el) => (el.shadowRoot?.innerHTML ?? '[no shadow]'))
  console.log('=== lynx-view shadowRoot ===')
  console.log(shadowHtml.slice(0, 4000))

  // Look for lynx-text / x-view / xtext elements
  const texts = await lv.evaluate((el) => {
    const root = el.shadowRoot as ShadowRoot | null
    if (!root) return []
    const all = root.querySelectorAll('*')
    return Array.from(all).slice(0, 30).map((n) => ({
      tag: n.tagName.toLowerCase(),
      text: (n.textContent ?? '').slice(0, 60),
    }))
  })
  console.log('=== shadow children sample ===')
  for (const t of texts) console.log(t.tag, '|', t.text)
})
