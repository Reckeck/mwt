<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'

defineProps<{ error?: string }>()
const currency = defineModel<string>({ required: true })

const { t } = useI18n()
</script>

<template>
  <!-- One currency for the whole app: totals, budgets and reports are counted in it. -->
  <div class="grid gap-1.5">
    <CurrencyPicker id="vault-base-currency" v-model="currency" :label="t('auth.baseCurrency')" required :error="error" />
    <p class="text-xs text-muted-foreground">{{ t('auth.baseCurrencyHint') }}</p>
  </div>
</template>
