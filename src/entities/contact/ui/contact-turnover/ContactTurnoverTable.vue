<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Money } from '@/entities/currency'


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
        <span class="text-destructive">{{ t('contacts.sent') }} <Money :amount="row.sent" :currency="row.asset.symbol" /></span>
        <span class="text-success">{{ t('contacts.received') }} <Money :amount="row.received" :currency="row.asset.symbol" /></span>
      </div>
    </template>
  </div>
</template>
