const BASE = ''

export async function createLink(originalUrl, shortCode = null) {
  const res = await fetch(`${BASE}/api/links`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      original_url: originalUrl,
      ...(shortCode ? { short_code: shortCode } : {}),
    }),
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({}))
    throw new Error(err.detail || res.statusText || '创建失败')
  }
  return res.json()
}

export async function listLinks(skip = 0, limit = 20) {
  const res = await fetch(`${BASE}/api/links?skip=${skip}&limit=${limit}`)
  if (!res.ok) throw new Error('获取列表失败')
  return res.json()
}

export async function getStats(shortCode) {
  const res = await fetch(`${BASE}/api/links/stats/${encodeURIComponent(shortCode)}`)
  if (!res.ok) throw new Error('获取统计失败')
  return res.json()
}
