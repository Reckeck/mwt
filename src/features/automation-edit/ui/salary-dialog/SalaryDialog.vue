<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Combobox, type ComboboxOption } from '@/shared/ui/combobox'
import { Dialog } from '@/shared/ui/dialog'
import { FormField } from '@/shared/ui/form-field'
import { SwitchRow } from '@/shared/ui/switch-row'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { call } from '@/shared/api/ipc'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryLabel } from '@/shared/lib/categories'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { flattenStoreTree, withoutLoyalty } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'

import { SALARY_SHARE_PCT, defaultPayout, salaryInput, type SalaryPayout } from '../../lib/salary-plan'
import SalaryPayoutCard from './SalaryPayoutCard.vue'

/**
 * «Зарплата» (Auto-2 · 9): the ready-made start already filled in. One payout
 * a month or two (a salary and an advance), each its own automation with its
 * own day, month it is for and amount; the income category, the wallet and
 * the switch «10 % на Наличные» are shared. The switch adds the scheme's
 * second step once.
 */
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [automation: Automation] }>()

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()
const categories = useCategories()
const tree = useStoreTree()

const category = ref('')
const storeId = ref('')
const share = ref(true)
/** How many payouts a month the dialog shows; the second one's fields are kept while it is hidden. */
const count = ref(1)
const payouts = ref<SalaryPayout[]>([])

const nodes = computed(() => withoutLoyalty(tree.data.value ?? []))
const flat = computed(() => flattenStoreTree(nodes.value))
/** Where «10 %» goes: the first cash wallet; without one there is no switch. */
const cash = computed(() => flat.value.find((node) => node.store.kind === 'cash' && !node.store.archived)?.store ?? null)
const incomes = computed(() => (categories.data.value ?? []).filter((entry) => entry.group === 'income' && !entry.archived))
const options = computed<ComboboxOption[]>(() => incomes.value.map((entry) => ({ value: entry.key, label: categoryLabel(entry, t) })))
const shown = computed(() => payouts.value.slice(0, count.value))

let autoCurrency = settings.baseCurrency
watch(
  open,
  (isOpen) => {
    if (!isOpen) return
    category.value = incomes.value.find((entry) => entry.key === 'salary')?.key ?? incomes.value[0]?.key ?? ''
    storeId.value = flat.value.find((node) => node.store.kind === 'bank' && !node.store.archived)?.store.id ?? flat.value[0]?.store.id ?? ''
    share.value = true
    count.value = 1
    autoCurrency = walletCurrency(storeId.value)
    payouts.value = [
      defaultPayout('salary', t('salary.title'), autoCurrency),
      defaultPayout('advance', t('salary.advance'), autoCurrency),
    ]
  },
  { immediate: true },
)

/** What the wallet mostly holds: the usual amount is in that currency until the user picks another. */
function walletCurrency(id: string) {
  const held = [...(flat.value.find((node) => node.store.id === id)?.balances ?? [])].sort(
    (left, right) => Number(right.value_base) - Number(left.value_base),
  )
  return held[0]?.asset.symbol.toLowerCase() ?? settings.baseCurrency
}
watch(storeId, (id) => {
  const next = walletCurrency(id)
  payouts.value = payouts.value.map((payout) => (payout.currency === autoCurrency ? { ...payout, currency: next } : payout))
  autoCurrency = next
})

const form = useFormValidation(
  () => {
    const rules: Record<string, ReturnType<typeof required>> = {
      category: required(category.value, 'pick'),
      store: required(storeId.value, 'pick'),
    }
    shown.value.forEach((payout, index) => {
      rules[`name${index}`] = required(payout.name)
      rules[`amount${index}`] = positiveAmount(payout.amount)
    })
    return rules
  },
  { resetOn: open },
)

const setCount = (value: unknown) => {
  if (value === '1' || value === '2') count.value = Number(value)
}
const setPayout = (index: number, value: SalaryPayout) => {
  payouts.value = payouts.value.map((entry, at) => (at === index ? value : entry))
}

const save = useMutation({
  mutationFn: async () => {
    const made: Automation[] = []
    try {
      for (const payout of shown.value) {
        const draft = { ...payout, category: category.value, storeId: storeId.value }
        made.push(await call<Automation>('create_automation', { input: salaryInput(draft) }))
      }
    } catch (err) {
      // Two payouts are one decision: when the second fails the first is not left behind.
      for (const automation of made) await call('delete_automation', { id: automation.id }).catch(() => undefined)
      throw err
    }
    if (share.value && cash.value) {
      const params = { category: category.value, pct: String(SALARY_SHARE_PCT), target: cash.value.id }
      await call('workflow_from_template', { input: { template_id: 'pay_yourself_first', params } })
    }
    return made[0] as Automation
  },
  onSuccess: (automation) => {
    for (const queryKey of [...PLANNING_KEYS, ['workflows']]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
    emit('saved', automation)
  },
  meta: {
    error: 'automation.saveFailed',
    success: (data) => ({ text: t('automation.created'), description: (data as Automation).name }),
  },
})
</script>

<template>
  <Dialog overlay="automation-salary" v-model:open="open" size="form" :title="t('salary.title')" :description="t('salary.subtitle')">
    <form id="salary-form" class="grid gap-6" data-tour="salary-form" @submit.prevent="form.submit(save.mutate, $event)">
      <div class="flex flex-wrap items-center gap-x-4 gap-y-2">
        <span id="salary-count-label" class="text-sm font-medium">{{ t('salary.count') }}</span>
        <ToggleGroup type="single" variant="segmented" :spacing="1" aria-labelledby="salary-count-label" :model-value="String(count)" @update:model-value="setCount">
          <ToggleGroupItem value="1" class="px-4" data-salary-count="1">1</ToggleGroupItem>
          <ToggleGroupItem value="2" class="px-4" data-salary-count="2">2</ToggleGroupItem>
        </ToggleGroup>
      </div>
      <div class="grid gap-4">
        <SalaryPayoutCard
          v-for="(payout, index) in shown"
          :key="index"
          :model-value="payout"
          :title="t(index === 0 ? 'salary.title' : 'salary.advance')"
          :id-prefix="`salary-${index}`"
          :name-error="form.error(`name${index}`)"
          :amount-error="form.error(`amount${index}`)"
          @update:model-value="setPayout(index, $event)"
        />
      </div>
      <div class="grid items-start gap-4 sm:grid-cols-2">
        <FormField for="salary-source" :label="t('salary.source')" required :hint="t('salary.sourceHint')" :error="form.error('category')">
          <Combobox id="salary-source" v-model="category" :options="options" />
        </FormField>
        <FormField for="salary-store" :label="t('salary.store')" required :error="form.error('store')">
          <StoreNodeSelect id="salary-store" v-model="storeId" :nodes="nodes" class="w-full" />
        </FormField>
      </div>
      <SwitchRow
        v-if="cash"
        id="salary-share"
        v-model="share"
        plain
        :label="t('salary.share', { pct: SALARY_SHARE_PCT, name: cash.name })"
        :hint="t('salary.shareHint')"
      />
    </form>
    <template #footer>
      <Button variant="ghost" class="max-md:h-11" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button class="max-md:h-11 max-md:flex-1" form="salary-form" type="submit" :disabled="save.isPending.value">{{ t('actions.create') }}</Button>
    </template>
  </Dialog>
</template>
