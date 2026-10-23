<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Calculator, Plus } from 'lucide-vue-next'

import { signClass } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'

import { investMoney } from '../../lib/invest-view'

/** «Прибыль · Комиссии · Вложено», «Посчитать сделку» (Money 4 · 23) and «Записать сделку» (Money 2 · 13). */
const props = defineProps<{ profit: string; fees: string; invested: string; currency: string }>()
const emit = defineEmits<{ record: []; calc: [] }>()

const { t } = useI18n()
</script>

<template>
  <div class="grid gap-3" data-tour="invest-figures">
    <div class="grid grid-cols-3 gap-3 max-md:grid-cols-2">
      <section class="grid gap-1 rounded-xl border bg-card p-4 shadow-sm">
        <span class="text-xs text-muted-foreground">{{ t('money.invest.profit') }}</span>
        <b class="text-xl tabular-nums" :class="signClass(props.profit)" data-invest-profit>{{ investMoney(props.profit, props.currency, true) }}</b>
      </section>
      <section class="grid gap-1 rounded-xl border bg-card p-4 shadow-sm">
        <span class="text-xs text-muted-foreground">{{ t('money.invest.fees') }}</span>
        <b class="text-xl tabular-nums" :class="Number(props.fees) > 0 && 'text-warning'">{{ investMoney(props.fees, props.currency) }}</b>
      </section>
      <section class="grid gap-1 rounded-xl border bg-card p-4 shadow-sm max-md:hidden">
        <span class="text-xs text-muted-foreground">{{ t('money.invest.invested') }}</span>
        <b class="text-xl tabular-nums">{{ investMoney(props.invested, props.currency) }}</b>
      </section>
    </div>
    <div class="flex justify-end gap-2 max-md:grid max-md:grid-cols-2">
      <Button variant="ghost" data-invest-calc @click="emit('calc')"><Calculator />{{ t('calc.title') }}</Button>
      <Button variant="outline" data-invest-record @click="emit('record')"><Plus />{{ t('money.invest.record') }}</Button>
    </div>
  </div>
</template>
