<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { WizardSteps } from '@/shared/ui/wizard-steps'
import { firstError, nonNegativeAmount, positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { parseDecimal } from '@/shared/lib/decimal'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { PLANNING_KEYS, dateInputToIso, isoToDateInput, localDate } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'

import { GOAL_INCOME_CATEGORY, GOAL_SAVE_PCT } from './goal-edit.constants'
import type { GoalDialogProps, GoalField } from './goal-edit.types'
import GoalWhatStep from './ui/GoalWhatStep.vue'
import GoalWhereStep from './ui/GoalWhereStep.vue'

/**
 * A goal in two steps (Money 1 · 7, Money 2 · 8): what is saved for, then
 * where. A new goal can take the month's remainder at once and a share of each
 * salary after it; saving says so and can be taken back.
 */
const props = withDefaults(defineProps<GoalDialogProps>(), { goal: null, draft: null, storeId: '', remainder: null })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [goal: Goal] }>()

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()

const tree = useStoreTree()
const liveNodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const step = ref(1)
const name = ref('')
const target = ref('')
const currency = ref('')
const startsAt = ref('')
const deadline = ref('')
const priority = ref(3)
const commitment = ref('')
const expectedReturn = ref('')
const storeIds = ref<string[]>([])
const active = ref(true)
const scenarioId = ref<string | null>(null)
const fund = ref(true)
const autosave = ref(false)

watch(open, (isOpen) => {
  if (!isOpen) return
  // A draft (a goal from a calculator scenario) fills a new goal like a saved one.
  const goal = props.goal ?? props.draft
  step.value = 1
  name.value = goal?.name ?? ''
  target.value = goal?.target_amount ?? ''
  currency.value = goal?.currency ?? props.remainder?.currency ?? (settings.vault?.budget_currency || settings.baseCurrency)
  startsAt.value = isoToDateInput(goal?.starts_at) || localDate(new Date())
  deadline.value = isoToDateInput(goal?.deadline)
  priority.value = goal?.priority ?? 3
  commitment.value = goal?.monthly_commitment ?? ''
  expectedReturn.value = goal?.expected_return ?? ''
  storeIds.value = goal?.scope.store_ids.length ? [...goal.scope.store_ids] : props.storeId ? [props.storeId] : []
  active.value = goal?.active ?? true
  scenarioId.value = goal?.scenario_id ?? null
  fund.value = true
  autosave.value = false
})

/** Each step checks its own fields: «Далее» the first, «Создать» the second. */
const form = useFormValidation<GoalField>(
  () =>
    step.value === 1
      ? {
          name: required(name.value),
          target: positiveAmount(target.value),
          currency: required(currency.value, 'pick'),
          deadline: deadline.value && startsAt.value && deadline.value < startsAt.value ? t('validation.dateOrder') : null,
        }
      : {
          stores: storeIds.value.length ? null : t('validation.stores'),
          startsAt: required(startsAt.value),
          commitment: nonNegativeAmount(commitment.value),
          // The core takes 0..100 % a year; the form says so before it does.
          expectedReturn: firstError(
            nonNegativeAmount(expectedReturn.value),
            Number(parseDecimal(expectedReturn.value) ?? 0) > 100 ? t('validation.range', { min: 0, max: 100 }) : null,
          ),
        },
  { resetOn: [open, step] },
)

const steps = computed(() => [t('goal.step.what'), t('goal.step.where')])
const creating = computed(() => !props.goal)

const save = useMutation({
  mutationFn: async () => {
    const input: GoalInput = {
      name: name.value.trim(),
      active: active.value,
      target_amount: target.value.trim(),
      currency: currency.value,
      starts_at: dateInputToIso(startsAt.value),
      deadline: deadline.value ? dateInputToIso(deadline.value) : null,
      priority: priority.value,
      monthly_commitment: commitment.value.trim() || null,
      scope: { store_ids: storeIds.value, include_children: true },
      scenario_id: scenarioId.value,
      expected_return: expectedReturn.value.trim() || null,
    }
    if (props.goal) return call<Goal>('update_goal', { input: { id: props.goal.id, goal: input } })
    const goal = await call<Goal>('create_goal', { input })
    const home = storeIds.value[0]
    const remainder = props.remainder
    if (fund.value && remainder && remainder.currency.toLowerCase() === input.currency.toLowerCase()) {
      await call('fund_goal', { input: { goal_id: goal.id, store_id: home, amount: remainder.amount } })
    }
    if (autosave.value) {
      const params = { category: GOAL_INCOME_CATEGORY, pct: String(GOAL_SAVE_PCT), target: home }
      await call('workflow_from_template', { input: { template_id: 'pay_yourself_first', params } })
    }
    return goal
  },
  onSuccess: (goal) => {
    for (const queryKey of [...PLANNING_KEYS, ['envelopes'], ['workflows']]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
    emit('saved', goal)
  },
  meta: {
    error: 'goal.saveFailed',
    success: (data) => ({ text: props.goal ? t('goal.saved') : t('goal.created'), description: (data as Goal).name }),
  },
})

function submit(event: Event) {
  if (step.value === 1) form.submit(() => (step.value = 2), event)
  else form.submit(() => save.mutate(), event)
}
</script>

<template>
  <Dialog v-model:open="open" :title="props.goal ? t('goal.edit') : t('goal.new')">
    <form id="goal-form" class="grid gap-4" @submit.prevent="submit">
      <WizardSteps :step="step" :titles="steps" />
      <GoalWhatStep v-if="step === 1" v-model:name="name" v-model:target="target" v-model:currency="currency" v-model:deadline="deadline" :form="form" />
      <GoalWhereStep
        v-else
        v-model:store-ids="storeIds"
        v-model:fund="fund"
        v-model:autosave="autosave"
        v-model:starts-at="startsAt"
        v-model:commitment="commitment"
        v-model:expected-return="expectedReturn"
        v-model:priority="priority"
        v-model:active="active"
        :form="form"
        :nodes="liveNodes"
        :remainder="props.remainder"
        :creating="creating"
        :target="target"
        :currency="currency"
      />
    </form>

    <template #footer>
      <Button variant="outline" class="max-md:h-11" @click="step === 1 ? (open = false) : (step = 1)">
        {{ step === 1 ? t('actions.cancel') : t('actions.back') }}
      </Button>
      <Button
        form="goal-form"
        type="submit"
        class="max-md:h-11 max-md:flex-1"
        :disabled="save.isPending.value"
        :data-wizard-next="step === 1 || undefined"
        :data-wizard-save="step === 2 || undefined"
      >
        {{ step === 1 ? t('actions.next') : creating ? t('actions.create') : t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
