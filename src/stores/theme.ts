import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

type Theme = 'light' | 'dark'
const STORAGE_KEY = 'examops-theme'

export const useThemeStore = defineStore('theme', () => {
  const theme = ref<Theme>('light')
  const isDark = computed(() => theme.value === 'dark')

  function apply(value: Theme) {
    theme.value = value
    document.documentElement.dataset.theme = value
  }

  function initialize() {
    let saved: Theme = 'light'
    try {
      if (localStorage.getItem(STORAGE_KEY) === 'dark') saved = 'dark'
    } catch { /* Keep the default when storage is unavailable. */ }
    apply(saved)
  }

  function toggle() {
    apply(isDark.value ? 'light' : 'dark')
    try {
      localStorage.setItem(STORAGE_KEY, theme.value)
    } catch { /* Switching still works for the current session. */ }
  }

  return { theme, isDark, initialize, toggle }
})
