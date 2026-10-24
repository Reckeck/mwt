<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueryClient } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { AmountInput } from '@/shared/ui/amount-input'
import { Label } from '@/shared/ui/label'
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

function saveManual(key: 'manual_income' | 'manual_expenses', raw: string) {
  const value = raw.trim().replace(',', '.') || '0'
  const parsed = Number(value)
  if (!Number.isFinite(parsed) || parsed < 0) {
    notify.error(null, 'settings.manualInvalid')
    return
  }
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
    notify.error(null, 'settings.savingsRateInvalid')
    return
  }
  const rate = String(value / 100)
  if (rate !== settings.vault?.savings_rate) void persist({ savings_rate: rate })
}
</script>

<template>
  <Card>
    <template #header>
      <h3 class="heading-card">{{ t('settings.budgetTitle') }}</h3>
    </template>

    <div class="grid gap-4">
      <div class="grid gap-1.5">
        <Label for="savings-rate">{{ t('settings.savingsRate') }}</Label>
        <AmountInput id="savings-rate" v-model="percent" class="w-32" @change="saveRate" />
        <p class="text-xs text-muted-foreground">{{ t('settings.savingsRateHint') }}</p>
      </div>

      <div class="grid gap-1.5">
        <div class="flex flex-wrap gap-3">
          <div class="grid gap-1.5">
            <Label for="manual-income">{{ t('settings.manualIncome') }}</Label>
            <AmountInput id="manual-income" v-model="manualIncome" class="w-40" @change="saveManual('manual_income', manualIncome)" />
          </div>
          <div class="grid gap-1.5">
            <Label for="manual-expenses">{{ t('settings.manualExpenses') }}</Label>
            <AmountInput
              id="manual-expenses"
              v-model="manualExpenses"
              class="w-40"
              @change="saveManual('manual_expenses', manualExpenses)"
            />
          </div>
        </div>
        <p class="text-xs text-muted-foreground">{{ t('settings.manualHint') }}</p>
      </div>

      <div class="grid gap-1.5">
        <Label for="daily-budget-mode">{{ t('settings.dailyBudgetMode') }}</Label>
        <Select
          id="daily-budget-mode"
          :model-value="settings.vault?.daily_budget_mode ?? 'month'"
          @update:model-value="(value) => persist({ daily_budget_mode: value as DailyBudgetMode })"
        >
          <SelectItem value="month">{{ t('settings.dailyBudget.month') }}</SelectItem>
          <SelectItem value="until_income">{{ t('settings.dailyBudget.until_income') }}</SelectItem>
        </Select>
        <p class="text-xs text-muted-foreground">{{ t('settings.dailyBudgetHint') }}</p>
      </div>

      <div class="grid gap-1.5">
        <Label for="timezone">{{ t('settings.timezone') }}</Label>
        <Select
          id="timezone"
          :model-value="settings.app?.timezone ?? ''"
          @update:model-value="(value) => saveZone(String(value))"
        >
          <SelectItem value="">{{ t('settings.timezoneSystem', { zone: systemZone }) }}</SelectItem>
          <SelectItem v-for="zone in zones" :key="zone" :value="zone">{{ zone }}</SelectItem>
        </Select>
        <p class="text-xs text-muted-foreground">{{ t('settings.timezoneHint') }}</p>
      </div>
    </div>
  </Card>
</template>
