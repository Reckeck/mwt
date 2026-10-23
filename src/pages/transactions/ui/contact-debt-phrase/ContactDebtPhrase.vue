<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { cn } from '@/shared/lib/utils'

import type { ContactNet } from '@/entities/contact'

/**
 * A contact's debts in words (Ops-1 · 6): «вам должны 5 000 ₽», «вы должны
 * 120 USDT», one phrase per asset, green when they owe, red when you do.
 * Without debts it falls back to the given text («долгов нет», «обменник»).
 */
const props = defineProps<{ nets: ContactNet[]; empty: string; strong?: boolean }>()

const { t } = useI18n()
</script>

<template>
  <span v-if="props.nets.length" class="inline-flex flex-wrap gap-x-2">
    <span
      v-for="row in props.nets"
      :key="row.symbol"
      :class="cn('tabular-nums', row.net > 0 ? 'text-success' : 'text-destructive', props.strong && 'font-semibold')"
    >
      {{ row.net > 0 ? t('contacts.debt.owesYou') : t('contacts.debt.youOwe') }}
      <Money :amount="Math.abs(row.net)" :currency="row.symbol" :decimals="0" :convert="false" />
    </span>
  </span>
  <span v-else>{{ props.empty }}</span>
</template>
