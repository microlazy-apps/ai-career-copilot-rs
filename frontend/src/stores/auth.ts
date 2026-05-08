import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api, type User } from '../api'

export const useAuthStore = defineStore('auth', () => {
  const user = ref<User | null>(null)
  const checked = ref(false)

  async function fetchMe(): Promise<void> {
    try {
      const res = await api.me()
      user.value = res.authenticated && res.user ? res.user : null
    } catch (_) {
      user.value = null
    } finally {
      checked.value = true
    }
  }

  async function logout(): Promise<void> {
    await api.logout()
    user.value = null
    checked.value = true
  }

  return { user, checked, fetchMe, logout }
})
