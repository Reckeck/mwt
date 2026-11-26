import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { useSettingsStore } from '@/shared/store/settings'

/**
 * The base currency asked on a fresh install, in the create and open forms.
 * It is the whole app's, not the vault's; once picked (here or in Settings)
 * the forms stop asking.
 */
export function useBaseCurrencyChoice() {
  const { locale } = useI18n()
  const settings = useSettingsStore()

  const currency = ref('')
  /** Read once per opening: picking the currency must not hide the field mid-form. */
  const asked = ref(false)

  /** A guess from the language: Russian speakers mostly count in roubles. */
  const suggested = computed(() => (String(locale.value).startsWith('ru') ? 'rub' : 'usd'))

  function reset() {
    asked.value = settings.needsBaseCurrency
    currency.value = asked.value ? suggested.value : settings.baseCurrency
  }

  /** After the vault is open: the backend re-values the vault in the new currency. */
  async function save() {
    if (!asked.value || !currency.value) return
    await settings.updateApp({ base_currency: currency.value, base_currency_chosen: true })
  }

  return { currency, asked, reset, save }
}
