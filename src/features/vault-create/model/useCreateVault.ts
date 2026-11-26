import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

import { useBaseCurrencyChoice } from '@/features/vault-unlock/@x/vault-create'
import { vaultErrorMessage } from '@/shared/lib/vault-error'
import { useVaultStore } from '@/shared/store/vault'

/** Name suggested for the first vault in app storage (`vault.wallet`). */
const DEFAULT_VAULT_NAME = 'vault'

/** State and actions of the create form; the dialog only renders them. */
export function useCreateVault() {
  const { t } = useI18n()
  const router = useRouter()
  const vault = useVaultStore()

  const path = ref('')
  const password = ref('')
  const passwordRepeat = ref('')
  /** `With a password` (the default) or `no password`: a plain vault skips the password fields altogether. */
  const withPassword = ref(true)
  const baseCurrency = useBaseCurrencyChoice()
  const error = ref<string | null>(null)
  const busy = ref(false)

  async function reset() {
    password.value = ''
    passwordRepeat.value = ''
    withPassword.value = true
    baseCurrency.reset()
    error.value = null
    // Pre-fill the default location only while it is still free — suggesting a
    // path that already holds a vault would just fail with ALREADY_EXISTS. In
    // app storage (mobile) the form asks for a name, not a path.
    try {
      const status = vault.status ?? (await vault.refreshStatus())
      const suggested = status?.storage === 'app' ? DEFAULT_VAULT_NAME : (status?.path ?? '')
      path.value = status?.exists ? '' : suggested
    } catch {
      path.value = ''
    }
  }

  async function browse() {
    try {
      const picked = await vault.pickVaultFile(true)
      if (picked) path.value = picked.path
    } catch (err) {
      error.value = vaultErrorMessage(err, t)
    }
  }

  async function submit() {
    const target = path.value.trim()
    if (!target) {
      error.value = t(vault.appStorage ? 'auth.nameRequired' : 'auth.pathRequired')
      return
    }
    busy.value = true
    error.value = null
    try {
      await vault.createVault(target, withPassword.value ? password.value : null)
      await baseCurrency.save()
      await router.replace({ name: 'dashboard' })
    } catch (err) {
      error.value = vaultErrorMessage(err, t)
    } finally {
      busy.value = false
    }
  }

  return { path, password, passwordRepeat, withPassword, baseCurrency, error, busy, reset, browse, submit }
}
