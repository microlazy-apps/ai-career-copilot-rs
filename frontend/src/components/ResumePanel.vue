<script setup lang="ts">
import { computed, ref } from 'vue'
import { api, type AttachmentSummary } from '../api'

const props = defineProps<{
  attachments: AttachmentSummary[]
  uploading: boolean
  disabled?: boolean
}>()

const emit = defineEmits<{
  (e: 'upload', file: File): void
  (e: 'remove', id: string): void
}>()

const fileInput = ref<HTMLInputElement | null>(null)
const dragOver = ref(false)
const previewId = ref<string | null>(null)
const previewLoading = ref(false)
const previewText = ref('')
const previewError = ref<string | null>(null)

const ACCEPT = '.pdf,.docx,.txt,.md,.markdown'

const remainingSlots = computed(() => Math.max(0, 5 - props.attachments.length))
const canUpload = computed(() => !props.disabled && !props.uploading && remainingSlots.value > 0)

function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`
}

function statusLabel(att: AttachmentSummary): { text: string; tone: 'ok' | 'warn' | 'err' } {
  if (att.extract_status === 'failed') return { text: '解析失败', tone: 'err' }
  if (att.extract_status === 'partial') return { text: '已截断', tone: 'warn' }
  if (att.text_chars === 0) return { text: '无文本', tone: 'warn' }
  return { text: `${att.text_chars.toLocaleString()} 字`, tone: 'ok' }
}

function pickFile() {
  fileInput.value?.click()
}

function onPicked(e: Event) {
  const target = e.target as HTMLInputElement
  const file = target.files?.[0]
  if (file) {
    emit('upload', file)
  }
  target.value = ''
}

function onDrop(e: DragEvent) {
  e.preventDefault()
  dragOver.value = false
  if (!canUpload.value) return
  const file = e.dataTransfer?.files?.[0]
  if (file) emit('upload', file)
}

async function togglePreview(att: AttachmentSummary) {
  if (previewId.value === att.id) {
    previewId.value = null
    return
  }
  previewId.value = att.id
  previewLoading.value = true
  previewText.value = ''
  previewError.value = null
  try {
    const detail = await api.getAttachment(att.session_id, att.id)
    previewText.value = detail.text_preview || '（未提取到文本）'
  } catch (e) {
    previewError.value = (e as Error).message
  } finally {
    previewLoading.value = false
  }
}
</script>

<template>
  <section
    class="resume-panel"
    :class="{ 'is-dragover': dragOver, 'is-empty': attachments.length === 0 }"
    @dragover.prevent="canUpload && (dragOver = true)"
    @dragleave="dragOver = false"
    @drop="onDrop"
  >
    <header class="head">
      <div class="head-text">
        <span class="title">📎 简历附件</span>
        <span class="hint" v-if="attachments.length === 0">
          上传 PDF / DOCX / TXT / Markdown，AI 会自动作为事实依据加入对话
        </span>
        <span class="hint" v-else>
          已上传 {{ attachments.length }} / 5 ·
          剩 {{ remainingSlots }} 个名额 ·
          AI 在每轮对话中会自动读取这些文本
        </span>
      </div>
      <div class="head-actions">
        <button
          type="button"
          class="upload-btn"
          :disabled="!canUpload"
          @click="pickFile"
          :title="remainingSlots === 0 ? '已达 5 个上限' : '上传简历'"
        >
          <span v-if="uploading">⌛ 解析中…</span>
          <span v-else-if="remainingSlots === 0">已达上限</span>
          <span v-else>＋ 上传</span>
        </button>
        <input
          ref="fileInput"
          type="file"
          :accept="ACCEPT"
          class="hidden-input"
          @change="onPicked"
        />
      </div>
    </header>

    <ul v-if="attachments.length" class="list">
      <li v-for="att in attachments" :key="att.id" class="row">
        <div class="row-main">
          <button
            type="button"
            class="row-toggle"
            :aria-expanded="previewId === att.id"
            @click="togglePreview(att)"
          >
            <span class="row-icon">{{ att.kind === 'PDF' ? '📕' : att.kind === 'DOCX' ? '📘' : att.kind === 'Markdown' ? '📝' : '📄' }}</span>
            <span class="row-name" :title="att.filename">{{ att.filename }}</span>
            <span class="row-meta">
              <span class="row-kind">{{ att.kind }}</span>
              <span class="row-size">{{ fmtSize(att.size_bytes) }}</span>
              <span class="row-status" :data-tone="statusLabel(att).tone">
                {{ statusLabel(att).text }}
              </span>
            </span>
            <span class="row-caret">{{ previewId === att.id ? '▾' : '▸' }}</span>
          </button>
          <div class="row-actions">
            <a
              class="row-btn"
              :href="api.attachmentDownloadUrl(att.session_id, att.id)"
              target="_blank"
              rel="noopener"
              title="下载原文件"
            >⬇</a>
            <button
              class="row-btn danger"
              type="button"
              title="删除"
              @click="emit('remove', att.id)"
            >🗑</button>
          </div>
        </div>
        <div v-if="previewId === att.id" class="preview">
          <div v-if="previewLoading" class="preview-state">读取中…</div>
          <div v-else-if="previewError" class="preview-state err">{{ previewError }}</div>
          <pre v-else class="preview-text">{{ previewText }}</pre>
          <div v-if="att.extract_error" class="preview-warn">
            ⚠ {{ att.extract_error }}
          </div>
        </div>
      </li>
    </ul>

    <button v-else type="button" class="empty" :disabled="!canUpload" @click="pickFile">
      <span class="empty-icon">📎</span>
      <span class="empty-title">点击或拖入简历文件</span>
      <span class="empty-sub">支持 PDF / DOCX / TXT / Markdown · 最大 10 MB · 单会话至多 5 份</span>
    </button>
  </section>
</template>

<style scoped>
.resume-panel {
  border: 1px solid var(--border);
  background: var(--panel);
  border-radius: 12px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  transition: border-color 0.15s ease, background 0.15s ease;
}
.resume-panel.is-dragover {
  border-color: var(--accent);
  background:
    linear-gradient(0deg, rgba(124, 140, 255, 0.10), rgba(124, 140, 255, 0.06)),
    var(--panel);
}

.head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.head-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.title { font-size: 13px; font-weight: 600; color: var(--text); }
.hint { font-size: 11.5px; color: var(--text-dim); }

.head-actions { display: flex; gap: 8px; }
.upload-btn {
  background: linear-gradient(90deg, #4f5ee8, #7c8cff);
  color: white;
  border: 0;
  padding: 7px 14px;
  border-radius: 8px;
  font-size: 12.5px;
  font-weight: 500;
  transition: transform 0.1s ease, box-shadow 0.15s ease, opacity 0.15s ease;
  box-shadow: 0 6px 16px -8px rgba(124, 140, 255, 0.7);
}
.upload-btn:hover:not(:disabled) { transform: translateY(-1px); }
.upload-btn:disabled {
  background: var(--panel-2);
  color: var(--text-dim);
  box-shadow: none;
  cursor: not-allowed;
}
.hidden-input { display: none; }

.list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.row {
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg);
  overflow: hidden;
}
.row-main {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 6px 0 0;
}
.row-toggle {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  background: transparent;
  border: 0;
  color: var(--text);
  text-align: left;
  font: inherit;
  min-width: 0;
}
.row-toggle:hover { background: rgba(255, 255, 255, 0.02); }
.row-icon { font-size: 16px; flex-shrink: 0; }
.row-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}
.row-meta {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: var(--text-dim);
  font-size: 11px;
  flex-shrink: 0;
}
.row-kind {
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 1px 6px;
  font-weight: 500;
  letter-spacing: 0.02em;
}
.row-status[data-tone="ok"] { color: #6ee7a8; }
.row-status[data-tone="warn"] { color: #facc15; }
.row-status[data-tone="err"] { color: #f87171; }
.row-caret { font-size: 11px; color: var(--text-dim); flex-shrink: 0; }

.row-actions { display: flex; gap: 4px; flex-shrink: 0; padding-right: 4px; }
.row-btn {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--text-dim);
  width: 28px; height: 28px;
  border-radius: 6px;
  text-decoration: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  transition: border-color 0.15s ease, color 0.15s ease;
}
.row-btn:hover { color: var(--text); border-color: var(--accent); }
.row-btn.danger:hover { color: #f87171; border-color: rgba(248, 113, 113, 0.5); }

.preview {
  border-top: 1px dashed var(--border);
  padding: 10px 12px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  background: rgba(0, 0, 0, 0.18);
}
.preview-state { color: var(--text-dim); font-size: 12px; }
.preview-state.err { color: #f87171; }
.preview-text {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 220px;
  overflow-y: auto;
  font-family: ui-monospace, "JetBrains Mono", Menlo, monospace;
  font-size: 11.5px;
  line-height: 1.6;
  color: var(--text-dim);
}
.preview-warn {
  font-size: 11px;
  color: #facc15;
}

.empty {
  width: 100%;
  border: 1px dashed var(--border);
  background: transparent;
  border-radius: 10px;
  padding: 22px 16px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  color: var(--text-dim);
  font: inherit;
  cursor: pointer;
  transition: border-color 0.15s ease, color 0.15s ease;
}
.empty:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--text);
}
.empty:disabled { opacity: 0.55; cursor: not-allowed; }
.empty-icon { font-size: 26px; }
.empty-title { font-size: 13px; font-weight: 500; }
.empty-sub { font-size: 11px; color: var(--text-dim); }
</style>
