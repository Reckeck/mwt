<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

import PlanCalculator from './PlanCalculator.vue'
import type { CalculatorPlan, PlanCalculatorProps } from './plan-calculator.types'

const props = defineProps<PlanCalculatorProps>()
const open = defineModel<boolean>('open', { required: true })
/** The page closes this dialog and opens the transaction form with the plan. */
const emit = defineEmits<{ apply: [plan: CalculatorPlan] }>()

const { t } = useI18n()
const calculator = ref<InstanceType<typeof PlanCalculator> | null>(null)
</script>

<template>
  <Dialog v-model:open="open" :title="t('wallet.plan')" :description="t('calc.hint')" size="lg">
    <div class="@container">
      <PlanCalculator ref="calculator" :base-currency="props.baseCurrency" @apply="(plan) => emit('apply', plan)" />
    </div>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.close') }}</Button>
      <Button :disabled="!calculator?.canApply" @click="calculator?.apply()">{{ t('calc.apply') }}</Button>
    </template>
  </Dialog>
</template>
