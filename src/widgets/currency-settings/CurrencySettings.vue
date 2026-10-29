<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ArrowRight, ChevronDown, Info } from 'lucide-vue-next'

import { BaseCurrencyChangeDialog } from '@/features/base-currency-change'
import { CurrencyPicker, FX_TABLE_STALE_MS } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { FormField } from '@/shared/ui/form-field'
import { Select, SelectItem } from '@/shared/ui/select'
import { SettingsSection } from '@/shared/ui/settings-section'
import { call } from '@/shared/api/ipc'
import { formatAsOf, numberFormat } from '@/shared/lib/format'
import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'

import FavoriteChips from './ui/FavoriteChips.vue'

/**
 * `Currencies` (Currency-1 · 1): the three roles of a currency side by side —
 * the base one, the budget's, the favourites — with what each one changes,
 * how often rates refresh and how many digits a coin shows. A new base
 * currency is asked first (Currency-2 · 9).
 */
const { t } = useI18n()
const settings = useSettingsStore()

const changeTo = ref('')
const changeOpen = ref(false)
function askBase(code: string) {
  if (!code || code === settings.baseCurrency) return
  changeTo.value = code
  changeOpen.value = true
}

async function persist(save: () => Promise<void>, reload: () => Promise<unknown>) {
  try {
    await save()
    notify.saved()
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
    await reload().catch(() => undefined)
  }
}
const saveApp = (patch: Partial<UpdateAppSettingsInput>) => persist(() => settings.updateApp(patch), () => settings.loadApp())
const saveBudget = (code: string) => persist(() => settings.updateVault({ budget_currency: code }), () => settings.loadVault())

const intervals = computed(() => [30, 60, 300, 900, 1800, 3600].map((value) => ({ value, label: t(`currencySettings.every.${value}`) })))
const interval = computed(() => String(settings.app?.fx_refresh_interval_sec ?? settings.vault?.fx_refresh_interval_sec ?? 300))

const table = useQuery({ queryKey: ['fx-table'], queryFn: () => call<FxTable>('get_fx_table'), staleTime: FX_TABLE_STALE_MS })
/** The newest rate in the table: `Latest: today 14:20`. */
const lastRefresh = computed(() => {
  const stamps = (table.data.value?.rows ?? []).map((row) => row.updated_at).filter(Boolean).sort()
  const last = stamps.at(-1)
  if (!last) return t('currencySettings.neverRefreshed')
  const today = new Date(last).toDateString() === new Date().toDateString()
  return today ? t('currencySettings.lastToday', { time: formatAsOf(last) }) : t('currencySettings.lastAt', { at: formatAsOf(last) })
})

/** «8 — 0,00980000»: the same small coin amount at each precision. */
const DECIMALS = [2, 4, 6, 8]
const decimalsLabel = (digits: number) =>
  `${digits} — ${numberFormat({ minimumFractionDigits: digits, maximumFractionDigits: digits }).format(0.0098)}`

function openRates() {
  document.querySelector<HTMLElement>('[data-section="rates"]')?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}
</script>

<template>
  <SettingsSection :title="t('currencySettings.title')">
    <div class="grid gap-3 @md:grid-cols-2">
      <FormField for="base-currency" :label="t('settings.baseCurrency')" required :hint="t('currencySettings.baseHint')">
        <CurrencyPicker id="base-currency" :model-value="settings.baseCurrency" @update:model-value="askBase" />
      </FormField>
      <FavoriteChips />
    </div>
    <!-- The canvas shows only the base and favourite currencies; the rest of Currency-1 waits under `More`. -->
    <Collapsible class="grid gap-4">
      <CollapsibleTrigger class="group flex min-h-10 w-fit items-center gap-1 text-sm text-primary hover:underline" data-currency-more>
        {{ t('currencySettings.more') }}
        <ChevronDown class="size-4 transition-transform group-data-[state=open]:rotate-180" />
      </CollapsibleTrigger>
      <CollapsibleContent class="grid gap-4">
        <FormField for="budget-currency" :label="t('settings.budgetCurrency')" :hint="t('currencySettings.budgetHint')">
          <CurrencyPicker
            id="budget-currency"
            allow-empty
            :placeholder="t('settings.budgetCurrencyBase', { currency: settings.baseCurrency.toUpperCase() })"
            :model-value="settings.vault?.budget_currency ?? ''"
            @update:model-value="(value) => saveBudget(String(value))"
          />
        </FormField>
        <p class="flex items-start gap-3 rounded-lg bg-muted px-3 py-2.5 text-sm" data-currency-note>
          <Info class="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
          <span>{{ t('currencySettings.note') }}</span>
        </p>
        <div class="grid gap-3 @md:grid-cols-2">
          <FormField for="fx-interval" :label="t('currencySettings.refresh')" :hint="lastRefresh">
            <Select id="fx-interval" :model-value="interval" @update:model-value="(value) => saveApp({ fx_refresh_interval_sec: Number(value) })">
              <SelectItem v-for="option in intervals" :key="option.value" :value="String(option.value)">{{ option.label }}</SelectItem>
            </Select>
          </FormField>
          <FormField for="crypto-decimals" :label="t('currencySettings.decimals')" :hint="t('currencySettings.decimalsHint')">
            <Select
              id="crypto-decimals"
              :model-value="String(settings.displayDecimals)"
              @update:model-value="(value) => saveApp({ display_decimals: Number(value) })"
            >
              <SelectItem v-for="digits in DECIMALS" :key="digits" :value="String(digits)">{{ decimalsLabel(digits) }}</SelectItem>
            </Select>
          </FormField>
        </div>
        <div class="flex items-center gap-3">
          <span class="min-w-0 flex-1 text-sm font-semibold">{{ t('currencySettings.rates') }}</span>
          <Button variant="link" size="sm" class="h-auto touch-hit p-0" data-currency-rates @click="openRates">
            {{ t('currencySettings.open') }}<ArrowRight class="size-3.5" />
          </Button>
        </div>
      </CollapsibleContent>
    </Collapsible>
    <BaseCurrencyChangeDialog v-model:open="changeOpen" :target="changeTo" />
  </SettingsSection>
</template>
