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
const oidcRedirecting = ref(false)
const errorMsg = ref('')

const useOidc = computed(() => auth.methods.oidc)
const target = computed(() => (route.query.from as string) || '/')

onMounted(async () => {
  await auth.fetchMe()
  if (auth.user) router.replace(target.value)
})

function loginWithLazycat() {
  oidcRedirecting.value = true
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
    <div class="login-aurora" aria-hidden="true">
      <span class="aurora-blob aurora-a"></span>
      <span class="aurora-blob aurora-b"></span>
      <span class="aurora-grid"></span>
    </div>

    <article class="login-card" :data-mode="useOidc ? 'oidc' : 'email'">
      <header class="login-brand">
        <div class="brand-mark">
          <span>AI</span>
        </div>
        <div class="brand-copy">
          <p class="brand-eyebrow">CAREER COPILOT</p>
          <h1>求职助手</h1>
        </div>
      </header>

      <p class="login-lede">
        多轮对话辅导岗位投递、简历润色与面试准备。<br />
        会话历史自动持久化，刷新页面也不会丢。
      </p>

      <ul class="login-feats" aria-label="主要能力">
        <li><span class="dot dot-blue"></span>JD 拆解 · 简历定向润色</li>
        <li><span class="dot dot-green"></span>历史会话持久化</li>
        <li><span class="dot dot-amber"></span>简历附件即上下文</li>
      </ul>

      <section v-if="useOidc" class="login-method">
        <div class="method-tag method-oidc">
          <span class="tag-dot"></span>
          已检测到懒猫微服 OIDC
        </div>
        <button
          type="button"
          class="btn-login btn-primary"
          :disabled="oidcRedirecting"
          @click="loginWithLazycat"
        >
          <svg
            class="btn-icon"
            viewBox="0 0 24 24"
            width="18"
            height="18"
            aria-hidden="true"
          >
            <path
              fill="currentColor"
              d="M12 2 4 6v6c0 5 3.4 9.5 8 10 4.6-.5 8-5 8-10V6l-8-4Zm0 4.6 4 2v3.4c0 3.5-2.4 6.7-4 7.3-1.6-.6-4-3.8-4-7.3V8.6l4-2Z"
            />
          </svg>
          {{ oidcRedirecting ? '正在跳转懒猫账号…' : '使用懒猫账号登录' }}
        </button>
        <p class="method-hint">点击按钮跳转懒猫账号体系，授权后自动回跳。</p>
      </section>

      <section v-else class="login-method">
        <div class="method-tag method-email">
          <span class="tag-dot"></span>
          普通环境 · 邮箱登录
        </div>
        <form class="login-email" @submit.prevent="loginWithEmail">
          <label class="field">
            <span class="field-label">邮箱</span>
            <input
              v-model="email"
              type="email"
              autocomplete="email"
              placeholder="you@example.com"
              required
              :disabled="submitting"
            />
          </label>
          <button
            type="submit"
            class="btn-login btn-primary"
            :disabled="submitting"
          >
            <span v-if="submitting" class="spinner" aria-hidden="true"></span>
            {{ submitting ? '登录中…' : '使用邮箱登录' }}
          </button>
          <p v-if="errorMsg" class="method-error" role="alert">
            <span class="error-icon">!</span>{{ errorMsg }}
          </p>
          <p class="method-hint">
            现阶段不发送验证邮件，邮箱即身份。后续上线 OIDC 后可无缝迁移。
          </p>
        </form>
      </section>

      <footer class="login-footer">
        <a href="https://github.com/microlazy-apps/ai-career-copilot-rs" target="_blank" rel="noreferrer">
          <span class="gh-icon" aria-hidden="true">★</span>
          microlazy-apps/ai-career-copilot-rs
        </a>
      </footer>
    </article>
  </div>
</template>

<style scoped>
.login-shell {
  position: relative;
  min-height: 100vh;
  display: grid;
  place-items: center;
  padding: 32px 16px;
  background:
    radial-gradient(1100px 600px at 75% -10%, rgba(124, 140, 255, 0.18), transparent 60%),
    radial-gradient(900px 500px at 10% 110%, rgba(74, 222, 128, 0.10), transparent 60%),
    var(--bg);
  overflow: hidden;
}

.login-aurora { position: absolute; inset: 0; pointer-events: none; }
.aurora-blob {
  position: absolute;
  border-radius: 50%;
  filter: blur(80px);
  opacity: 0.45;
}
.aurora-a {
  width: 460px; height: 460px;
  top: -160px; right: -120px;
  background: radial-gradient(circle, #4f5ee8, transparent 70%);
}
.aurora-b {
  width: 360px; height: 360px;
  bottom: -120px; left: -80px;
  background: radial-gradient(circle, #4ade80, transparent 70%);
  opacity: 0.28;
}
.aurora-grid {
  position: absolute;
  inset: 0;
  background-image:
    linear-gradient(rgba(255, 255, 255, 0.025) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255, 255, 255, 0.025) 1px, transparent 1px);
  background-size: 32px 32px;
  mask-image: radial-gradient(ellipse at center, black 0%, transparent 75%);
}

.login-card {
  position: relative;
  width: min(420px, 100%);
  background:
    linear-gradient(180deg, rgba(28, 34, 48, 0.92), rgba(22, 26, 34, 0.94));
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 20px;
  padding: 36px 32px 28px;
  box-shadow:
    0 30px 80px -20px rgba(0, 0, 0, 0.6),
    0 0 0 1px rgba(124, 140, 255, 0.08) inset;
  backdrop-filter: blur(8px);
}
.login-card::before {
  content: '';
  position: absolute;
  inset: -1px;
  border-radius: inherit;
  padding: 1px;
  background: linear-gradient(135deg, rgba(124, 140, 255, 0.55), transparent 40%, rgba(74, 222, 128, 0.35));
  -webkit-mask:
    linear-gradient(#fff 0 0) content-box,
    linear-gradient(#fff 0 0);
  -webkit-mask-composite: xor;
          mask-composite: exclude;
  pointer-events: none;
  opacity: 0.6;
}

.login-brand {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 18px;
}
.brand-mark {
  width: 48px; height: 48px;
  border-radius: 14px;
  background: linear-gradient(135deg, #4f5ee8, #7c8cff 55%, #4ade80);
  display: flex; align-items: center; justify-content: center;
  color: white;
  font-weight: 700;
  font-size: 17px;
  letter-spacing: 0.04em;
  box-shadow: 0 12px 28px -10px rgba(124, 140, 255, 0.6);
}
.brand-mark span { transform: translateY(-1px); }
.brand-copy { display: flex; flex-direction: column; gap: 2px; }
.brand-eyebrow {
  margin: 0;
  font-size: 11px;
  letter-spacing: 0.2em;
  color: var(--text-dim);
  font-weight: 500;
}
.login-card h1 {
  margin: 0;
  font-size: 24px;
  font-weight: 600;
  letter-spacing: -0.01em;
  color: var(--text);
}

.login-lede {
  margin: 0 0 18px;
  color: var(--text-dim);
  font-size: 13.5px;
  line-height: 1.6;
}

.login-feats {
  list-style: none;
  margin: 0 0 24px;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: 12.5px;
  color: var(--text-dim);
}
.login-feats li { display: flex; align-items: center; gap: 8px; }
.dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
.dot-blue { background: #7c8cff; box-shadow: 0 0 0 3px rgba(124, 140, 255, 0.18); }
.dot-green { background: #4ade80; box-shadow: 0 0 0 3px rgba(74, 222, 128, 0.18); }
.dot-amber { background: #f5b454; box-shadow: 0 0 0 3px rgba(245, 180, 84, 0.18); }

.login-method {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-top: 18px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}

.method-tag {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  font-size: 11.5px;
  letter-spacing: 0.02em;
  border: 1px solid;
}
.method-tag .tag-dot {
  width: 6px; height: 6px; border-radius: 50%;
}
.method-oidc {
  color: #b6c1ff;
  border-color: rgba(124, 140, 255, 0.35);
  background: rgba(124, 140, 255, 0.08);
}
.method-oidc .tag-dot { background: #7c8cff; box-shadow: 0 0 0 3px rgba(124, 140, 255, 0.25); }
.method-email {
  color: #b3eccb;
  border-color: rgba(74, 222, 128, 0.32);
  background: rgba(74, 222, 128, 0.08);
}
.method-email .tag-dot { background: #4ade80; box-shadow: 0 0 0 3px rgba(74, 222, 128, 0.25); }

.btn-login {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
  border: 0;
  border-radius: 12px;
  padding: 12px 18px;
  font-weight: 600;
  font-size: 14px;
  letter-spacing: 0.01em;
  transition: transform 0.12s ease, box-shadow 0.18s ease, opacity 0.12s ease;
}
.btn-primary {
  background: linear-gradient(135deg, #4f5ee8 0%, #7c8cff 100%);
  color: white;
  box-shadow:
    0 14px 28px -12px rgba(124, 140, 255, 0.6),
    inset 0 1px 0 rgba(255, 255, 255, 0.18);
}
.btn-primary:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow:
    0 18px 36px -12px rgba(124, 140, 255, 0.7),
    inset 0 1px 0 rgba(255, 255, 255, 0.22);
}
.btn-primary:active:not(:disabled) { transform: translateY(0); }
.btn-primary:disabled {
  opacity: 0.65;
  cursor: not-allowed;
}
.btn-icon { flex-shrink: 0; opacity: 0.95; }

.method-hint {
  margin: 0;
  font-size: 12px;
  color: var(--text-dim);
  text-align: left;
}

.login-email {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  text-align: left;
}
.field-label {
  font-size: 11px;
  letter-spacing: 0.12em;
  color: var(--text-dim);
  text-transform: uppercase;
  font-weight: 500;
}
.field input {
  background: rgba(14, 16, 20, 0.7);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: var(--text);
  border-radius: 10px;
  padding: 12px 14px;
  font: inherit;
  outline: none;
  transition: border-color 0.15s ease, box-shadow 0.15s ease, background 0.15s ease;
}
.field input::placeholder { color: rgba(153, 163, 179, 0.5); }
.field input:hover:not(:disabled) {
  border-color: rgba(255, 255, 255, 0.14);
}
.field input:focus {
  border-color: var(--accent);
  background: rgba(14, 16, 20, 0.9);
  box-shadow: 0 0 0 4px rgba(124, 140, 255, 0.18);
}
.field input:disabled { opacity: 0.6; cursor: not-allowed; }

.method-error {
  margin: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12.5px;
  color: var(--danger);
  background: rgba(248, 113, 113, 0.10);
  border: 1px solid rgba(248, 113, 113, 0.3);
  padding: 8px 12px;
  border-radius: 10px;
  text-align: left;
}
.error-icon {
  width: 18px; height: 18px;
  border-radius: 50%;
  background: var(--danger);
  color: #1a0808;
  display: inline-flex; align-items: center; justify-content: center;
  font-size: 12px; font-weight: 700;
  flex-shrink: 0;
}

.spinner {
  width: 14px; height: 14px;
  border-radius: 50%;
  border: 2px solid rgba(255, 255, 255, 0.35);
  border-top-color: white;
  animation: spin 0.7s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.login-footer {
  margin-top: 22px;
  padding-top: 16px;
  border-top: 1px solid rgba(255, 255, 255, 0.05);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
}
.login-footer a {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
  transition: color 0.12s ease;
}
.login-footer a:hover { color: var(--text); }
.gh-icon {
  display: inline-flex;
  width: 16px; height: 16px;
  align-items: center; justify-content: center;
  border-radius: 4px;
  background: rgba(124, 140, 255, 0.18);
  color: #b6c1ff;
  font-size: 10px;
}

@media (max-width: 480px) {
  .login-card { padding: 28px 22px 22px; border-radius: 18px; }
  .login-card h1 { font-size: 22px; }
  .login-feats { display: none; }
}
</style>
