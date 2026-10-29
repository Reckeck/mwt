<script setup lang="ts">
import { Minus, Plus } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { decimalAdd, decimalSub, parseDecimal } from '@/shared/lib/decimal'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'

/** One person's share of an item in units: «−  3  +  шт  675 ₽». */
const props = defineProps<{ name: string; unit: string; amount: string; currency: string; decimals: number }>()
const qty = defineModel<string>({ required: true })
const { t } = useI18n()

function step(delta: 1 | -1) {
  const now = parseDecimal(qty.value) ?? '0'
  const next = (delta > 0 ? decimalAdd(now, '1') : decimalSub(now, '1')) ?? now
  qty.value = next.startsWith('-') ? '0' : next
}
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-x-3 gap-y-1">
    <span class="truncate text-sm">{{ props.name }}</span>
    <div class="flex items-center gap-1">
      <Button type="button" variant="outline" size="icon-sm" :aria-label="t('sharedEvents.split.less', { name: props.name })" @click="step(-1)">
        <Minus class="size-4" />
      </Button>
      <Input v-model="qty" inputmode="decimal" placeholder="0" class="h-9 w-14 px-1 text-center tabular-nums" :aria-label="props.name" />
      <Button type="button" variant="outline" size="icon-sm" :aria-label="t('sharedEvents.split.more', { name: props.name })" @click="step(1)">
        <Plus class="size-4" />
      </Button>
      <span class="w-6 text-xs text-muted-foreground">{{ props.unit }}</span>
    </div>
    <span class="w-24 text-right text-sm tabular-nums" :class="props.amount === '0' ? 'text-muted-foreground' : 'font-semibold'">
      <Money :amount="props.amount" :currency="props.currency" :decimals="props.decimals" :convert="false" />
    </span>
  </div>
</template>
