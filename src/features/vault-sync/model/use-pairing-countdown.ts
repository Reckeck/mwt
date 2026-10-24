import { computed, onScopeDispose, ref, type Ref } from 'vue'

const TICK_MS = 1000

/** «4:52»: what is left of the pairing code, from the backend's own deadline. */
export function usePairingCountdown(expiresMs: Ref<number | null | undefined>) {
  const now = ref(Date.now())
  const timer = window.setInterval(() => {
    now.value = Date.now()
  }, TICK_MS)
  onScopeDispose(() => window.clearInterval(timer))

  return computed(() => {
    if (expiresMs.value == null) return null
    const seconds = Math.max(0, Math.ceil((expiresMs.value - now.value) / TICK_MS))
    return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
  })
}
