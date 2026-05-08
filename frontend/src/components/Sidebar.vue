<script setup lang="ts">
import { computed } from 'vue'
import type { SessionView, User } from '../api'

const props = defineProps<{
  sessions: SessionView[]
  activeId: string | null
  user: User | null
}>()

const emit = defineEmits<{
  (e: 'select', id: string): void
  (e: 'new'): void
  (e: 'rename', id: string, title: string): void
  (e: 'remove', id: string): void
  (e: 'logout'): void
}>()

const displayName = computed(() => props.user?.name || props.user?.email || props.user?.id || '未登录')

function handleRename(s: SessionView) {
  const next = window.prompt('重命名会话', s.title)
  if (next && next.trim() && next !== s.title) emit('rename', s.id, next.trim())
}

function handleRemove(s: SessionView) {
  if (window.confirm(`删除「${s.title}」？删除后无法恢复。`)) emit('remove', s.id)
}

function format(updated: string) {
  const d = new Date(updated)
  return `${d.getMonth() + 1}/${d.getDate()} ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-header">
      <h1>
        <span class="badge">AI</span>
        求职助手
      </h1>
      <button class="btn-new" @click="emit('new')">＋ 新建对话</button>
    </div>

    <div class="sidebar-list">
      <div
        v-for="s in props.sessions"
        :key="s.id"
        :class="['item', { active: s.id === props.activeId }]"
        @click="emit('select', s.id)"
      >
        <span class="title">{{ s.title }}</span>
        <span class="meta">{{ s.message_count }} 条 · {{ format(s.updated_at) }}</span>
        <span class="actions" @click.stop>
          <button class="icon-btn" title="重命名" @click="handleRename(s)">✎</button>
          <button class="icon-btn danger" title="删除" @click="handleRemove(s)">✕</button>
        </span>
      </div>
      <div v-if="!props.sessions.length" style="padding: 20px; color: var(--text-dim); font-size: 12.5px">
        还没有对话记录。点击上方「新建对话」开始。
      </div>
    </div>

    <div class="sidebar-footer">
      <span>{{ displayName }}</span>
      <button class="icon-btn" title="退出登录" @click="emit('logout')">⎋</button>
    </div>
  </aside>
</template>
