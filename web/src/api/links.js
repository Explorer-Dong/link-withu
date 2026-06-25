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

export async function listLinks(skip = 0, limit = 20, q = '') {
  const params = new URLSearchParams()
  params.set('skip', String(skip))
  params.set('limit', String(limit))
  if (q) params.set('q', q)
  const res = await fetch(`${BASE}/api/links?${params}`)
  if (!res.ok) throw new Error('获取列表失败')
  return res.json()
}

function adminHeaders(token) {
  return token ? { Authorization: `Bearer ${token}` } : {}
}

export async function adminLogin(password) {
  const res = await fetch(`${BASE}/api/admin/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ password }),
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({}))
    throw new Error(err.detail || '登录失败')
  }
  return res.json()
}

export async function deleteLink(shortCode, token) {
  const res = await fetch(`${BASE}/api/links/${encodeURIComponent(shortCode)}`, {
    method: 'DELETE',
    headers: adminHeaders(token),
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({}))
    throw new Error(err.detail || `删除失败 (${res.status})`)
  }
  return true
}

export async function getStats(shortCode, token) {
  const res = await fetch(`${BASE}/api/links/stats/${encodeURIComponent(shortCode)}`, {
    headers: adminHeaders(token),
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({}))
    throw new Error(err.detail || '获取统计失败')
  }
  return res.json()
}
