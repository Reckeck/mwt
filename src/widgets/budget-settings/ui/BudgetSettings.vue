<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueryClient } from '@tanstack/vue-query'

import { FormField } from '@/shared/ui/form-field'
import { AmountInput } from '@/shared/ui/amount-input'
import { Select, SelectItem } from '@/shared/ui/select'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { useSettingsStore } from '@/shared/store/settings'
import { notify } from '@/shared/lib/notify'

const { t } = useI18n()
const settings = useSettingsStore()
const queryClient = useQueryClient()

/** The share is stored as 0..1 and edited as a percent. */
const percent = ref('')
watch(
  () => settings.vault?.savings_rate,
  (rate) => {
    percent.value = rate === undefined ? '' : String(Math.round(Number(rate) * 10_000) / 100)
  },
  { immediate: true },
)

/** A usual month for a vault without history; blank is sent as zero, which clears it. */
const manualIncome = ref('')
const manualExpenses = ref('')
watch(
  () => [settings.vault?.manual_income, settings.vault?.manual_expenses],
  ([income, expenses]) => {
    manualIncome.value = income ?? ''
    manualExpenses.value = expenses ?? ''
  },
  { immediate: true },
)

/** Field messages: a value that does not fit stays in the field with its reason beside it. */
const manualError = ref<Record<'manual_income' | 'manual_expenses', string | undefined>>({ manual_income: undefined, manual_expenses: undefined })
const rateError = ref<string>()

function saveManual(key: 'manual_income' | 'manual_expenses', raw: string) {
  const value = raw.trim().replace(',', '.') || '0'
  const parsed = Number(value)
  if (!Number.isFinite(parsed) || parsed < 0) {
    manualError.value[key] = t('settings.manualInvalid')
    return
  }
  manualError.value[key] = undefined
  if (value !== (settings.vault?.[key] ?? '0')) void persist({ [key]: value })
}

async function persist(patch: Partial<Settings>) {
  try {
    await settings.updateVault(patch)
    notify.saved()
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
    await settings.loadVault().catch(() => undefined)
  }
}

/** Every zone the platform knows; the OS zone is the empty choice. */
const zones = (() => {
  try {
    return Intl.supportedValuesOf('timeZone')
  } catch {
    return []
  }
})()
const systemZone = Intl.DateTimeFormat().resolvedOptions().timeZone

async function saveZone(zone: string) {
  try {
    await settings.updateApp({ timezone: zone })
    notify.saved()
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
  }
}

function saveRate() {
  const value = Number(percent.value.replace(',', '.'))
  if (!Number.isFinite(value) || value < 0 || value > 100) {
    rateError.value = t('settings.savingsRateInvalid')
    return
  }
  rateError.value = undefined
  const rate = String(value / 100)
  if (rate !== settings.vault?.savings_rate) void persist({ savings_rate: rate })
}
</script>

<template>
  <section class="grid gap-4 rounded-lg border border-border bg-card p-4" data-budget-settings>
    <h3 class="heading-card">{{ t('settings.budgetTitle') }}</h3>

    <FormField for="savings-rate" :label="t('settings.savingsRate')" :hint="t('settings.savingsRateHint')" :error="rateError">
      <AmountInput id="savings-rate" v-model="percent" class="w-32" @change="saveRate" />
    </FormField>

    <div class="grid gap-1.5">
      <div class="flex flex-wrap gap-3">
        <FormField for="manual-income" :label="t('settings.manualIncome')" :error="manualError.manual_income">
          <AmountInput id="manual-income" v-model="manualIncome" class="w-40" @change="saveManual('manual_income', manualIncome)" />
        </FormField>
        <FormField for="manual-expenses" :label="t('settings.manualExpenses')" :error="manualError.manual_expenses">
          <AmountInput id="manual-expenses" v-model="manualExpenses" class="w-40" @change="saveManual('manual_expenses', manualExpenses)" />
        </FormField>
      </div>
      <p class="text-xs text-muted-foreground">{{ t('settings.manualHint') }}</p>
    </div>

    <FormField for="daily-budget-mode" :label="t('settings.dailyBudgetMode')" :hint="t('settings.dailyBudgetHint')">
      <Select
        id="daily-budget-mode"
        :model-value="settings.vault?.daily_budget_mode ?? 'month'"
        @update:model-value="(value) => persist({ daily_budget_mode: value as DailyBudgetMode })"
      >
        <SelectItem value="month">{{ t('settings.dailyBudget.month') }}</SelectItem>
        <SelectItem value="until_income">{{ t('settings.dailyBudget.until_income') }}</SelectItem>
      </Select>
    </FormField>

    <FormField for="timezone" :label="t('settings.timezone')" :hint="t('settings.timezoneHint')">
      <Select id="timezone" :model-value="settings.app?.timezone ?? ''" @update:model-value="(value) => saveZone(String(value))">
        <SelectItem value="">{{ t('settings.timezoneSystem', { zone: systemZone }) }}</SelectItem>
        <SelectItem v-for="zone in zones" :key="zone" :value="zone">{{ zone }}</SelectItem>
      </Select>
    </FormField>
  </section>
</template>
