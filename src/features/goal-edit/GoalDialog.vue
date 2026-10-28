<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { firstError, nonNegativeAmount, positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { parseDecimal } from '@/shared/lib/decimal'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { PLANNING_KEYS, dateInputToIso, isoToDateInput, localDate } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { matchesHotkey } from '@/shared/lib/hotkeys'
import { useSettingsStore } from '@/shared/store/settings'

import { GOAL_INCOME_CATEGORY, GOAL_SAVE_PCT } from './goal-edit.constants'
import type { GoalDialogProps, GoalField } from './goal-edit.types'
import GoalWhatStep from './ui/GoalWhatStep.vue'
import GoalWhereStep from './ui/GoalWhereStep.vue'

/**
 * A goal on one page (Full-New-Fixes 1, 3): what is saved for, how much, in which one
 * wallet, by when. A new goal can take the month's remainder at once and a share of each
 * salary after it; saving says so and can be taken back. Creating is all-or-nothing: if a
 * later step fails, the goal that was just made is removed again.
 */
const props = withDefaults(defineProps<GoalDialogProps>(), {
  goal: null,
  draft: null,
  storeId: '',
  remainder: null,
  fromCalc: null,
  saved: null,
})
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [goal: Goal] }>()
/** An edited goal's own rows (what fills it, its files) and actions («Удалить цель») come from the card. */
defineSlots<{ extras?(): unknown; actions?(): unknown }>()

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()

const tree = useStoreTree()
const liveNodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

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
// Off until picked: moving the month's remainder is a choice, not a default.
const fund = ref(false)
const autosave = ref(false)

watch(open, (isOpen) => {
  if (!isOpen) return
  // A draft (a goal from a calculator scenario) fills a new goal like a saved one.
  const goal = props.goal ?? props.draft
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
  fund.value = false
  autosave.value = false
})

const creating = computed(() => !props.goal)

const whatRules = () => ({
  name: name.value.trim() ? null : t('goal.nameRequired'),
  target: positiveAmount(target.value) ? t('goal.targetPositive') : null,
  currency: required(currency.value, 'pick'),
  deadline: deadline.value && startsAt.value && deadline.value < startsAt.value ? t('validation.dateOrder') : null,
})
const whereRules = () => ({
  stores: storeIds.value.length ? null : t('goal.storeRequired'),
  startsAt: required(startsAt.value),
  commitment: nonNegativeAmount(commitment.value),
  // The core takes 0..100 % a year; the form says so before it does.
  expectedReturn: firstError(
    nonNegativeAmount(expectedReturn.value),
    Number(parseDecimal(expectedReturn.value) ?? 0) > 100 ? t('validation.range', { min: 0, max: 100 }) : null,
  ),
})
const form = useFormValidation<GoalField>(() => ({ ...whatRules(), ...whereRules() }), { resetOn: [open] })

const title = computed(() => (props.goal ? t('goal.edit') : props.fromCalc ? t('goal.newFromCalc') : t('goal.new')))
const saveLabel = computed(() => {
  if (!creating.value) return t('actions.save')
  return autosave.value ? t('goal.createWithAutomation') : t('goal.create')
})

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
    try {
      const home = storeIds.value[0]
      const remainder = props.remainder
      if (fund.value && home && remainder && remainder.currency.toLowerCase() === input.currency.toLowerCase()) {
        await call('fund_goal', { input: { goal_id: goal.id, store_id: home, amount: remainder.amount } })
      }
      if (autosave.value) {
        const params = { category: GOAL_INCOME_CATEGORY, pct: String(GOAL_SAVE_PCT), target: home }
        await call('workflow_from_template', { input: { template_id: 'pay_yourself_first', params } })
      }
    } catch (error) {
      // Nothing stays half-made: a failed step takes the new goal (and its envelope) back.
      await call('delete_goal', { id: goal.id }).catch(() => {})
      throw error
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

function onKeydown(event: KeyboardEvent) {
  if (matchesHotkey(event, 'saveAndNext')) submit(event)
}

function submit(event: Event) {
  form.submit(() => save.mutate(), event)
}
</script>

<template>
  <Dialog overlay="goal" v-model:open="open" :title="title" size="form">
    <fieldset class="grid min-w-0 gap-4" @keydown="onKeydown">
    <form id="goal-form" class="grid gap-4" @submit.prevent="submit">
      <p v-if="creating && props.fromCalc" class="text-sm text-muted-foreground">{{ t('goal.fromCalcNote', { name: props.fromCalc.scenario }) }}</p>
      <GoalWhatStep v-model:name="name" v-model:target="target" v-model:currency="currency" :form="form" />
      <GoalWhereStep
        v-model:store-ids="storeIds"
        v-model:deadline="deadline"
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
        :already="props.fromCalc?.initial ?? null"
        :saved="props.saved"
      />
    </form>
    <!-- Outside the form: the files block has a form of its own. -->
    <slot name="extras" />
    </fieldset>

    <template #footer>
      <slot name="actions" />
      <span v-if="$slots.actions" class="flex-1" />
      <Button variant="outline" class="max-md:h-11" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button
        form="goal-form"
        type="submit"
        class="max-md:h-11 max-md:flex-1"
        :disabled="save.isPending.value"
        data-wizard-save
      >
        {{ saveLabel }}
      </Button>
    </template>
  </Dialog>
</template>
