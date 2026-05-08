export interface User {
  id: string
  email: string | null
  name: string | null
  avatar_url: string | null
}

export interface SessionView {
  id: string
  user_id: string
  title: string
  target_jd: string | null
  created_at: string
  updated_at: string
  message_count: number
}

export interface Session {
  id: string
  user_id: string
  title: string
  target_jd: string | null
  created_at: string
  updated_at: string
}

export interface Message {
  id: string
  session_id: string
  role: 'user' | 'assistant' | 'system' | 'tool'
  content: string
  created_at: string
}

async function request<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(url, {
    ...init,
    credentials: 'include',
    headers: {
      'Content-Type': 'application/json',
      ...(init?.headers || {}),
    },
  })
  if (!res.ok) {
    let msg = `request failed: ${res.status}`
    try {
      const body = await res.json()
      if (body?.error) msg = body.error
    } catch (_) {}
    throw new Error(msg)
  }
  if (res.status === 204) return null as T
  return res.json() as Promise<T>
}

export const api = {
  async me(): Promise<{ authenticated: boolean; user?: User }> {
    return request('/auth/me')
  },

  loginUrl(): string {
    return '/auth/oidc/login'
  },

  async logout(): Promise<void> {
    await fetch('/auth/logout', { method: 'POST', credentials: 'include' })
  },

  async listSessions(): Promise<SessionView[]> {
    return request('/api/sessions')
  },

  async createSession(payload: { title?: string; target_jd?: string } = {}): Promise<Session> {
    return request('/api/sessions', { method: 'POST', body: JSON.stringify(payload) })
  },

  async updateSession(id: string, payload: Partial<{ title: string; target_jd: string }>): Promise<Session> {
    return request(`/api/sessions/${id}`, { method: 'PATCH', body: JSON.stringify(payload) })
  },

  async deleteSession(id: string): Promise<void> {
    await request(`/api/sessions/${id}`, { method: 'DELETE' })
  },

  async listMessages(sessionId: string): Promise<Message[]> {
    return request(`/api/sessions/${sessionId}/messages`)
  },

  /**
   * Stream a chat completion. Returns the assistant message_id once the
   * upstream confirms it, plus an async iterator of deltas.
   */
  async streamChat(
    sessionId: string,
    content: string,
    handlers: {
      onInit?: (messageId: string) => void
      onDelta: (delta: string) => void
      onDone: () => void
      onError: (err: string) => void
    },
    signal?: AbortSignal,
  ): Promise<void> {
    const res = await fetch(`/api/sessions/${sessionId}/messages`, {
      method: 'POST',
      credentials: 'include',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content }),
      signal,
    })
    if (!res.ok || !res.body) {
      const text = await res.text().catch(() => '')
      handlers.onError(text || `request failed: ${res.status}`)
      return
    }

    const reader = res.body.getReader()
    const decoder = new TextDecoder()
    let buffer = ''

    const handleEvent = (raw: string) => {
      let event = 'message'
      let data = ''
      for (const line of raw.split(/\r?\n/)) {
        if (line.startsWith('event:')) event = line.slice(6).trim()
        else if (line.startsWith('data:')) data += (data ? '\n' : '') + line.slice(5).trim()
      }
      if (!data) return
      try {
        const payload = JSON.parse(data)
        if (event === 'init' && payload.message_id) handlers.onInit?.(payload.message_id)
        else if (event === 'delta' && typeof payload.delta === 'string') handlers.onDelta(payload.delta)
        else if (event === 'done') handlers.onDone()
        else if (event === 'error') handlers.onError(payload.error || 'stream error')
      } catch {
        /* ignore malformed payloads */
      }
    }

    while (true) {
      const { value, done } = await reader.read()
      if (done) break
      buffer += decoder.decode(value, { stream: true })

      // SSE frames are separated by a blank line. Be tolerant of LF or CRLF.
      const normalized = buffer.replace(/\r\n/g, '\n')
      const parts = normalized.split('\n\n')
      buffer = parts.pop() ?? ''
      for (const raw of parts) handleEvent(raw)
    }
  },
}
