<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

import { Card } from '@/shared/ui/card'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const settings = useSettingsStore()

/** The share is stored as 0..1 and edited as a percent. */
const percent = ref('')
watch(
  () => settings.vault?.savings_rate,
  (rate) => {
    percent.value = rate === undefined ? '' : String(Math.round(Number(rate) * 10_000) / 100)
  },
  { immediate: true },
)

async function persist(patch: Partial<Settings>) {
  try {
    await settings.updateVault(patch)
    toast.success(t('settings.saved'))
  } catch (err) {
    toast.error(t('settings.saveFailed'), { description: codeErrorMessage(parseError(err), t) })
    await settings.loadVault().catch(() => undefined)
  }
}

function saveRate() {
  const value = Number(percent.value.replace(',', '.'))
  if (!Number.isFinite(value) || value < 0 || value > 100) {
    toast.error(t('settings.savingsRateInvalid'))
    return
  }
  const rate = String(value / 100)
  if (rate !== settings.vault?.savings_rate) void persist({ savings_rate: rate })
}
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t('settings.budgetTitle') }}</p>
    </template>

    <div class="grid gap-4">
      <div class="grid gap-1.5">
        <Label for="savings-rate">{{ t('settings.savingsRate') }}</Label>
        <Input id="savings-rate" v-model="percent" inputmode="decimal" class="w-32" @change="saveRate" />
        <p class="text-xs text-muted-foreground">{{ t('settings.savingsRateHint') }}</p>
      </div>

      <div class="grid gap-1.5">
        <Label for="budget-currency">{{ t('settings.budgetCurrency') }}</Label>
        <Select
          id="budget-currency"
          :model-value="settings.vault?.budget_currency ?? ''"
          @update:model-value="(value) => persist({ budget_currency: String(value) })"
        >
          <option value="">{{ t('settings.budgetCurrencyBase', { currency: settings.baseCurrency.toUpperCase() }) }}</option>
          <option v-for="code in settings.allCurrencies" :key="code" :value="code">{{ code.toUpperCase() }}</option>
        </Select>
        <p class="text-xs text-muted-foreground">{{ t('settings.budgetCurrencyHint') }}</p>
      </div>

      <div class="grid gap-1.5">
        <Label for="daily-budget-mode">{{ t('settings.dailyBudgetMode') }}</Label>
        <Select
          id="daily-budget-mode"
          :model-value="settings.vault?.daily_budget_mode ?? 'month'"
          @update:model-value="(value) => persist({ daily_budget_mode: value as DailyBudgetMode })"
        >
          <option value="month">{{ t('settings.dailyBudget.month') }}</option>
          <option value="until_income">{{ t('settings.dailyBudget.until_income') }}</option>
        </Select>
        <p class="text-xs text-muted-foreground">{{ t('settings.dailyBudgetHint') }}</p>
      </div>
    </div>
  </Card>
</template>
