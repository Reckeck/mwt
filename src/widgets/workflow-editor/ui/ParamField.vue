<script setup lang="ts">
import { computed, useId } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { AssetInput } from '@/entities/asset'
import { CurrencyPicker } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { CategorySelect } from '@/features/category-select'
import { AccountRefPicker } from '@/features/transaction-create'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { Switch } from '@/shared/ui/switch'
import { Textarea } from '@/shared/ui/textarea'
import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'
import { WEEKDAYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY } from '@/shared/lib/store-tree'
import { WORKFLOW_KEYS, localized } from '@/shared/lib/workflow'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

import TargetsField from './TargetsField.vue'

/** One field of a node, edited by the control its kind asks for. */
const props = defineProps<{ param: ParamDef; value: unknown; params: Record<string, unknown> }>()
const emit = defineEmits<{ update: [value: unknown] }>()

const { t, locale } = useI18n()
const id = useId()
const kind = computed(() => props.param.kind.kind)
const current = computed(() => props.value ?? props.param.default)
const text = computed(() => (current.value === null || current.value === undefined ? '' : String(current.value)))

const tree = useQuery({
  queryKey: STORE_TREE_QUERY_KEY,
  queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
  enabled: computed(() => kind.value === 'store'),
})
const budgets = useQuery({
  queryKey: ['budgets'],
  queryFn: () => call<Budget[]>('list_budgets'),
  enabled: computed(() => kind.value === 'budget'),
})
const workflows = useQuery({
  queryKey: WORKFLOW_KEYS.list,
  queryFn: () => call<Workflow[]>('list_workflows'),
  enabled: computed(() => kind.value === 'workflow'),
})
/** Only workflows that can be called: they start with «Вызов из другой схемы». */
const callable = computed(() =>
  (workflows.data.value ?? []).filter((workflow) => workflow.graph.includes('"trigger.called"')),
)
const goals = useQuery({
  queryKey: ['goals'],
  queryFn: () => call<Goal[]>('list_goals'),
  enabled: computed(() => kind.value === 'goal'),
})

const storeId = computed(() => {
  const raw = current.value as { store?: string } | string | null
  if (!raw) return ''
  return typeof raw === 'string' ? raw : (raw.store ?? '')
})
const CATEGORY_GROUPS: CategoryGroup[] = ['income', 'fixed', 'daily', 'savings', 'service']
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)] gap-1.5 [&>*]:min-w-0">
    <div v-if="kind === 'bool'" class="flex items-center justify-between gap-2">
      <Label :for="id" class="min-w-0 [overflow-wrap:anywhere]">{{ localized(props.param.label, locale) }}</Label>
      <Switch :id="id" :model-value="Boolean(current)" @update:model-value="(on) => emit('update', on)" />
    </div>
    <template v-else>
      <Label :for="id">{{ localized(props.param.label, locale) }}</Label>
      <Select v-if="props.param.kind.kind === 'select'" :id="id" :model-value="text" @update:model-value="(next) => emit('update', next)">
        <option v-for="[value, label] in props.param.kind.options" :key="value" :value="value">
          {{ localized(label, locale) }}
        </option>
      </Select>
      <Select v-else-if="kind === 'weekday'" :id="id" :model-value="text" @update:model-value="(next) => emit('update', Number(next))">
        <option v-for="day in WEEKDAYS" :key="day" :value="String(day)">{{ t(`schedule.weekday.${day}`) }}</option>
      </Select>
      <Input
        v-else-if="kind === 'number' || kind === 'day'"
        :id="id"
        :model-value="text"
        inputmode="decimal"
        @update:model-value="(next) => emit('update', kind === 'day' ? Number(next) || 1 : String(next))"
      />
      <Input v-else-if="kind === 'time'" :id="id" type="time" :model-value="text" @update:model-value="(next) => emit('update', String(next))" />
      <DateTimePicker v-else-if="kind === 'date'" :id="id" :model-value="text" @update:model-value="(next) => emit('update', next || null)" />
      <Textarea v-else-if="kind === 'long_text'" :id="id" :model-value="text" rows="3" @update:model-value="(next) => emit('update', String(next))" />
      <StoreNodeSelect
        v-else-if="kind === 'store'"
        :model-value="storeId"
        :nodes="tree.data.value ?? []"
        :empty-label="t('workflow.field.anyStore')"
        @update:model-value="(next) => emit('update', next ? { store: next } : null)"
      />
      <AccountRefPicker
        v-else-if="kind === 'account'"
        :id="id"
        :model-value="(current as AccountRef | null) ?? null"
        :empty-label="t('workflow.field.notSet')"
        @update:model-value="(next) => emit('update', next)"
      />
      <AssetInput
        v-else-if="kind === 'asset'"
        :model-value="typeof current === 'object' ? (current as AssetRef | null) : null"
        label=""
        @update:model-value="(next) => emit('update', next)"
      />
      <CurrencyPicker v-else-if="kind === 'currency'" :id="id" :model-value="text" label="" @update:model-value="(next) => emit('update', next)" />
      <CategorySelect
        v-else-if="kind === 'category'"
        :id="id"
        :model-value="text"
        :groups="CATEGORY_GROUPS"
        no-create
        @update:model-value="(next) => emit('update', next || null)"
      />
      <Select v-else-if="kind === 'budget'" :id="id" :model-value="text" @update:model-value="(next) => emit('update', next || null)">
        <option value="">{{ t('workflow.field.notSet') }}</option>
        <option v-for="budget in budgets.data.value ?? []" :key="budget.id" :value="budget.id">
          {{ t(`budget.periodKind.${budget.period}`) }} · {{ formatMoney(budget.limit, { currency: budget.currency }) }}
        </option>
      </Select>
      <Select v-else-if="kind === 'goal'" :id="id" :model-value="text" @update:model-value="(next) => emit('update', next || null)">
        <option value="">{{ t('workflow.field.notSet') }}</option>
        <option v-for="goal in goals.data.value ?? []" :key="goal.id" :value="goal.id">{{ goal.name }}</option>
      </Select>
      <template v-else-if="kind === 'workflow'">
        <Select :id="id" :model-value="text" @update:model-value="(next) => emit('update', next || null)">
          <option value="">{{ t('workflow.field.notSet') }}</option>
          <option v-for="workflow in callable" :key="workflow.id" :value="workflow.id">{{ workflow.name }}</option>
        </Select>
        <p class="text-xs text-muted-foreground">{{ t('workflow.field.callableHint') }}</p>
      </template>
      <TargetsField
        v-else-if="kind === 'targets'"
        :value="current"
        :percent="props.params.mode !== 'fixed'"
        @update="(next) => emit('update', next)"
      />
      <Input v-else :id="id" :model-value="text" @update:model-value="(next) => emit('update', String(next))" />
    </template>
  </div>
</template>
