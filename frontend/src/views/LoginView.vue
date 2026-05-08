<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useAuthStore } from '../stores/auth'
import { api } from '../api'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()

const email = ref('')
const submitting = ref(false)
const errorMsg = ref('')

const showOidc = computed(() => auth.methods.oidc)
const showEmail = computed(() => auth.methods.email)
const target = computed(() => (route.query.from as string) || '/')

onMounted(async () => {
  await auth.fetchMe()
  if (auth.user) router.replace(target.value)
})

function loginWithLazycat() {
  window.location.href = api.loginUrl()
}

async function loginWithEmail() {
  errorMsg.value = ''
  const value = email.value.trim()
  if (!value) {
    errorMsg.value = '请输入邮箱地址'
    return
  }
  submitting.value = true
  try {
    await auth.loginWithEmail(value)
    router.replace(target.value)
  } catch (e: unknown) {
    errorMsg.value = e instanceof Error ? e.message : '登录失败'
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <div class="login-shell">
    <div class="login-card">
      <h1>AI 求职助手</h1>
      <p>登录后开启多轮求职辅导对话，<br />历史会话会自动保存，刷新页面也不会丢失。</p>

      <button
        v-if="showOidc"
        class="btn-login"
        @click="loginWithLazycat"
      >
        使用懒猫账号登录
      </button>

      <div
        v-if="showOidc && showEmail"
        class="login-divider"
        role="separator"
      >
        或
      </div>

      <form
        v-if="showEmail"
        class="login-email"
        @submit.prevent="loginWithEmail"
      >
        <label class="login-label" for="login-email">邮箱</label>
        <input
          id="login-email"
          v-model="email"
          type="email"
          autocomplete="email"
          placeholder="you@example.com"
          required
          :disabled="submitting"
        />
        <button
          type="submit"
          class="btn-login"
          :disabled="submitting"
        >
          {{ submitting ? '登录中…' : '使用邮箱登录' }}
        </button>
        <p v-if="errorMsg" class="login-error">{{ errorMsg }}</p>
        <p class="login-hint">现阶段不发送验证邮件 — 邮箱即身份。</p>
      </form>

      <p
        v-if="!showOidc && !showEmail"
        class="login-error"
      >
        当前部署既未配置懒猫 OIDC，也未启用邮箱登录。请检查后端配置。
      </p>
    </div>
  </div>
</template>
