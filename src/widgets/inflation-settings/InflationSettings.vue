<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus, RefreshCw } from 'lucide-vue-next'

import { CurrencyPicker, useInflationOverview } from '@/entities/currency'
import { useInflationActions } from '@/features/inflation-mode'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { required, useFormValidation } from '@/shared/lib/form'
import { useVaultStore } from '@/shared/store/vault'

import { useManualInflation } from './model/use-manual-inflation'
import InflationRow from './ui/InflationRow.vue'

const { t } = useI18n()
const vault = useVaultStore()
const { query } = useInflationOverview()
const { refresh, setMode } = useInflationActions()
const { table, persist, add, remove } = useManualInflation()

const overview = computed(() => (vault.locked ? [] : (query.data.value ?? [])))
/** Built-in currencies first (as the vault orders them), then the user's own. */
const codes = computed(() => {
  const known = overview.value.map((entry) => entry.currency)
  return [...known, ...Object.keys(table.value).filter((code) => !known.includes(code))]
})
const entryOf = (code: string) => overview.value.find((entry) => entry.currency === code) ?? null

function setManual(code: string, value: string) {
  table.value = { ...table.value, [code]: value }
}

const adding = ref('')
const adder = useFormValidation(() => ({
  currency:
    required(adding.value, 'pick') ?? (codes.value.includes(adding.value) ? t('validation.duplicateCurrency') : null),
}))

function submitAdd() {
  add(adding.value)
  adding.value = ''
  adder.reset()
}
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-start gap-2">
        <div class="grid gap-1">
          <h3 class="heading-card">{{ t('inflation.title') }}</h3>
          <p class="text-sm text-muted-foreground">{{ t('inflation.hint') }}</p>
        </div>
        <Button
          v-if="!vault.locked"
          variant="outline"
          size="sm"
          class="ml-auto shrink-0"
          :loading="refresh.isPending.value"
          @click="refresh.mutate()"
        >
          <RefreshCw class="size-4" />
          {{ t('inflation.refresh') }}
        </Button>
      </div>
    </template>
    <div class="grid gap-2">
      <div>
        <InflationRow
          v-for="code in codes"
          :key="code"
          :code="code"
          :entry="entryOf(code)"
          :removable="!entryOf(code)?.source && table[code] !== undefined"
          :model-value="table[code] ?? ''"
          @update:model-value="(value) => setManual(code, value)"
          @change="persist()"
          @mode="(mode) => setMode.mutate({ currency: code, mode })"
          @remove="remove(code)"
        />
      </div>
      <div class="flex items-center gap-2">
        <div class="w-48" :data-invalid="adder.invalidAttr('currency')">
          <CurrencyPicker v-model="adding" :kinds="['fiat']" :placeholder="t('inflation.pick')" />
        </div>
        <Button variant="outline" size="sm" @click="adder.submit(submitAdd, $event)">
          <Plus class="size-4" />
          {{ t('inflation.add') }}
        </Button>
      </div>
      <p v-if="adder.firstMessage()" class="text-xs text-destructive" role="alert">{{ adder.firstMessage() }}</p>
    </div>
  </Card>
</template>
