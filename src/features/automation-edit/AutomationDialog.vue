<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { TransactionForm } from '@/features/transaction-create'
import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS, dateInputToIso, isoToDateInput, localDate } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { txLegs } from '@/shared/lib/transactions'
import { firstError, integerInRange, required, useFormValidation } from '@/shared/lib/form'
import { Switch } from '@/shared/ui/switch'

import type { ScheduleKind } from './automation-edit.constants'
import type { AutomationDialogProps } from './automation-edit.types'
import { AutomationScheduleFields } from './ui/automation-schedule-fields'

const props = withDefaults(defineProps<AutomationDialogProps>(), { automation: null, storeId: '', preset: null })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [automation: Automation] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const liveNodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const name = ref('')
const active = ref(true)
const storeId = ref('')
const scheduleKind = ref<ScheduleKind>('monthly')
const day = ref('1')
const weekday = ref('0')
const everyDays = ref('7')
const startsAt = ref('')

/** The stored template as a transaction the form can prefill from. */
const template = computed(() => (props.automation?.template as Transaction | undefined) ?? null)

// Every opening starts from the automation as stored (or blank).
watch(open, (isOpen) => {
  if (!isOpen) return
  const automation = props.automation
  name.value = automation?.name ?? (props.preset ? t(`preset.${props.preset.preset}`) : '')
  active.value = automation?.active ?? true
  storeId.value =
    (template.value && txLegs(template.value)[0]?.store_id) ||
    props.storeId ||
    (liveNodes.value.find((node) => node.store.purpose === 'everyday') ?? liveNodes.value[0])?.store.id ||
    ''
  const schedule = automation?.schedule ?? { monthly: { day: new Date().getDate() } }
  if ('monthly' in schedule) {
    scheduleKind.value = 'monthly'
    day.value = String(schedule.monthly.day)
  } else if ('weekly' in schedule) {
    scheduleKind.value = 'weekly'
    weekday.value = String(schedule.weekly.weekday)
  } else {
    scheduleKind.value = 'every_n_days'
    everyDays.value = String(schedule.every_n_days.days)
  }
  startsAt.value = isoToDateInput(automation?.starts_at) || localDate(new Date())
})

const schedule = computed<Schedule | null>(() => {
  if (scheduleKind.value === 'monthly') {
    const value = Number(day.value)
    return value >= 1 && value <= 31 ? { monthly: { day: value } } : null
  }
  if (scheduleKind.value === 'weekly') return { weekly: { weekday: Number(weekday.value) } }
  const value = Number(everyDays.value)
  return Number.isInteger(value) && value >= 1 ? { every_n_days: { days: value } } : null
})

/** What a preset starts the template as. */
const initialTx = computed(() =>
  props.preset && !props.automation
    ? { kind: props.preset.tx_kind, categories: props.preset.category ? [props.preset.category] : [] }
    : null,
)

const form = ref<InstanceType<typeof TransactionForm> | null>(null)
const fields = useFormValidation(
  () => ({
    name: required(name.value),
    day: scheduleKind.value === 'monthly' ? firstError(required(day.value), integerInRange(day.value, 1, 31)) : null,
    everyDays:
      scheduleKind.value === 'monthly' || scheduleKind.value === 'weekly'
        ? null
        : firstError(required(everyDays.value), integerInRange(everyDays.value, 1, 3650)),
    startsAt: required(startsAt.value),
    // The template's own fields carry their messages; this only keeps the save from going through.
    template: form.value?.valid ? null : t('validation.summary'),
  }),
  { resetOn: open },
)

/**
 * Enter in the schedule fields saves, as in any form. The operation below is
 * a form of its own (forms cannot nest) and saves on Enter by itself.
 */
function saveOnEnter(event: KeyboardEvent) {
  const target = event.target as HTMLElement
  if (target.closest('form') || target.tagName === 'TEXTAREA' || target.tagName === 'BUTTON') return
  event.preventDefault()
  trySave(event)
}

/** Both halves are checked at once: the schedule above and the operation below. */
function trySave(event?: Event) {
  form.value?.showErrors()
  fields.submit(save.mutate, event)
}

const save = useMutation({
  mutationFn: () => {
    const templateInput = form.value?.input
    if (!templateInput || !schedule.value) throw { code: 'INVALID_INPUT', message: t('form.assetRequired') }
    const input: AutomationInput = {
      name: name.value.trim(),
      active: active.value,
      preset: props.automation?.preset ?? props.preset?.preset ?? null,
      schedule: schedule.value,
      starts_at: dateInputToIso(startsAt.value),
      template: templateInput,
    }
    return props.automation
      ? call<Automation>('update_automation', { input: { id: props.automation.id, automation: input } })
      : call<Automation>('create_automation', { input })
  },
  onSuccess: (automation) => {
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
    emit('saved', automation)
  },
  meta: {
    error: 'automation.saveFailed',
    success: (data) => ({
      text: props.automation ? t('automation.saved') : t('automation.created'),
      description: (data as Automation).name,
    }),
  },
})
</script>

<template>
  <Dialog
    v-model:open="open"
    :title="props.automation ? t('automation.edit') : props.preset ? t(`preset.${props.preset.preset}`) : t('automation.new')"
    size="lg"
  >
    <!-- biome-ignore lint/a11y/useSemanticElements: a fieldset would wrap the operation form inside; this only catches Enter from the schedule fields. -->
    <div class="@container grid gap-4" role="group" @keydown.enter="saveOnEnter">
      <div class="grid gap-3 @sm:grid-cols-2">
        <FormField for="automation-name" :label="t('automation.name')" required :error="fields.error('name')">
          <Input id="automation-name" v-model="name" :placeholder="t('automation.namePlaceholder')" />
        </FormField>
        <FormField for="automation-store" :label="t('automation.store')" :hint="t('automation.checkHint')">
          <StoreNodeSelect id="automation-store" v-model="storeId" :nodes="liveNodes" />
        </FormField>
      </div>

      <TransactionForm
        v-if="open && storeId"
        :key="storeId"
        ref="form"
        :store-id="storeId"
        :transaction="template"
        :initial="initialTx"
        template
        @submit="trySave()"
      />

      <AutomationScheduleFields
        v-model:kind="scheduleKind"
        v-model:day="day"
        v-model:weekday="weekday"
        v-model:every-days="everyDays"
        v-model:starts-at="startsAt"
        :errors="{ day: fields.error('day'), everyDays: fields.error('everyDays'), startsAt: fields.error('startsAt') }"
      />

      <div class="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2">
        <Label for="automation-active">{{ t('automation.active') }}</Label>
        <Switch id="automation-active" v-model="active" />
      </div>
    </div>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button :disabled="save.isPending.value" @click="trySave($event)">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
