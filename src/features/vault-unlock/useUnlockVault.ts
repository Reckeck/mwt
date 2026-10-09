import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

import { vaultErrorMessage } from '@/shared/lib/vault-error'
import { useVaultStore } from '@/shared/store/vault'

/** State and actions of the unlock form; the dialog only renders them. */
export function useUnlockVault() {
  const { t } = useI18n()
  const router = useRouter()
  const vault = useVaultStore()

  const password = ref('')
  const error = ref<string | null>(null)
  const busy = ref(false)
  /** `true` when the vault came from the recents list and the path is fixed. */
  const pathFixed = ref(false)
  const targetPath = ref('')
  const path = computed(() => targetPath.value.trim())

  /** Start over for `fixedPath`, or with an editable path when it is `null`. */
  function reset(fixedPath: string | null) {
    pathFixed.value = fixedPath !== null
    targetPath.value = fixedPath ?? ''
    password.value = ''
    error.value = null
  }

  async function browse() {
    try {
      const picked = await vault.pickVaultFile(false)
      if (picked) targetPath.value = picked
    } catch (err) {
      error.value = vaultErrorMessage(err, t)
    }
  }

  async function submit() {
    if (!path.value) {
      error.value = t('auth.pathRequired')
      return
    }
    busy.value = true
    error.value = null
    try {
      await vault.unlock(path.value, password.value)
      await router.replace({ name: 'dashboard' })
    } catch (err) {
      error.value = vaultErrorMessage(err, t)
    } finally {
      busy.value = false
    }
  }

  return { password, error, busy, pathFixed, targetPath, path, reset, browse, submit }
}
