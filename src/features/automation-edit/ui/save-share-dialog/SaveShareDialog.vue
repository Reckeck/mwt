<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { StoreNodeSelect } from '@/entities/store'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { FormField } from '@/shared/ui/form-field'
import { Select, SelectItem } from '@/shared/ui/select'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { firstError, integerInRange, required, useFormValidation } from '@/shared/lib/form'
import { formatMoney } from '@/shared/lib/format'
import { flattenStoreTree, withoutLoyalty } from '@/shared/lib/store-tree'

import { SALARY_SHARE_PCT } from '../../lib/salary-plan'

/**
 * «Заплати себе первым» (Auto-2 · 12): a share of each income from one wallet
 * goes to a goal's wallet. Three fields and what it makes of the usual salary.
 */
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()
const tree = useStoreTree()
const goals = useQuery({ queryKey: ['goals'], queryFn: () => call<Goal[]>('list_goals'), enabled: open })
const automations = useQuery({ queryKey: ['automations'], queryFn: () => call<Automation[]>('list_automations'), enabled: open })

const pct = ref(String(SALARY_SHARE_PCT))
const source = ref('')
const goalId = ref('')

const nodes = computed(() => withoutLoyalty(tree.data.value ?? []))
const flat = computed(() => flattenStoreTree(nodes.value))
/** Goals that have a wallet to put the share in. */
const targets = computed(() => (goals.data.value ?? []).filter((goal) => goal.active && goal.scope.store_ids.length > 0))
/** The usual salary, for «С зарплаты 1 575 $ → 157,50 $ в месяц». */
const salary = computed(() => {
  const active = (automations.data.value ?? []).filter((entry) => entry.active)
  const salaries = active.flatMap((entry) => ('topup' in entry.template ? [{ preset: entry.preset, topup: entry.template.topup }] : []))
  // A salary made before the «Зарплата» start has no preset: its category tells.
  const found = salaries.find((row) => row.preset === 'salary') ?? salaries.find((row) => row.topup.meta.categories.includes('salary'))
  const topup = found?.topup
  return topup ? { amount: Number(topup.amount), currency: topup.asset.symbol } : null
})
const share = computed(() => {
  const part = Number(pct.value)
  if (!salary.value || !Number.isFinite(part) || part <= 0) return null
  const money = (value: number) => formatMoney(value, { currency: salary.value?.currency ?? '' })
  return { from: money(salary.value.amount), part: money((salary.value.amount * part) / 100) }
})

watch(
  [open, targets],
  ([isOpen], [wasOpen]) => {
    if (!isOpen) return
    if (!wasOpen) {
      pct.value = String(SALARY_SHARE_PCT)
      source.value = flat.value.find((node) => node.store.kind === 'bank' && !node.store.archived)?.store.id ?? ''
    }
    if (!targets.value.some((goal) => goal.id === goalId.value)) goalId.value = targets.value[0]?.id ?? ''
  },
  { immediate: true },
)

const form = useFormValidation(
  () => ({
    pct: firstError(required(pct.value), integerInRange(pct.value, 1, 100)),
    source: required(source.value, 'pick'),
    goal: required(goalId.value, 'pick'),
  }),
  { resetOn: open },
)

const save = useMutation({
  mutationFn: () => {
    const target = targets.value.find((goal) => goal.id === goalId.value)?.scope.store_ids[0] ?? ''
    const params = { category: 'salary', pct: pct.value.trim(), target, source: source.value }
    return call<SavedWorkflow>('workflow_from_template', { input: { template_id: 'pay_yourself_first', params } })
  },
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ['workflows'] })
    open.value = false
  },
  meta: {
    error: 'automation.saveFailed',
    success: (data) => ({ text: t('automation.created'), description: (data as SavedWorkflow).workflow.name }),
  },
})
</script>

<template>
  <Dialog v-model:open="open" size="form" :title="t('saveShare.title')" :description="t('saveShare.subtitle')">
    <form id="save-share-form" class="grid gap-4" data-tour="save-share-form" @submit.prevent="form.submit(save.mutate, $event)">
      <FormField for="save-share-pct" :label="t('saveShare.pct')" required :error="form.error('pct')">
        <AmountInput id="save-share-pct" v-model="pct" size="lg">
          <template #suffix><span class="px-2 text-sm text-muted-foreground">%</span></template>
        </AmountInput>
      </FormField>
      <div class="grid items-start gap-4 sm:grid-cols-2">
        <FormField for="save-share-source" :label="t('saveShare.source')" required :error="form.error('source')">
          <StoreNodeSelect id="save-share-source" v-model="source" :nodes="nodes" class="w-full" />
        </FormField>
        <FormField for="save-share-goal" :label="t('saveShare.goal')" required :hint="targets.length ? undefined : t('saveShare.noGoals')" :error="form.error('goal')">
          <Select id="save-share-goal" v-model="goalId" :disabled="!targets.length">
            <SelectItem v-for="goal in targets" :key="goal.id" :value="goal.id">{{ goal.name }}</SelectItem>
          </Select>
        </FormField>
      </div>
      <p v-if="share" class="flex flex-wrap items-baseline justify-between gap-2 rounded-md bg-muted px-3 py-2 text-sm" data-save-share-line>
        <span>{{ t('saveShare.from', { sum: share.from }) }}</span>
        <b class="font-semibold tabular-nums">{{ t('saveShare.perMonth', { sum: share.part }) }}</b>
      </p>
    </form>
    <template #footer>
      <Button variant="ghost" class="max-md:h-11" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button class="max-md:h-11 max-md:flex-1" form="save-share-form" type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
