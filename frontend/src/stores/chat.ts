import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api, type AttachmentSummary, type Message, type SessionView } from '../api'

export const useChatStore = defineStore('chat', () => {
  const sessions = ref<SessionView[]>([])
  const messages = ref<Message[]>([])
  const attachments = ref<AttachmentSummary[]>([])
  const activeId = ref<string | null>(null)
  const streaming = ref(false)
  const uploading = ref(false)
  const error = ref<string | null>(null)

  async function loadSessions() {
    try {
      sessions.value = await api.listSessions()
    } catch (e) {
      error.value = (e as Error).message
    }
  }

  async function selectSession(id: string) {
    activeId.value = id
    messages.value = []
    attachments.value = []
    try {
      const [msgs, atts] = await Promise.all([
        api.listMessages(id),
        api.listAttachments(id),
      ])
      messages.value = msgs
      attachments.value = atts
    } catch (e) {
      error.value = (e as Error).message
    }
  }

  async function loadAttachments() {
    if (!activeId.value) return
    try {
      attachments.value = await api.listAttachments(activeId.value)
    } catch (e) {
      error.value = (e as Error).message
    }
  }

  async function uploadAttachment(file: File): Promise<boolean> {
    if (!activeId.value) {
      const id = await createSession()
      activeId.value = id
    }
    uploading.value = true
    error.value = null
    try {
      const att = await api.uploadAttachment(activeId.value!, file)
      attachments.value = [att, ...attachments.value]
      return true
    } catch (e) {
      error.value = (e as Error).message
      return false
    } finally {
      uploading.value = false
    }
  }

  async function removeAttachment(id: string) {
    if (!activeId.value) return
    try {
      await api.deleteAttachment(activeId.value, id)
      attachments.value = attachments.value.filter((a) => a.id !== id)
    } catch (e) {
      error.value = (e as Error).message
    }
  }

  async function createSession(title?: string): Promise<string> {
    const created = await api.createSession({ title })
    await loadSessions()
    return created.id
  }

  async function renameSession(id: string, title: string) {
    await api.updateSession(id, { title })
    const s = sessions.value.find((x) => x.id === id)
    if (s) s.title = title
  }

  async function removeSession(id: string) {
    await api.deleteSession(id)
    sessions.value = sessions.value.filter((s) => s.id !== id)
    if (activeId.value === id) {
      activeId.value = null
      messages.value = []
      attachments.value = []
    }
  }

  async function send(content: string) {
    if (!activeId.value) return
    const sessionId = activeId.value

    // Optimistically render the user message.
    const tempUserId = `temp-user-${Date.now()}`
    messages.value.push({
      id: tempUserId,
      session_id: sessionId,
      role: 'user',
      content,
      created_at: new Date().toISOString(),
    })

    const tempAssistantId = `temp-assistant-${Date.now()}`
    messages.value.push({
      id: tempAssistantId,
      session_id: sessionId,
      role: 'assistant',
      content: '',
      created_at: new Date().toISOString(),
    })

    streaming.value = true
    error.value = null
    try {
      await api.streamChat(sessionId, content, {
        onInit(messageId) {
          const target = messages.value.find((m) => m.id === tempAssistantId)
          if (target) target.id = messageId
        },
        onDelta(delta) {
          const target = messages.value[messages.value.length - 1]
          if (target?.role === 'assistant') target.content += delta
        },
        onDone() {
          streaming.value = false
          // refresh sessions list ordering
          loadSessions()
        },
        onError(msg) {
          error.value = msg
          streaming.value = false
        },
      })
    } catch (e) {
      error.value = (e as Error).message
      streaming.value = false
    }
  }

  return {
    sessions,
    messages,
    attachments,
    activeId,
    streaming,
    uploading,
    error,
    loadSessions,
    selectSession,
    loadAttachments,
    uploadAttachment,
    removeAttachment,
    createSession,
    renameSession,
    removeSession,
    send,
  }
})
