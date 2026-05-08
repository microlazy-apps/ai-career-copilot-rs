<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useAuthStore } from '../stores/auth'
import { api } from '../api'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()

onMounted(async () => {
  await auth.fetchMe()
  if (auth.user) {
    const target = (route.query.from as string) || '/'
    router.replace(target)
  }
})

function login() {
  // Full-page redirect into Lazycat's OIDC flow.
  window.location.href = api.loginUrl()
}
</script>

<template>
  <div class="login-shell">
    <div class="login-card">
      <h1>AI 求职助手</h1>
      <p>使用懒猫微服账号登录，开启你的多轮求职辅导对话。<br />历史会话会自动保存，刷新页面也不会丢失。</p>
      <button class="btn-login" @click="login">使用懒猫账号登录</button>
    </div>
  </div>
</template>
