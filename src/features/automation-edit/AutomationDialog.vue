<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { TransactionForm } from '@/features/transaction-create'
import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call, parseError } from '@/shared/api/ipc'
import { PLANNING_KEYS, WEEKDAYS, dateInputToIso, isoToDateInput, localDate } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { txLegs } from '@/shared/lib/transactions'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import { SCHEDULE_KINDS, type ScheduleKind } from './automation-edit.constants'
import type { AutomationDialogProps } from './automation-edit.types'

const props = withDefaults(defineProps<AutomationDialogProps>(), { automation: null, storeId: '' })
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
  name.value = automation?.name ?? ''
  active.value = automation?.active ?? true
  storeId.value =
    (template.value && txLegs(template.value)[0]?.store_id) ||
    props.storeId ||
    (liveNodes.value.find((node) => node.store.in_budget) ?? liveNodes.value[0])?.store.id ||
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

const form = ref<InstanceType<typeof TransactionForm> | null>(null)
const valid = computed(
  () => name.value.trim() !== '' && schedule.value !== null && startsAt.value !== '' && Boolean(form.value?.valid),
)

const save = useMutation({
  mutationFn: () => {
    const templateInput = form.value?.input
    if (!templateInput || !schedule.value) throw { code: 'INVALID_INPUT', message: t('form.assetRequired') }
    const input: AutomationInput = {
      name: name.value.trim(),
      active: active.value,
      schedule: schedule.value,
      starts_at: dateInputToIso(startsAt.value),
      template: templateInput,
    }
    return props.automation
      ? call<Automation>('update_automation', { input: { id: props.automation.id, automation: input } })
      : call<Automation>('create_automation', { input })
  },
  onSuccess: (automation) => {
    toast.success(props.automation ? t('automation.saved') : t('automation.created'), { description: automation.name })
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
    emit('saved', automation)
  },
  onError: (err) => toast.error(t('automation.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.automation ? t('automation.edit') : t('automation.new')" size="lg">
    <div class="@container grid gap-4">
      <div class="grid gap-3 @sm:grid-cols-2">
        <div class="grid gap-1.5">
          <Label for="automation-name">{{ t('automation.name') }}</Label>
          <Input id="automation-name" v-model="name" :placeholder="t('automation.namePlaceholder')" />
        </div>
        <div class="grid gap-1.5">
          <Label for="automation-store">{{ t('automation.store') }}</Label>
          <StoreNodeSelect id="automation-store" v-model="storeId" :nodes="liveNodes" />
        </div>
      </div>

      <div class="grid items-start gap-3 @sm:grid-cols-3">
        <div class="grid gap-1.5">
          <Label for="automation-schedule">{{ t('automation.schedule') }}</Label>
          <Select id="automation-schedule" v-model="scheduleKind">
            <option v-for="kind in SCHEDULE_KINDS" :key="kind" :value="kind">{{ t(`schedule.kind.${kind}`) }}</option>
          </Select>
        </div>
        <div class="grid gap-1.5">
          <template v-if="scheduleKind === 'monthly'">
            <Label for="automation-day">{{ t('schedule.day') }}</Label>
            <Input id="automation-day" v-model="day" type="number" min="1" max="31" />
            <p class="text-xs text-muted-foreground">{{ t('schedule.dayHint') }}</p>
          </template>
          <template v-else-if="scheduleKind === 'weekly'">
            <Label for="automation-weekday">{{ t('schedule.weekdayLabel') }}</Label>
            <Select id="automation-weekday" v-model="weekday">
              <option v-for="entry in WEEKDAYS" :key="entry" :value="String(entry)">{{ t(`schedule.weekday.${entry}`) }}</option>
            </Select>
          </template>
          <template v-else>
            <Label for="automation-days">{{ t('schedule.days') }}</Label>
            <Input id="automation-days" v-model="everyDays" type="number" min="1" />
          </template>
        </div>
        <div class="grid gap-1.5">
          <Label for="automation-start">{{ t('automation.startsAt') }}</Label>
          <Input id="automation-start" v-model="startsAt" type="date" />
        </div>
      </div>

      <label class="flex items-center gap-2 text-sm">
        <input v-model="active" type="checkbox" class="size-4 accent-primary" />
        {{ t('automation.active') }}
      </label>

      <p class="text-xs text-muted-foreground">{{ t('automation.checkHint') }}</p>

      <TransactionForm
        v-if="open && storeId"
        :key="storeId"
        ref="form"
        :store-id="storeId"
        :transaction="template"
        template
        @submit="valid && save.mutate()"
      />
    </div>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button :disabled="!valid || save.isPending.value" @click="save.mutate()">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
