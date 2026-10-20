<script setup lang="ts">
import { computed, useId } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { AssetInput } from '@/entities/asset'
import { CurrencyPicker, Money } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { CategorySelect } from '@/features/category-select'
import { AccountRefPicker } from '@/features/transaction-create'
import { Input } from '@/shared/ui/input'
import { TimeInput } from '@/shared/ui/time-input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { Switch } from '@/shared/ui/switch'
import { Textarea } from '@/shared/ui/textarea'
import { call } from '@/shared/api/ipc'
import { WEEKDAYS } from '@/shared/lib/planning'
import { WORKFLOW_KEYS, localized } from '@/shared/lib/workflow'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

import TargetsField from './TargetsField.vue'
import { useStoreTree } from '@/shared/api/queries'

/** One field of a node, edited by the control its kind asks for. */
/** `error`: the check's word on this very field (an empty required one), shown under it. */
const props = defineProps<{ param: ParamDef; value: unknown; params: Record<string, unknown>; error?: string | null }>()
const emit = defineEmits<{ update: [value: unknown] }>()

const { t, locale } = useI18n()
const id = useId()
const kind = computed(() => props.param.kind.kind)
const current = computed(() => props.value ?? props.param.default)
const text = computed(() => (current.value === null || current.value === undefined ? '' : String(current.value)))

const tree = useStoreTree(computed(() => kind.value === 'store'))
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
  <div class="grid grid-cols-[minmax(0,1fr)] gap-1.5 [&>*]:min-w-0" :data-invalid="props.error ? '' : undefined">
    <div v-if="kind === 'bool'" class="flex items-center justify-between gap-2">
      <Label :for="id" class="min-w-0 [overflow-wrap:anywhere]">{{ localized(props.param.label, locale) }}</Label>
      <Switch :id="id" :model-value="Boolean(current)" @update:model-value="(on) => emit('update', on)" />
    </div>
    <template v-else>
      <Label :for="id" :required="props.param.required">{{ localized(props.param.label, locale) }}</Label>
      <Select v-if="props.param.kind.kind === 'select'" :id="id" :model-value="text" @update:model-value="(next) => emit('update', next)">
        <SelectItem v-for="[value, label] in props.param.kind.options" :key="value" :value="value">
          {{ localized(label, locale) }}
        </SelectItem>
      </Select>
      <Select v-else-if="kind === 'weekday'" :id="id" :model-value="text" @update:model-value="(next) => emit('update', Number(next))">
        <SelectItem v-for="day in WEEKDAYS" :key="day" :value="String(day)">{{ t(`schedule.weekday.${day}`) }}</SelectItem>
      </Select>
      <Input
        v-else-if="kind === 'number' || kind === 'day'"
        :id="id"
        :model-value="text"
        inputmode="decimal"
        @update:model-value="(next) => emit('update', kind === 'day' ? Number(next) || 1 : String(next))"
      />
      <TimeInput v-else-if="kind === 'time'" :id="id" :model-value="text" @update:model-value="(next) => emit('update', next)" />
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
        <SelectItem value="">{{ t('workflow.field.notSet') }}</SelectItem>
        <SelectItem v-for="budget in budgets.data.value ?? []" :key="budget.id" :value="budget.id">
          {{ t(`budget.periodKind.${budget.period}`) }} · <Money :amount="budget.limit" :currency="budget.currency" />
        </SelectItem>
      </Select>
      <Select v-else-if="kind === 'goal'" :id="id" :model-value="text" @update:model-value="(next) => emit('update', next || null)">
        <SelectItem value="">{{ t('workflow.field.notSet') }}</SelectItem>
        <SelectItem v-for="goal in goals.data.value ?? []" :key="goal.id" :value="goal.id">{{ goal.name }}</SelectItem>
      </Select>
      <template v-else-if="kind === 'workflow'">
        <Select :id="id" :model-value="text" @update:model-value="(next) => emit('update', next || null)">
          <SelectItem value="">{{ t('workflow.field.notSet') }}</SelectItem>
          <SelectItem v-for="workflow in callable" :key="workflow.id" :value="workflow.id">{{ workflow.name }}</SelectItem>
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
    <p v-if="props.error" class="text-xs text-destructive" role="alert">{{ props.error }}</p>
  </div>
</template>
