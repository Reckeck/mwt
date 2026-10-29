<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Calculator, Plus } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { signClass } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'

import { investMoney } from '../../lib/invest-view'

/** `Profit · Fees · Invested` (all three on a phone too), `Work out a trade` (Money 4 · 23) and `Record a trade` (Money 2 · 13). */
const props = defineProps<{ profit: string; fees: string; invested: string; currency: string }>()
const emit = defineEmits<{ record: []; calc: [] }>()

const { t } = useI18n()
</script>

<template>
  <div class="grid gap-3" data-tour="invest-figures">
    <div class="grid grid-cols-3 gap-3 max-md:gap-2">
      <section class="grid min-w-0 gap-1 rounded-xl border bg-card p-4 shadow-sm max-md:p-3">
        <span class="text-xs text-muted-foreground">{{ t('money.invest.profit') }}</span>
        <b class="text-xl tabular-nums max-md:text-base" :class="signClass(props.profit)" data-invest-profit>
          <Money v-bind="investMoney(props.profit, props.currency, true)" />
        </b>
      </section>
      <section class="grid min-w-0 gap-1 rounded-xl border bg-card p-4 shadow-sm max-md:p-3">
        <span class="text-xs text-muted-foreground">{{ t('money.invest.fees') }}</span>
        <b class="text-xl tabular-nums max-md:text-base" :class="Number(props.fees) > 0 && 'text-warning'">
          <Money v-bind="investMoney(props.fees, props.currency)" />
        </b>
      </section>
      <section class="grid min-w-0 gap-1 rounded-xl border bg-card p-4 shadow-sm max-md:p-3">
        <span class="text-xs text-muted-foreground">{{ t('money.invest.invested') }}</span>
        <b class="text-xl tabular-nums max-md:text-base"><Money v-bind="investMoney(props.invested, props.currency)" /></b>
      </section>
    </div>
    <div class="flex justify-end gap-2 max-md:grid max-md:grid-cols-2">
      <Button variant="ghost" class="max-md:h-10" data-invest-calc @click="emit('calc')"><Calculator />{{ t('calc.title') }}</Button>
      <Button variant="outline" class="max-md:h-10" data-invest-record @click="emit('record')"><Plus />{{ t('money.invest.record') }}</Button>
    </div>
  </div>
</template>
