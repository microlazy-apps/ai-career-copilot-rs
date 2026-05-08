<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth'
import { useChatStore } from '../stores/chat'
import Sidebar from '../components/Sidebar.vue'
import MessageList from '../components/MessageList.vue'
import Composer from '../components/Composer.vue'

const route = useRoute()
const router = useRouter()
const auth = useAuthStore()
const chat = useChatStore()

const messagesEl = ref<HTMLDivElement | null>(null)

const activeSession = computed(() =>
  chat.sessions.find((s) => s.id === chat.activeId) || null,
)

async function bootstrap() {
  await auth.fetchMe()
  if (!auth.user) {
    router.replace('/login')
    return
  }
  await chat.loadSessions()

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

        <div v-if="chat.error" style="color: var(--danger); font-size: 12px">{{ chat.error }}</div>
      </header>

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
  </div>
</template>
