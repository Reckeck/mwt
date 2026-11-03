<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { TransactionForm, formFromTransaction, formToInput } from '@/features/transaction-create'
import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { FormField } from '@/shared/ui/form-field'
import { SyncChangedBanner } from '@/shared/ui/sync-changed-banner'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS, localDate } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { txLegs } from '@/shared/lib/transactions'
import { firstError, integerInRange, required, sameData, useEditBase, useFormValidation } from '@/shared/lib/form'

import type { AutomationDialogProps } from './automation-edit.types'
import { automationBaseOf, automationInputOfFields, fieldsOfAutomation } from './lib/automation-fields'
import { useAutomationFields } from './model/use-automation-fields'
import { AutomationScheduleFields } from './ui/automation-schedule-fields'
import { useStoreTree } from '@/shared/api/queries'
import { SwitchRow } from '@/shared/ui/switch-row'

const props = withDefaults(defineProps<AutomationDialogProps>(), { automation: null, storeId: '', preset: null })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [automation: Automation] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const tree = useStoreTree()
const liveNodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const storeId = ref('')
const {
  name,
  active,
  scheduleKind,
  day,
  weekday,
  everyDays,
  startsAt,
  varies,
  workdayShift,
  load,
  currentFields,
  schedule,
} = useAutomationFields()
/** The property the automation's operations belong to (plan G4). */
const propertyId = ref('')

/** The automation the form shows: the prop at opening, the live record after `Refresh`. */
const source = ref<Automation | null>(null)
/** Changes when the operation form must start over with the (refreshed) template. */
const templateKey = ref(0)
/** The stored template as a transaction the form can prefill from. */
const template = computed(() => (source.value?.template as Transaction | undefined) ?? null)

// Every opening starts from the automation as stored (or blank).
watch(open, (isOpen) => {
  if (!isOpen) return
  const automation = props.automation
  source.value = automation
  // A property takes no money: a preset opened on it books to the everyday wallet and links the property.
  const opener = liveNodes.value.find((node) => node.store.id === props.storeId)?.store
  propertyId.value = automation ? (automation.property_id ?? '') : opener?.kind === 'property' ? opener.id : ''
  storeId.value =
    (template.value && txLegs(template.value)[0]?.store_id) ||
    (propertyId.value && !automation ? '' : props.storeId) ||
    (liveNodes.value.find((node) => node.store.purpose === 'everyday') ?? liveNodes.value[0])?.store.id ||
    ''
  const defaults = fieldsOfAutomation(
    {
      name: props.preset ? t(`preset.${props.preset.preset}`) : '',
      active: true,
      schedule: { monthly: { day: new Date().getDate() } },
      starts_at: '',
      amount_mode: 'fixed',
      // Salaries come on the working day before a day off.
      workday_shift: props.preset?.preset === 'salary' ? 'back' : 'none',
    },
    localDate(new Date()),
  )
  load(automation ? fieldsOfAutomation(automation, localDate(new Date())) : defaults)
})

/** The automation as the sync keeps it now (`['automations']` is re-read after `vault-synced`); the prop may be a stale copy. */
const automations = useQuery({ queryKey: ['automations'], queryFn: () => call<Automation[]>('list_automations'), enabled: open })
const liveAutomation = () =>
  props.automation ? automations.data.value?.find((entry) => entry.id === props.automation?.id) : undefined
const edit = useEditBase({ open, entity: () => props.automation, live: liveAutomation, model: fieldsOfAutomation })
/** The base the core gets: the stored automation written the way this form saves it. */
const baseInput = () => {
  const base = edit.base.value
  if (!base) return null
  const tx = formFromTransaction(base.template as unknown as Transaction, storeId.value)
  if (base.property_id) tx.propertyId = base.property_id
  return automationBaseOf(base, formToInput(tx))
}
/**
 * `Refresh` merges the schedule fields; the operation template is replaced by the other device's
 * only when the user has not touched it (its input still equals the base's).
 */
function refreshFromOtherDevice() {
  const live = liveAutomation()
  const untouched = sameData(form.value?.input, baseInput()?.template)
  edit.refresh(currentFields, load)
  if (live && untouched) {
    source.value = live
    templateKey.value += 1
  }
}

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
    // An exchange has two amounts: there is no one amount to average.
    varies: varies.value && form.value?.input?.type === 'convert' ? t('automation.varies.notExchange') : null,
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
    const input = automationInputOfFields(currentFields(), schedule.value, {
      preset: source.value?.preset ?? props.preset?.preset ?? null,
      time: source.value?.time ?? null,
      asks: source.value?.asks ?? false,
      template: templateInput,
    })
    return props.automation
      ? call<Automation>('update_automation', {
          input: { id: props.automation.id, automation: input, base: baseInput() ?? undefined },
        })
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
  <Dialog overlay="automation-edit"
    v-model:open="open"
    :title="props.automation ? t('automation.edit') : props.preset ? t(`preset.${props.preset.preset}`) : t('automation.new')"
    size="lg"
  >
    <!-- biome-ignore lint/a11y/useSemanticElements: a fieldset would wrap the operation form inside; this only catches Enter from the schedule fields. -->
    <div class="@container grid gap-4" role="group" @keydown.enter="saveOnEnter">
      <SyncChangedBanner v-if="edit.changedElsewhere.value" @refresh="refreshFromOtherDevice" />
      <div class="grid gap-3 @xl:grid-cols-2">
        <FormField for="automation-name" :label="t('automation.name')" required :error="fields.error('name')">
          <Input id="automation-name" v-model="name" :placeholder="t('automation.namePlaceholder')" />
        </FormField>
        <FormField for="automation-store" :label="t('automation.store')" :hint="t('automation.checkHint')">
          <StoreNodeSelect id="automation-store" v-model="storeId" :nodes="liveNodes" />
        </FormField>
      </div>

      <TransactionForm
        v-if="open && storeId"
        :key="`${storeId}:${templateKey}`"
        ref="form"
        :store-id="storeId"
        :transaction="template"
        :initial="initialTx"
        :property-id="propertyId"
        template
        @submit="trySave()"
      />

      <AutomationScheduleFields
        v-model:kind="scheduleKind"
        v-model:day="day"
        v-model:weekday="weekday"
        v-model:every-days="everyDays"
        v-model:starts-at="startsAt"
        v-model:workday-shift="workdayShift"
        :errors="{ day: fields.error('day'), everyDays: fields.error('everyDays'), startsAt: fields.error('startsAt') }"
      />

      <FormField :error="fields.error('varies')">
        <SwitchRow
          id="automation-varies"
          v-model="varies"
          :label="t('automation.varies.label')"
          :hint="t('automation.varies.hint')"
        />
      </FormField>
      <SwitchRow id="automation-active" :label="t('automation.active')" v-model="active" />
    </div>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button :disabled="save.isPending.value" @click="trySave($event)">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
