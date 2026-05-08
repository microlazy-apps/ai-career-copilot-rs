<script setup lang="ts">
import { computed } from 'vue'
import { marked } from 'marked'
import type { Message } from '../api'

const props = defineProps<{
  messages: Message[]
  streaming: boolean
}>()

marked.setOptions({ breaks: true, gfm: true })

function render(content: string) {
  if (!content) return ''
  return marked.parse(content) as string
}

const list = computed(() => props.messages.filter((m) => m.role !== 'system'))
</script>

<template>
  <template v-for="(m, idx) in list" :key="m.id">
    <div :class="['bubble', m.role]">
      <div class="avatar">{{ m.role === 'user' ? '我' : 'AI' }}</div>
      <div class="body">
        <div v-if="m.role === 'assistant'" v-html="render(m.content)"></div>
        <div v-else style="white-space: pre-wrap">{{ m.content }}</div>
        <div
          v-if="props.streaming && idx === list.length - 1 && m.role === 'assistant' && !m.content"
          style="color: var(--text-dim); font-size: 12.5px"
        >
          AI 正在思考...
        </div>
      </div>
    </div>
  </template>
</template>
