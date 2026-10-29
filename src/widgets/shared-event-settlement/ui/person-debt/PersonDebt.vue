<script setup lang="ts">
import { Money } from '@/entities/currency'
import { cn } from '@/shared/lib/utils'

import type { LineTone, PersonLine } from '../../lib/person-line'

/** One person's debt in words with the sums drawn as `<Money>`: green is owed to me, red is owed by me. */
const props = defineProps<{ line: PersonLine; currency: string; decimals: number }>()

const TONE: Record<LineTone, string> = {
  'owes-me': 'font-semibold text-success',
  'i-owe': 'font-semibold text-destructive',
  muted: 'text-muted-foreground',
}
</script>

<template>
  <i18n-t :keypath="`sharedEvents.settle.line.${props.line.key}`" tag="span" :class="cn('tabular-nums', TONE[props.line.tone])">
    <template #amount>
      <Money :amount="props.line.amount ?? '0'" :currency="props.currency" :decimals="props.decimals" :convert="false" />
    </template>
    <template #total>
      <Money :amount="props.line.total ?? '0'" :currency="props.currency" :decimals="props.decimals" :convert="false" />
    </template>
    <template #returned>
      <Money :amount="props.line.returned ?? '0'" :currency="props.currency" :decimals="props.decimals" :convert="false" />
    </template>
    <template #writtenOff>
      <Money :amount="props.line.writtenOff ?? '0'" :currency="props.currency" :decimals="props.decimals" :convert="false" />
    </template>
    <template #name>{{ props.line.name }}</template>
  </i18n-t>
</template>
