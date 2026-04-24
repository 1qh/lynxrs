import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'
import { deflateSync } from 'node:zlib'

const BACKEND = 'http://localhost:8088'

function makeRedPng(w = 16, h = 16): Buffer {
  const sig = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
  const crc32 = (buf: Buffer): number => {
    let c = 0xffffffff
    for (const b of buf) {
      c ^= b
      for (let i = 0; i < 8; i++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1))
    }
    return (c ^ 0xffffffff) >>> 0
  }
  const chunk = (type: string, data: Buffer): Buffer => {
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length, 0)
    const t = Buffer.from(type)
    const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(Buffer.concat([t, data])), 0)
    return Buffer.concat([len, t, data, crc])
  }
  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4)
  ihdr[8] = 8; ihdr[9] = 2
  const rows: Buffer[] = []
  for (let y = 0; y < h; y++) {
    const row = Buffer.alloc(1 + w * 3)
    for (let x = 0; x < w; x++) {
      row[1 + x * 3] = 255
    }
    rows.push(row)
  }
  const idat = deflateSync(Buffer.concat(rows))
  return Buffer.concat([sig, chunk('IHDR', ihdr), chunk('IDAT', idat), chunk('IEND', Buffer.alloc(0))])
}

test('uploading an image produces a thumbnail served as JPEG', async () => {
  const api = await newApi()
  const email = `thumb-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const png = makeRedPng(64, 64)
  const up = await api.post('/api/files/json', {
    data: {
      filename: 'r.png',
      content_type: 'image/png',
      data_base64: png.toString('base64'),
    },
    headers: { 'content-type': 'application/json' },
  })
  const { id } = (await up.json()) as { id: string }

  // Thumbnail is generated asynchronously.
  await expect.poll(async () => {
    const r = await api.get(`/api/files/${id}/thumbnail`)
    return r.status()
  }, { timeout: 10_000 }).toBe(200)

  const res = await api.get(`/api/files/${id}/thumbnail`)
  expect(res.headers()['content-type']).toBe('image/jpeg')
  const bytes = await res.body()
  expect(bytes[0]).toBe(0xff)
  expect(bytes[1]).toBe(0xd8)
})
