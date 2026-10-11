<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { formatMoney } from '@/shared/lib/format'

import type { ContactTurnoverProps } from './contact-turnover.types'

const props = defineProps<ContactTurnoverProps>()
const { t } = useI18n()
</script>

<template>
  <div class="grid gap-2">
    <p v-if="!props.turnover?.rows.length" class="text-sm text-muted-foreground">{{ t('contacts.noTurnover') }}</p>
    <template v-else>
      <p class="text-xs text-muted-foreground">{{ t('contacts.txCount', { count: props.turnover.tx_count }) }}</p>
      <div v-for="row in props.turnover.rows" :key="row.asset.symbol" class="flex flex-wrap gap-x-4 text-sm tabular-nums">
        <span class="w-16 font-medium">{{ row.asset.symbol }}</span>
        <span class="text-destructive">{{ t('contacts.sent') }} {{ formatMoney(row.sent) }}</span>
        <span class="text-emerald-600 dark:text-emerald-400">{{ t('contacts.received') }} {{ formatMoney(row.received) }}</span>
      </div>
    </template>
  </div>
</template>
