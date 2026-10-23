<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

import PlanCalculator from './PlanCalculator.vue'
import type { CalculatorPlan, PlanCalculatorDialogProps } from './plan-calculator.types'

/** «Посчитать сделку · Bybit» (Money 4 · 23): records nothing until «Создать операцию». */
const props = defineProps<PlanCalculatorDialogProps>()
const open = defineModel<boolean>('open', { required: true })
/** The page closes this dialog and opens the transaction form with the plan. */
const emit = defineEmits<{ apply: [plan: CalculatorPlan] }>()

const { t } = useI18n()
const calculator = ref<InstanceType<typeof PlanCalculator> | null>(null)
</script>

<template>
  <Dialog
    v-model:open="open"
    :title="props.exchange ? t('calc.titleAt', { exchange: props.exchange }) : t('calc.title')"
    :description="t('calc.hint')"
    size="form"
  >
    <!-- A form, so Enter in any field applies the plan like the button does. -->
    <form id="plan-calculator-form" class="@container" @submit.prevent="calculator?.apply($event)">
      <PlanCalculator ref="calculator" :base-currency="props.baseCurrency" @apply="(plan) => emit('apply', plan)" />
    </form>

    <template #footer>
      <Button variant="outline" class="max-sm:hidden" @click="open = false">{{ t('actions.close') }}</Button>
      <Button type="submit" form="plan-calculator-form" class="max-sm:w-full" data-trade-calc-apply>{{ t('calc.apply') }}</Button>
    </template>
  </Dialog>
</template>
