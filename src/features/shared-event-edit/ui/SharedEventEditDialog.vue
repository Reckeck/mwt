<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { useCreateSharedEvent, useUpdateSharedEvent } from '@/entities/shared-event'
import { DateRangePicker } from '@/shared/ui/date-range-picker'
import { Checkbox } from '@/shared/ui/checkbox'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { Switch } from '@/shared/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { useEventDraft } from '../model/use-event-draft'
import { ROUNDING_STEPS } from '../shared-event-edit.constants'
import type { GuestDetailsResult, SharedEventEditDialogProps } from '../shared-event-edit.types'
import DraftParticipants from './DraftParticipants.vue'
import GuestDetailsDialog from './GuestDetailsDialog.vue'

/**
 * «Новое событие» and «Изменить событие»: title, a day or several days, the currency the shares are
 * counted in, who leads, whether I take part, and — for a new event — the people. A sheet from below on
 * a phone. Saving a new event with people sends them in the same request. The category picker and
 * the requisite form belong to other features: the layer above hands them in (`#category`,
 * `#requisite-form`).
 */
const props = defineProps<SharedEventEditDialogProps>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [view: SharedEventView] }>()

const { t } = useI18n()
const draft = useEventDraft(open, () => props.event)
const create = useCreateSharedEvent()
const update = useUpdateSharedEvent()
const pending = computed(() => create.isPending.value || update.isPending.value)
const isNew = computed(() => !props.event)

/** The guest whose details are being filled in (a draft person of the new event). */
const detailsOf = ref<string | null>(null)
const detailsOpen = computed({
  get: () => detailsOf.value !== null,
  set: (now) => {
    if (!now) detailsOf.value = null
  },
})
const detailsGuest = computed(() => {
  const person = draft.people.value.find((entry) => entry.id === detailsOf.value)
  return person ? { name: person.name, details: person.details } : null
})

function saveGuest(result: GuestDetailsResult) {
  if (detailsOf.value) draft.updateGuest(detailsOf.value, result.name, result.details)
  detailsOf.value = null
}

function done(view: SharedEventView) {
  open.value = false
  emit('saved', view)
}

function save() {
  const body = draft.toDraft()
  if (props.event) update.mutate({ id: props.event.id, event: body }, { onSuccess: done })
  else create.mutate(body, { onSuccess: done })
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="shared-event-edit"
    form-id="shared-event-form"
    :title="isNew ? t('sharedEvents.edit.new') : t('sharedEvents.edit.change')"
    :description="t('sharedEvents.edit.description')"
    :submit-label="isNew ? t('sharedEvents.edit.create') : t('actions.save')"
    :pending="pending"
    @submit="draft.form.submit(save, $event)"
  >
    <FormField for="event-title" :label="t('sharedEvents.edit.title')" required :error="draft.form.error('title')">
      <Input id="event-title" v-model="draft.title.value" autocomplete="off" :placeholder="t('sharedEvents.edit.titlePlaceholder')" />
    </FormField>

    <div class="grid gap-4 @sm:grid-cols-[1fr_11rem]">
      <FormField :label="t('sharedEvents.edit.dates')" :error="draft.form.error('dates')">
        <DateRangePicker
          v-model="draft.dates.value"
          :range="draft.multiDay.value"
          :invalid="Boolean(draft.form.error('dates'))"
          :aria-label="t('sharedEvents.edit.dates')"
        />
      </FormField>
      <CurrencyPicker
        v-model="draft.currency.value"
        :label="t('sharedEvents.edit.currency')"
        required
        :error="draft.form.error('currency')"
      />
    </div>
    <div class="-mt-2 flex items-center gap-2">
      <Switch id="event-multiday" v-model="draft.multiDay.value" />
      <Label for="event-multiday" class="text-sm font-normal">{{ t('sharedEvents.edit.multiDay') }}</Label>
    </div>

    <div class="grid gap-4 @sm:grid-cols-2">
      <FormField :label="t('sharedEvents.edit.step')" :hint="t('sharedEvents.edit.stepHint')">
        <Select v-model="draft.step.value" :aria-label="t('sharedEvents.edit.step')">
          <SelectItem v-for="entry in ROUNDING_STEPS" :key="entry" :value="entry">{{ entry.replace('.', ',') }}</SelectItem>
        </Select>
      </FormField>
      <FormField :label="t('sharedEvents.edit.category')" :hint="t('sharedEvents.edit.categoryHint')">
        <slot name="category" :value="draft.category.value" :update="(key: string) => (draft.category.value = key)" />
      </FormField>
    </div>

    <FormField :label="t('sharedEvents.edit.organizer')" :hint="t('sharedEvents.edit.organizerHint')" :error="draft.form.error('organizer')">
      <div class="grid gap-2">
        <ToggleGroup
          type="single"
          variant="outline"
          :model-value="draft.organizerIsMe.value ? 'me' : 'person'"
          :aria-label="t('sharedEvents.edit.organizer')"
          @update:model-value="(value) => value && (draft.organizerIsMe.value = value === 'me')"
        >
          <ToggleGroupItem value="me">{{ t('sharedEvents.edit.organizerMe') }}</ToggleGroupItem>
          <ToggleGroupItem value="person">{{ t('sharedEvents.edit.organizerPerson') }}</ToggleGroupItem>
        </ToggleGroup>
        <Select
          v-if="!draft.organizerIsMe.value"
          v-model="draft.organizerId.value"
          :aria-label="t('sharedEvents.edit.organizerPerson')"
          :placeholder="t('sharedEvents.edit.organizerPick')"
        >
          <SelectItem v-for="person in draft.candidates.value" :key="person.id" :value="person.id">{{ person.name }}</SelectItem>
        </Select>
      </div>
    </FormField>

    <div class="flex items-start gap-3">
      <Checkbox
        id="event-participate"
        :model-value="draft.iParticipate.value"
        class="mt-0.5"
        @update:model-value="(on) => (draft.iParticipate.value = on === true)"
      />
      <div class="grid gap-0.5">
        <Label for="event-participate" class="font-normal">{{ t('sharedEvents.edit.iParticipate') }}</Label>
        <span class="text-xs text-muted-foreground">{{ t('sharedEvents.edit.iParticipateHint') }}</span>
      </div>
    </div>

    <DraftParticipants
      v-if="isNew"
      :people="draft.people.value"
      :include-me="draft.iParticipate.value"
      @add-contact="draft.addContact"
      @add-guest="draft.addGuest"
      @remove="draft.removePerson"
      @details="(id) => (detailsOf = id)"
    />
    <p v-else class="text-xs text-muted-foreground">{{ t('sharedEvents.edit.peopleElsewhere') }}</p>

    <GuestDetailsDialog
      v-model:open="detailsOpen"
      :guest="detailsGuest"
      :event-label="t('sharedEvents.guestForm.ofEvent', { title: draft.title.value || t('sharedEvents.edit.new') })"
      @save="saveGuest"
    >
      <template #requisite-form="slotProps"><slot name="requisite-form" v-bind="slotProps" /></template>
    </GuestDetailsDialog>
  </FormDialog>
</template>
