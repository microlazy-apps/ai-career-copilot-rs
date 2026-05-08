<script setup lang="ts">
import { ref, watch } from 'vue'
import { api, type AppSettings } from '../api'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'saved'): void }>()

const settings = ref<AppSettings | null>(null)
const baseUrl = ref('')
const model = ref('')
const apiKey = ref('') // empty = leave existing key untouched
const showKey = ref(false)

const loading = ref(false)
const saving = ref(false)
const testing = ref(false)
const message = ref<{ kind: 'ok' | 'err'; text: string } | null>(null)

async function load() {
  loading.value = true
  try {
    const s = await api.getSettings()
    settings.value = s
    baseUrl.value = s.llm_base_url
    model.value = s.llm_model
    apiKey.value = ''
  } finally {
    loading.value = false
  }
}

async function save() {
  saving.value = true
  message.value = null
  try {
    const payload: Record<string, string> = {}
    if (baseUrl.value && baseUrl.value !== settings.value?.llm_base_url) payload.llm_base_url = baseUrl.value
    if (model.value && model.value !== settings.value?.llm_model) payload.llm_model = model.value
    if (apiKey.value) payload.llm_api_key = apiKey.value
    if (Object.keys(payload).length === 0) {
      message.value = { kind: 'ok', text: '没有变化' }
      return
    }
    settings.value = await api.updateSettings(payload)
    apiKey.value = ''
    message.value = { kind: 'ok', text: '已保存' }
    emit('saved')
  } catch (e) {
    message.value = { kind: 'err', text: (e as Error).message }
  } finally {
    saving.value = false
  }
}

async function test() {
  testing.value = true
  message.value = null
  try {
    // Save any pending edits first so the test reflects current form state.
    const payload: Record<string, string> = {}
    if (baseUrl.value && baseUrl.value !== settings.value?.llm_base_url) payload.llm_base_url = baseUrl.value
    if (model.value && model.value !== settings.value?.llm_model) payload.llm_model = model.value
    if (apiKey.value) payload.llm_api_key = apiKey.value
    if (Object.keys(payload).length > 0) {
      settings.value = await api.updateSettings(payload)
      apiKey.value = ''
    }
    const res = await api.testLlm()
    const trimmed = (res.reply || '').trim().slice(0, 80) || '(空)'
    message.value = {
      kind: 'ok',
      text: `连通成功 · ${res.model} · 回复："${trimmed}"`,
    }
  } catch (e) {
    message.value = { kind: 'err', text: (e as Error).message }
  } finally {
    testing.value = false
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      message.value = null
      load()
    }
  },
  { immediate: true },
)
</script>

<template>
  <div v-if="props.open" class="modal-backdrop" @click.self="emit('close')">
    <div class="modal" role="dialog" aria-modal="true">
      <header>
        <h2>设置</h2>
        <button class="icon-btn" @click="emit('close')" aria-label="关闭">✕</button>
      </header>
      <p class="hint">
        在这里配置 OpenAI 兼容的 LLM 服务。所有字段保存到本地 SQLite，
        懒猫安装时<strong>无需</strong>提前填入 API Key。
      </p>

      <section v-if="loading" class="loading">读取中…</section>
      <form v-else @submit.prevent="save" class="form">
        <label class="field">
          <span>API Base URL</span>
          <input v-model="baseUrl" placeholder="https://api.deepseek.com/v1" />
        </label>

        <label class="field">
          <span>模型名称</span>
          <input v-model="model" placeholder="deepseek-chat / gpt-4o-mini / ..." />
        </label>

        <label class="field">
          <span>
            API Key
            <em v-if="settings?.llm_configured" class="muted">
              （已保存：{{ settings.llm_api_key_hint }}，留空保持不变）
            </em>
            <em v-else class="muted danger">（未配置，请填入）</em>
          </span>
          <div class="key-row">
            <input
              :type="showKey ? 'text' : 'password'"
              v-model="apiKey"
              autocomplete="off"
              spellcheck="false"
              placeholder="sk-... / dsk-... / ..."
            />
            <button type="button" class="icon-btn" @click="showKey = !showKey">
              {{ showKey ? '🙈' : '👁' }}
            </button>
          </div>
        </label>

        <div v-if="message" :class="['notice', message.kind]">{{ message.text }}</div>

        <div class="actions">
          <button type="button" class="btn-ghost" :disabled="testing || saving" @click="test">
            {{ testing ? '测试中…' : '测试连通' }}
          </button>
          <button type="submit" class="btn-primary" :disabled="saving">
            {{ saving ? '保存中…' : '保存' }}
          </button>
        </div>

        <p class="muted small" v-if="settings">
          上次更新：{{ settings.updated_at }}
        </p>
      </form>
    </div>
  </div>
</template>

<style scoped>
.modal-backdrop {
  position: fixed; inset: 0;
  background: rgba(7, 11, 18, 0.6);
  backdrop-filter: blur(6px);
  display: flex; align-items: center; justify-content: center;
  z-index: 50;
}
.modal {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 22px;
  width: 100%;
  max-width: 520px;
  box-shadow: 0 24px 64px rgba(0, 0, 0, 0.45);
}
.modal header {
  display: flex; align-items: center; justify-content: space-between;
  margin-bottom: 6px;
}
.modal h2 { margin: 0; font-size: 17px; }
.hint { color: var(--text-dim); font-size: 12.5px; margin: 4px 0 16px; }
.loading { padding: 30px 0; text-align: center; color: var(--text-dim); }
.form { display: flex; flex-direction: column; gap: 14px; }
.field { display: flex; flex-direction: column; gap: 6px; font-size: 13px; }
.field span { color: var(--text-dim); }
.field input {
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 9px 12px;
  color: var(--text);
  font: inherit;
  width: 100%;
}
.field input:focus { outline: none; border-color: var(--accent); }
.muted { color: var(--text-dim); font-style: normal; font-weight: 400; font-size: 12px; margin-left: 6px; }
.muted.danger { color: var(--danger); }
.muted.small { font-size: 11.5px; }
.key-row { display: flex; gap: 8px; align-items: center; }
.key-row input { flex: 1; }
.notice {
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 12.5px;
  border: 1px solid var(--border);
}
.notice.ok { background: rgba(74, 222, 128, 0.08); border-color: #2f5e3f; }
.notice.err { background: rgba(248, 113, 113, 0.08); border-color: #6e2f2f; color: #ffb3b3; }
.actions { display: flex; gap: 10px; justify-content: flex-end; }
.btn-ghost {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--text);
  padding: 9px 16px;
  border-radius: 8px;
}
.btn-ghost:hover { background: var(--panel-2); }
.btn-primary {
  background: linear-gradient(90deg, #4f5ee8, #7c8cff);
  color: white;
  border: 0;
  padding: 9px 18px;
  border-radius: 8px;
  font-weight: 500;
}
.btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
</style>
