<script setup lang="ts">
import { ref } from 'vue'

const props = defineProps<{ disabled: boolean }>()
const emit = defineEmits<{ (e: 'send', text: string): void }>()

const text = ref('')

function send() {
  const v = text.value.trim()
  if (!v || props.disabled) return
  emit('send', v)
  text.value = ''
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
    e.preventDefault()
    send()
  }
}
</script>

<template>
  <footer class="composer">
    <div class="center">
      <textarea
        v-model="text"
        :disabled="props.disabled"
        placeholder="输入消息，Enter 发送，Shift+Enter 换行"
        @keydown="handleKeydown"
      ></textarea>
      <div class="row">
        <span class="hint">支持 Markdown · 多轮对话历史会自动保存到本地数据库</span>
        <button class="send" :disabled="props.disabled || !text.trim()" @click="send">
          {{ props.disabled ? '生成中...' : '发送' }}
        </button>
      </div>
    </div>
  </footer>
</template>
