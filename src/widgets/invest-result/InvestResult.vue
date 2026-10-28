<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Target } from 'lucide-vue-next'

import type { InvestCalc } from '@/features/invest-calculator'
import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

import { useAnswerExtras } from './model/use-answer-extras'
import type { CompareLine, CompareLines } from './model/use-compare-lines'
import ResultAnswer from './ui/ResultAnswer.vue'
import ResultDetails from './ui/ResultDetails.vue'

/**
 * The answer first, then how it adds up. On a phone (Money 3 · 18) the answer
 * is a card of its own under the fields, «Создать цель» right under it, and the
 * figures, the chart and the years in a card below.
 */
const props = defineProps<{ calc: InvestCalc; compare: CompareLines }>()
const emit = defineEmits<{ goal: []; open: [target: CompareLine['open']] }>()

const { t } = useI18n()
const phone = useIsPhone()
const outcome = computed(() => props.calc.outcome.data.value)
const currency = computed(() => outcome.value?.input.currency ?? props.calc.form.currency)
const extras = useAnswerExtras(outcome, () => props.calc.form.target)

const runsOut = computed(() => Boolean(outcome.value?.input.withdrawal && outcome.value.result.depleted_at !== null))
/** A found value has its own goal button in the answer; money that runs out makes no goal. */
const goalButton = computed(() => Boolean(outcome.value && !outcome.value.solved && !runsOut.value))
</script>

<template>
  <div class="grid content-start gap-4">
    <Card :class="phone && runsOut && 'border-destructive/60'">
      <SectionState
        :loading="props.calc.outcome.isPending.value && props.calc.outcome.fetchStatus.value !== 'idle'"
        :error="props.calc.outcome.error.value"
        :empty="!outcome"
        :empty-title="t('invest.resultEmpty')"
        skeleton="report"
      >
        <div v-if="outcome" class="@container grid gap-4" data-tour="calc-result" data-calc-result>
          <ResultAnswer
            :outcome="outcome"
            :currency="currency"
            :target="props.calc.form.target"
            :prefilled="props.calc.incomeBasis.value"
            :safe-withdrawal="extras.safeWithdrawal.value"
            :unframed="phone"
            @accept="props.calc.acceptSolved"
            @goal="emit('goal')"
            @withdraw="props.calc.applyWithdrawal"
          />
          <ResultDetails
            v-if="!phone"
            :calc="props.calc"
            :compare="props.compare"
            :outcome="outcome"
            :currency="currency"
            :alternatives="extras.alternatives.value"
            @open="emit('open', $event)"
          />
        </div>
      </SectionState>
    </Card>
    <template v-if="phone && outcome">
      <Button v-if="goalButton" class="h-12 w-full" data-calc-goal @click="emit('goal')">
        <Target class="size-4" />{{ t('invest.scenario.makeGoal') }}
      </Button>
      <!-- A card of its own: the figures' top rule, which parts them from the answer, is not needed. -->
      <Card>
        <ResultDetails
          class="[&>dl:first-child]:border-t-0 [&>dl:first-child]:pt-0"
          :calc="props.calc"
          :compare="props.compare"
          :outcome="outcome"
          :currency="currency"
          :alternatives="extras.alternatives.value"
          @open="emit('open', $event)"
        />
      </Card>
    </template>
  </div>
</template>
