<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth'
import { useChatStore } from '../stores/chat'
import { api } from '../api'
import Sidebar from '../components/Sidebar.vue'
import MessageList from '../components/MessageList.vue'
import Composer from '../components/Composer.vue'
import SettingsModal from '../components/SettingsModal.vue'

const route = useRoute()
const router = useRouter()
const auth = useAuthStore()
const chat = useChatStore()

const messagesEl = ref<HTMLDivElement | null>(null)
const settingsOpen = ref(false)
const llmConfigured = ref<boolean | null>(null)

async function refreshLlmStatus() {
  try {
    const s = await api.getSettings()
    llmConfigured.value = s.llm_configured
  } catch (_) {
    llmConfigured.value = null
  }
}

const activeSession = computed(() =>
  chat.sessions.find((s) => s.id === chat.activeId) || null,
)

async function bootstrap() {
  await auth.fetchMe()
  if (!auth.user) {
    router.replace('/login')
    return
  }
  await Promise.all([chat.loadSessions(), refreshLlmStatus()])

  const targetId = (route.params.id as string) || ''
  if (targetId) {
    if (chat.sessions.some((s) => s.id === targetId)) {
      await chat.selectSession(targetId)
    } else {
      router.replace('/')
    }
  }
}

async function ensureActiveSession(): Promise<string> {
  if (chat.activeId) return chat.activeId
  const id = await chat.createSession()
  router.replace(`/c/${id}`)
  await chat.selectSession(id)
  return id
}

async function onSend(text: string) {
  await ensureActiveSession()
  await chat.send(text)
  scrollToBottom()
}

async function onPickPrompt(text: string) {
  await onSend(text)
}

async function onSelectSession(id: string) {
  router.push(`/c/${id}`)
}

async function onNewSession() {
  const id = await chat.createSession()
  router.push(`/c/${id}`)
}

async function onRename(id: string, title: string) {
  await chat.renameSession(id, title)
}

async function onRemove(id: string) {
  await chat.removeSession(id)
  if (!chat.activeId) router.replace('/')
}

async function onSessionTitleEdit(title: string) {
  if (!chat.activeId) return
  await chat.renameSession(chat.activeId, title)
}

async function logout() {
  await auth.logout()
  router.replace('/login')
}

function scrollToBottom() {
  nextTick(() => {
    if (messagesEl.value) messagesEl.value.scrollTop = messagesEl.value.scrollHeight
  })
}

watch(
  () => route.params.id,
  async (id) => {
    const next = (id as string) || ''
    if (next && next !== chat.activeId) {
      await chat.selectSession(next)
      scrollToBottom()
    } else if (!next) {
      chat.activeId = null
      chat.messages = []
    }
  },
)

watch(
  () => chat.messages.length,
  () => scrollToBottom(),
)

watch(
  () => chat.messages.map((m) => m.content).join('|'),
  () => scrollToBottom(),
  { flush: 'post' },
)

onMounted(bootstrap)
</script>

<template>
  <div class="app-shell">
    <Sidebar
      :sessions="chat.sessions"
      :active-id="chat.activeId"
      :user="auth.user"
      @select="onSelectSession"
      @new="onNewSession"
      @rename="onRename"
      @remove="onRemove"
      @logout="logout"
    />

    <main class="main">
      <header class="main-header">
        <input
          v-if="activeSession"
          class="title-input"
          :value="activeSession.title"
          @change="onSessionTitleEdit(($event.target as HTMLInputElement).value)"
        />
        <div v-else class="title-input" style="opacity: 0.5">未选择对话</div>

        <div style="display: flex; align-items: center; gap: 12px">
          <div v-if="chat.error" style="color: var(--danger); font-size: 12px; max-width: 360px; text-align: right">
            {{ chat.error }}
          </div>
          <button class="header-btn" @click="settingsOpen = true" title="设置">⚙ 设置</button>
        </div>
      </header>

      <div v-if="llmConfigured === false" class="setup-banner">
        <span>⚠ 还没有配置 LLM API Key — AI 回答暂时无法生成。</span>
        <button class="header-btn" @click="settingsOpen = true">前往设置</button>
      </div>

      <div class="messages" ref="messagesEl">
        <div class="center" v-if="chat.messages.length || chat.activeId">
          <MessageList :messages="chat.messages" :streaming="chat.streaming" />
        </div>
        <div v-else class="empty-state">
          <h2>开始一段求职辅导对话</h2>
          <p>你可以先粘贴一份目标岗位 JD，再补充自己的项目和实习经历，AI 会逐步帮你完成简历、能力缺口分析与面试准备。</p>
          <div class="chip-row">
            <button class="chip" @click="onPickPrompt('帮我分析这份 JD 并提炼关键能力要求：\n\n')">
              📄 粘贴 JD 开始分析
            </button>
            <button class="chip" @click="onPickPrompt('我的目标岗位是后端工程师，下面是我的实习与项目经历，请帮我生成一份结构化简历：\n\n')">
              📝 让我生成简历
            </button>
            <button class="chip" @click="onPickPrompt('针对当前岗位，给我 8 道高频面试题与参考答案')">
              🎤 模拟面试问答
            </button>
          </div>
        </div>
      </div>

      <Composer :disabled="chat.streaming" @send="onSend" />
    </main>

    <SettingsModal :open="settingsOpen" @close="settingsOpen = false" @saved="refreshLlmStatus" />
  </div>
</template>

<style scoped>
.header-btn {
  background: var(--panel);
  border: 1px solid var(--border);
  color: var(--text);
  padding: 7px 12px;
  border-radius: 8px;
  font-size: 12.5px;
}
.header-btn:hover { background: var(--panel-2); border-color: var(--accent); }

.setup-banner {
  background: rgba(248, 113, 113, 0.08);
  border-bottom: 1px solid rgba(248, 113, 113, 0.25);
  color: #ffd1d1;
  padding: 10px 24px;
  display: flex; align-items: center; justify-content: space-between;
  font-size: 13px;
}
</style>
