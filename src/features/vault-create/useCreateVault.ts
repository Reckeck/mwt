import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

import { vaultErrorMessage } from '@/shared/lib/vault-error'
import { useVaultStore } from '@/shared/store/vault'

/** Shortest master password the form accepts. */
export const MIN_VAULT_PASSWORD = 4

/** State and actions of the create form; the dialog only renders them. */
export function useCreateVault() {
  const { t } = useI18n()
  const router = useRouter()
  const vault = useVaultStore()

  const path = ref('')
  const password = ref('')
  const passwordRepeat = ref('')
  const error = ref<string | null>(null)
  const busy = ref(false)

  const passwordMatch = computed(() => passwordRepeat.value.length > 0 && passwordRepeat.value === password.value)

  async function reset() {
    password.value = ''
    passwordRepeat.value = ''
    error.value = null
    // Pre-fill the default location only while it is still free — suggesting a
    // path that already holds a vault would just fail with ALREADY_EXISTS.
    try {
      const status = vault.status ?? (await vault.refreshStatus())
      path.value = status?.exists ? '' : (status?.path ?? '')
    } catch {
      path.value = ''
    }
  }

  async function browse() {
    try {
      const picked = await vault.pickVaultFile(true)
      if (picked) path.value = picked
    } catch (err) {
      error.value = vaultErrorMessage(err, t)
    }
  }

  async function submit() {
    const target = path.value.trim()
    if (!target) {
      error.value = t('auth.pathRequired')
      return
    }
    if (password.value.length < MIN_VAULT_PASSWORD) {
      error.value = t('auth.passwordTooShort')
      return
    }
    if (password.value !== passwordRepeat.value) {
      error.value = t('auth.passwordMismatch')
      return
    }
    busy.value = true
    error.value = null
    try {
      await vault.createVault(target, password.value)
      await router.replace({ name: 'dashboard' })
    } catch (err) {
      error.value = vaultErrorMessage(err, t)
    } finally {
      busy.value = false
    }
  }

  return { path, password, passwordRepeat, passwordMatch, error, busy, reset, browse, submit }
}
