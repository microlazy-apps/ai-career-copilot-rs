<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { api, type AppSettings } from '../api'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'saved'): void }>()

interface ProviderPreset {
  id: string
  label: string
  baseUrl: string
  model: string
  hint: string
}

const PRESETS: ProviderPreset[] = [
  {
    id: 'deepseek',
    label: 'DeepSeek',
    baseUrl: 'https://api.deepseek.com/v1',
    model: 'deepseek-chat',
    hint: '推荐 · 性价比高',
  },
  {
    id: 'openai',
    label: 'OpenAI',
    baseUrl: 'https://api.openai.com/v1',
    model: 'gpt-4o-mini',
    hint: '官方 GPT 系列',
  },
  {
    id: 'moonshot',
    label: 'Moonshot',
    baseUrl: 'https://api.moonshot.cn/v1',
    model: 'moonshot-v1-8k',
    hint: '国内长上下文',
  },
  {
    id: 'custom',
    label: '自定义',
    baseUrl: '',
    model: '',
    hint: 'vLLM / 私有网关',
  },
]

const settings = ref<AppSettings | null>(null)
const baseUrl = ref('')
const model = ref('')
const apiKey = ref('') // empty = leave existing key untouched
const showKey = ref(false)

const loading = ref(false)
const saving = ref(false)
const testing = ref(false)
const message = ref<{ kind: 'ok' | 'err'; text: string } | null>(null)

const dirty = computed(() => {
  if (!settings.value) return false
  if (baseUrl.value.trim() !== settings.value.llm_base_url) return true
  if (model.value.trim() !== settings.value.llm_model) return true
  if (apiKey.value.length > 0) return true
  return false
})

const status = computed(() => {
  if (!settings.value) return { kind: 'unknown' as const, label: '加载中' }
  return settings.value.llm_configured
    ? { kind: 'ok' as const, label: '已配置' }
    : { kind: 'warn' as const, label: '未配置' }
})

const activePreset = computed(() => {
  const url = baseUrl.value.trim()
  return PRESETS.find((p) => p.id !== 'custom' && p.baseUrl === url)?.id ?? 'custom'
})

async function load() {
  loading.value = true
  try {
    const s = await api.getSettings()
    settings.value = s
    baseUrl.value = s.llm_base_url
    model.value = s.llm_model
    apiKey.value = ''
  } catch (e) {
    message.value = { kind: 'err', text: (e as Error).message }
  } finally {
    loading.value = false
  }
}

function applyPreset(preset: ProviderPreset) {
  if (preset.id === 'custom') return
  baseUrl.value = preset.baseUrl
  model.value = preset.model
}

function buildPayload(): Record<string, string> {
  const payload: Record<string, string> = {}
  if (baseUrl.value && baseUrl.value !== settings.value?.llm_base_url) {
    payload.llm_base_url = baseUrl.value.trim()
  }
  if (model.value && model.value !== settings.value?.llm_model) {
    payload.llm_model = model.value.trim()
  }
  if (apiKey.value) payload.llm_api_key = apiKey.value
  return payload
}

async function save() {
  saving.value = true
  message.value = null
  try {
    const payload = buildPayload()
    if (Object.keys(payload).length === 0) {
      message.value = { kind: 'ok', text: '没有变化需要保存' }
      return
    }
    settings.value = await api.updateSettings(payload)
    apiKey.value = ''
    message.value = { kind: 'ok', text: '设置已保存到本地数据库' }
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
    const payload = buildPayload()
    if (Object.keys(payload).length > 0) {
      settings.value = await api.updateSettings(payload)
      apiKey.value = ''
    }
    const res = await api.testLlm()
    const trimmed = (res.reply || '').trim().slice(0, 80) || '(空回复)'
    message.value = {
      kind: 'ok',
      text: `连通成功 · ${res.model} · 回复："${trimmed}"`,
    }
    emit('saved')
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
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="settings-title">
      <header class="modal-header">
        <div class="title-block">
          <span class="eyebrow">应用设置</span>
          <h2 id="settings-title">LLM 服务配置</h2>
        </div>
        <div class="header-right">
          <span class="status-pill" :data-kind="status.kind">
            <span class="dot" />
            {{ status.label }}
          </span>
          <button class="icon-btn" @click="emit('close')" aria-label="关闭">✕</button>
        </div>
      </header>

      <p class="lead">
        所有 LLM 凭据都保存在<strong>本地 SQLite</strong> ，不会回写到懒猫微服元数据。
        懒猫安装时无需任何参数 — 你在这里改完即时生效。
      </p>

      <section v-if="loading" class="loading">读取中…</section>

      <form v-else @submit.prevent="save" class="form">
        <!-- Provider presets -->
        <div class="card">
          <div class="card-head">
            <span class="card-title">服务提供方</span>
            <span class="card-sub">选一个常见提供方一键填入，或选「自定义」</span>
          </div>
          <div class="preset-row">
            <button
              v-for="p in PRESETS"
              :key="p.id"
              type="button"
              class="preset"
              :class="{ active: activePreset === p.id }"
              :disabled="p.id === 'custom'"
              @click="applyPreset(p)"
            >
              <span class="preset-label">{{ p.label }}</span>
              <span class="preset-hint">{{ p.hint }}</span>
            </button>
          </div>
        </div>

        <!-- Endpoint -->
        <div class="card">
          <div class="card-head">
            <span class="card-title">服务端点</span>
            <span class="card-sub">OpenAI 兼容协议；通常以 <code>/v1</code> 结尾</span>
          </div>
          <label class="field">
            <span class="field-label">API Base URL</span>
            <input v-model="baseUrl" placeholder="https://api.deepseek.com/v1" spellcheck="false" />
          </label>
          <label class="field">
            <span class="field-label">模型名称</span>
            <input v-model="model" placeholder="deepseek-chat / gpt-4o-mini / ..." spellcheck="false" />
          </label>
        </div>

        <!-- Credentials -->
        <div class="card">
          <div class="card-head">
            <span class="card-title">访问凭据</span>
            <span class="card-sub">仅保存到本地，不会上报；可随时清空</span>
          </div>
          <label class="field">
            <span class="field-label">
              API Key
              <em v-if="settings?.llm_configured" class="badge ok">
                已保存 {{ settings.llm_api_key_hint }}
              </em>
              <em v-else class="badge warn">尚未填写</em>
            </span>
            <div class="key-row">
              <input
                :type="showKey ? 'text' : 'password'"
                v-model="apiKey"
                autocomplete="off"
                spellcheck="false"
                :placeholder="settings?.llm_configured ? '留空保持原值，输入新值则覆盖' : '粘贴你的 API Key（sk-... / dsk-... / ...）'"
              />
              <button
                type="button"
                class="icon-btn key-eye"
                :title="showKey ? '隐藏' : '显示'"
                @click="showKey = !showKey"
              >
                {{ showKey ? '🙈' : '👁' }}
              </button>
            </div>
          </label>
        </div>

        <Transition name="fade">
          <div v-if="message" :class="['notice', message.kind]" role="status">
            <span class="notice-icon">{{ message.kind === 'ok' ? '✓' : '⚠' }}</span>
            <span class="notice-text">{{ message.text }}</span>
          </div>
        </Transition>

        <footer class="actions">
          <p v-if="settings" class="updated">
            上次更新：<time>{{ settings.updated_at }}</time>
          </p>
          <div class="actions-right">
            <button
              type="button"
              class="btn-ghost"
              :disabled="testing || saving || (!settings?.llm_configured && !apiKey)"
              @click="test"
            >
              <span class="btn-icon">⚡</span>
              {{ testing ? '测试中…' : '测试连通' }}
            </button>
            <button type="submit" class="btn-primary" :disabled="saving || (!dirty)">
              {{ saving ? '保存中…' : dirty ? '保存设置' : '已是最新' }}
            </button>
          </div>
        </footer>
      </form>
    </div>
  </div>
</template>

<style scoped>
.modal-backdrop {
  position: fixed; inset: 0;
  background: rgba(7, 11, 18, 0.72);
  backdrop-filter: blur(8px);
  display: flex; align-items: center; justify-content: center;
  z-index: 50;
  padding: 24px;
  animation: fadeBg 0.18s ease-out;
}
@keyframes fadeBg { from { opacity: 0; } to { opacity: 1; } }

.modal {
  background:
    radial-gradient(1200px 200px at -10% -20%, rgba(124, 140, 255, 0.18), transparent 60%),
    radial-gradient(900px 200px at 110% -10%, rgba(74, 222, 128, 0.10), transparent 70%),
    var(--panel);
  border: 1px solid var(--border);
  border-radius: 16px;
  padding: 24px 26px 22px;
  width: 100%;
  max-width: 600px;
  max-height: calc(100vh - 48px);
  overflow-y: auto;
  box-shadow: 0 30px 80px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255,255,255,0.02) inset;
  animation: slideIn 0.2s cubic-bezier(0.2, 0.7, 0.3, 1);
}
@keyframes slideIn {
  from { opacity: 0; transform: translateY(8px) scale(0.985); }
  to   { opacity: 1; transform: translateY(0) scale(1); }
}

.modal-header {
  display: flex; align-items: flex-start; justify-content: space-between;
  gap: 16px;
  margin-bottom: 6px;
}
.title-block { display: flex; flex-direction: column; gap: 2px; }
.eyebrow {
  font-size: 11px; letter-spacing: 0.14em; text-transform: uppercase;
  color: var(--text-dim);
}
.modal-header h2 {
  margin: 0;
  font-size: 18px;
  letter-spacing: 0.01em;
  font-weight: 600;
  background: linear-gradient(90deg, #e6e9ef, #99a3b3);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}
.header-right { display: flex; align-items: center; gap: 10px; }

.status-pill {
  display: inline-flex; align-items: center; gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  font-size: 11.5px;
  border: 1px solid var(--border);
  background: var(--panel-2);
  color: var(--text-dim);
  white-space: nowrap;
}
.status-pill .dot {
  width: 6px; height: 6px; border-radius: 50%;
  background: var(--text-dim);
  box-shadow: 0 0 0 0 currentColor;
}
.status-pill[data-kind="ok"] {
  color: #6ee7a8; border-color: rgba(74, 222, 128, 0.35);
  background: rgba(74, 222, 128, 0.08);
}
.status-pill[data-kind="ok"] .dot {
  background: #4ade80;
  animation: pulse 2.4s ease-in-out infinite;
}
.status-pill[data-kind="warn"] {
  color: #fcd34d; border-color: rgba(252, 211, 77, 0.32);
  background: rgba(252, 211, 77, 0.08);
}
.status-pill[data-kind="warn"] .dot { background: #facc15; }
@keyframes pulse {
  0%, 100% { box-shadow: 0 0 0 0 rgba(74, 222, 128, 0.6); }
  50% { box-shadow: 0 0 0 6px rgba(74, 222, 128, 0); }
}

.lead {
  color: var(--text-dim);
  font-size: 12.5px;
  line-height: 1.6;
  margin: 6px 0 18px;
}
.lead strong { color: var(--text); font-weight: 500; }

.loading { padding: 40px 0; text-align: center; color: var(--text-dim); }

.form { display: flex; flex-direction: column; gap: 14px; }

.card {
  background: rgba(28, 34, 48, 0.55);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 14px 16px 16px;
  display: flex; flex-direction: column; gap: 12px;
}
.card-head { display: flex; flex-direction: column; gap: 2px; }
.card-title { font-size: 13px; font-weight: 600; color: var(--text); letter-spacing: 0.01em; }
.card-sub { font-size: 11.5px; color: var(--text-dim); }
.card-sub code {
  background: var(--bg);
  padding: 0 5px;
  border-radius: 4px;
  font-size: 11px;
  border: 1px solid var(--border);
}

.preset-row {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
}
.preset {
  display: flex; flex-direction: column; gap: 2px; align-items: flex-start;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 8px 10px;
  color: var(--text-dim);
  text-align: left;
  transition: border-color 0.15s ease, transform 0.1s ease, color 0.15s ease;
}
.preset:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--text);
  transform: translateY(-1px);
}
.preset:disabled { opacity: 0.55; cursor: not-allowed; }
.preset.active {
  border-color: var(--accent);
  background: rgba(124, 140, 255, 0.08);
  color: var(--text);
  box-shadow: 0 0 0 1px var(--accent) inset;
}
.preset-label { font-size: 12.5px; font-weight: 600; }
.preset-hint { font-size: 10.5px; color: var(--text-dim); }
.preset.active .preset-hint { color: rgba(230, 233, 239, 0.7); }

.field { display: flex; flex-direction: column; gap: 6px; font-size: 13px; }
.field-label {
  color: var(--text-dim);
  display: flex; align-items: center; gap: 8px;
  font-size: 12px;
}
.field input {
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 10px 12px;
  color: var(--text);
  font: inherit;
  width: 100%;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}
.field input:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px rgba(124, 140, 255, 0.18);
}
.field input::placeholder { color: #5b6678; }

.badge {
  font-style: normal;
  font-size: 10.5px;
  font-weight: 500;
  padding: 2px 7px;
  border-radius: 999px;
  letter-spacing: 0.01em;
}
.badge.ok {
  background: rgba(74, 222, 128, 0.12);
  color: #86efac;
  border: 1px solid rgba(74, 222, 128, 0.28);
}
.badge.warn {
  background: rgba(252, 211, 77, 0.10);
  color: #facc15;
  border: 1px solid rgba(252, 211, 77, 0.28);
}

.key-row { display: flex; gap: 8px; align-items: center; }
.key-row input { flex: 1; font-family: ui-monospace, 'JetBrains Mono', Menlo, monospace; font-size: 12.5px; letter-spacing: 0.02em; }
.icon-btn.key-eye {
  width: 38px; height: 38px;
  border: 1px solid var(--border);
  background: var(--bg);
}

.notice {
  display: flex; gap: 10px; align-items: flex-start;
  padding: 10px 14px;
  border-radius: 10px;
  font-size: 12.5px;
  border: 1px solid var(--border);
  line-height: 1.5;
}
.notice .notice-icon {
  flex-shrink: 0;
  width: 20px; height: 20px;
  display: inline-flex; align-items: center; justify-content: center;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 700;
}
.notice.ok {
  background: rgba(74, 222, 128, 0.07);
  border-color: rgba(74, 222, 128, 0.30);
  color: #c8f5dd;
}
.notice.ok .notice-icon { background: #4ade80; color: #052e16; }
.notice.err {
  background: rgba(248, 113, 113, 0.07);
  border-color: rgba(248, 113, 113, 0.32);
  color: #ffd1d1;
}
.notice.err .notice-icon { background: #f87171; color: #4a0c0c; }

.fade-enter-active, .fade-leave-active { transition: opacity 0.18s ease, transform 0.18s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; transform: translateY(-2px); }

.actions {
  display: flex; align-items: center; justify-content: space-between;
  gap: 14px;
  padding-top: 4px;
  border-top: 1px dashed var(--border);
  margin-top: 4px;
  padding-top: 14px;
}
.updated { font-size: 11px; color: var(--text-dim); margin: 0; }
.actions-right { display: flex; gap: 10px; }

.btn-ghost {
  background: var(--bg);
  border: 1px solid var(--border);
  color: var(--text);
  padding: 9px 16px;
  border-radius: 8px;
  display: inline-flex; align-items: center; gap: 6px;
  transition: border-color 0.15s ease, background 0.15s ease;
}
.btn-ghost:hover:not(:disabled) { border-color: var(--accent); background: var(--panel-2); }
.btn-ghost:disabled { opacity: 0.45; cursor: not-allowed; }
.btn-ghost .btn-icon { font-size: 12px; opacity: 0.85; }

.btn-primary {
  background: linear-gradient(90deg, #4f5ee8, #7c8cff);
  color: white;
  border: 0;
  padding: 9px 18px;
  border-radius: 8px;
  font-weight: 500;
  letter-spacing: 0.01em;
  transition: transform 0.1s ease, box-shadow 0.15s ease;
  box-shadow: 0 6px 20px -6px rgba(124, 140, 255, 0.6);
}
.btn-primary:hover:not(:disabled) { transform: translateY(-1px); box-shadow: 0 10px 24px -8px rgba(124, 140, 255, 0.7); }
.btn-primary:disabled {
  background: var(--panel-2);
  color: var(--text-dim);
  box-shadow: none;
  cursor: not-allowed;
}

@media (max-width: 540px) {
  .preset-row { grid-template-columns: repeat(2, 1fr); }
  .actions { flex-direction: column; align-items: stretch; }
  .actions-right { justify-content: flex-end; }
}
</style>
