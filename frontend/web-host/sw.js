/* Service worker for simu PWA. Cache-first for the app shell so previously-
 * visited pages and bundles open offline; network-first for /api. */
const SHELL = 'simu-shell-v1'
const SHELL_PATHS = ['/', '/index.html', '/manifest.webmanifest', '/main.web.bundle', '/entry.js', '/entry.css']

self.addEventListener('install', (e) => {
  e.waitUntil(
    caches.open(SHELL).then((c) => Promise.all(
      SHELL_PATHS.map((p) => c.add(p).catch(() => {})),
    )).then(() => self.skipWaiting()),
  )
})

self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches.keys().then((keys) => Promise.all(
      keys.filter((k) => k !== SHELL).map((k) => caches.delete(k)),
    )).then(() => self.clients.claim()),
  )
})

self.addEventListener('fetch', (e) => {
  const url = new URL(e.request.url)
  // API: always go to network. Backend SSE/JSON should never be cached.
  if (url.pathname.startsWith('/api/')) return
  // Same-origin static: cache-first, fallback to network.
  if (url.origin === self.location.origin) {
    e.respondWith(
      caches.match(e.request).then((hit) => hit ?? fetch(e.request).then((res) => {
        const copy = res.clone()
        caches.open(SHELL).then((c) => c.put(e.request, copy)).catch(() => {})
        return res
      }).catch(() => caches.match('/'))),
    )
  }
})
