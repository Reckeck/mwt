<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import type { ContactNet } from '@/entities/contact'
import { Money } from '@/entities/currency'
import { cn } from '@/shared/lib/utils'

/**
 * The largest debt in its own currency, coloured by direction, `+N` when more currencies are owed.
 * Nothing at all without a debt (the cell keeps its place).
 */
const props = defineProps<{ nets: ContactNet[] }>()
const { t } = useI18n()
</script>

<template>
  <span v-if="props.nets.length" class="inline-flex items-baseline gap-1.5 whitespace-nowrap tabular-nums">
    <span :class="cn('font-semibold', props.nets[0].net > 0 ? 'text-success' : 'text-destructive')">
      <Money :amount="props.nets[0].net" :currency="props.nets[0].symbol" signed :decimals="0" :convert="false" />
    </span>
    <span v-if="props.nets.length > 1" class="text-xs text-muted-foreground" :title="t('contacts.table.moreCurrencies')">
      +{{ props.nets.length - 1 }}
    </span>
  </span>
</template>
