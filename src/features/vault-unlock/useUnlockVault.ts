import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

import { useBaseCurrencyChoice } from '@/features/base-currency-choice'
import { parseError } from '@/shared/api/ipc'
import { vaultErrorMessage } from '@/shared/lib/vault-error'
import { useVaultStore } from '@/shared/store/vault'

/** State and actions of the unlock form; the dialog only renders them. */
export function useUnlockVault() {
  const { t } = useI18n()
  const router = useRouter()
  const vault = useVaultStore()

  const password = ref('')
  /** A fresh install opening its first vault (an import) asks for the base currency too. */
  const baseCurrency = useBaseCurrencyChoice()
  const error = ref<string | null>(null)
  const busy = ref(false)
  /** `true` when the vault came from the recents list and the path is fixed. */
  const pathFixed = ref(false)
  const targetPath = ref('')
  /** File name of a vault picked in the dialog (on mobile the path is app storage). */
  const pickedName = ref<string | null>(null)
  /** The vault and its external copy both changed: the user picks one. */
  const conflict = ref(false)
  const path = computed(() => targetPath.value.trim())

  /** Start over for `fixedPath`, or with an editable path when it is `null`. */
  function reset(fixedPath: string | null) {
    pathFixed.value = fixedPath !== null
    targetPath.value = fixedPath ?? ''
    pickedName.value = null
    password.value = ''
    error.value = null
    conflict.value = false
    baseCurrency.reset()
  }

  async function browse() {
    try {
      const picked = await vault.pickVaultFile(false)
      if (!picked) return
      targetPath.value = picked.path
      pickedName.value = picked.display_name
      error.value = null
      if (picked.imported) void vault.loadRecents()
    } catch (err) {
      error.value = vaultErrorMessage(err, t)
    }
  }

  async function submit() {
    if (!path.value) {
      error.value = t(vault.appStorage ? 'auth.fileRequired' : 'auth.pathRequired')
      return
    }
    busy.value = true
    error.value = null
    conflict.value = false
    try {
      await vault.unlock(path.value, password.value)
      await baseCurrency.save()
      await router.replace({ name: 'dashboard' })
    } catch (err) {
      conflict.value = parseError(err).code === 'SYNC_CONFLICT'
      error.value = conflict.value ? null : vaultErrorMessage(err, t)
    } finally {
      busy.value = false
    }
  }

  /** Keep one copy, then try the password again. */
  async function resolveConflict(keep: ConflictSide) {
    busy.value = true
    try {
      await vault.resolveConflict(path.value, keep)
      conflict.value = false
    } catch (err) {
      error.value = vaultErrorMessage(err, t)
      return
    } finally {
      busy.value = false
    }
    await submit()
  }

  return {
    password,
    baseCurrency,
    error,
    busy,
    pathFixed,
    targetPath,
    pickedName,
    conflict,
    path,
    reset,
    browse,
    submit,
    resolveConflict,
  }
}
