import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api, type AuthMethods, type User } from '../api'

const DEFAULT_METHODS: AuthMethods = { oidc: false, email: false }

export const useAuthStore = defineStore('auth', () => {
  const user = ref<User | null>(null)
  const methods = ref<AuthMethods>({ ...DEFAULT_METHODS })
  const checked = ref(false)

  async function fetchMe(): Promise<void> {
    try {
      const res = await api.me()
      user.value = res.authenticated && res.user ? res.user : null
      methods.value = res.methods ?? { ...DEFAULT_METHODS }
    } catch (_) {
      user.value = null
      methods.value = { ...DEFAULT_METHODS }
    } finally {
      checked.value = true
    }
  }

  async function loginWithEmail(email: string, name?: string): Promise<void> {
    const res = await api.emailLogin(email, name)
    user.value = res.user
  }

  async function logout(): Promise<void> {
    await api.logout()
    user.value = null
    checked.value = true
  }

  return { user, methods, checked, fetchMe, loginWithEmail, logout }
})
