<script setup lang="ts">
import { useQuery } from '@tanstack/vue-query'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'

import { StoreNodeSelect } from '@/entities/store'
import { AutomationDialog, AutomationWizard, SalaryDialog, SaveShareDialog, TemplateFields, type WizardHandoff } from '@/features/automation-edit'
import { PriceAlertDialog } from '@/features/price-alert-edit'
import { ReminderDialog } from '@/features/reminder-edit'
import { AutomationDetailsSheet } from '@/widgets/automation-details'
import { call } from '@/shared/api/ipc'
import { FormField } from '@/shared/ui/form-field'

/**
 * `?new=blank` opens `New automation`. What the wizard does not make itself opens in its own
 * form: a reminder, a price alert, a scheme on the canvas, a ready-made start by its `?new=`.
 * `?edit=<id>` opens an automation's form, `?card=<id>` its card. Step 2 gets the operation
 * form from here: the wizard's feature cannot import it.
 */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()

/** Drops one address key, keeping the rest. */
const without = (key: string) => void router.replace({ query: { ...route.query, [key]: undefined } })

const wizardOpen = computed({
  get: () => route.query.new === 'blank',
  set: (open: boolean) => {
    if (!open && route.query.new === 'blank') without('new')
  },
})

const automations = useQuery({ queryKey: ['automations'], queryFn: () => call<Automation[]>('list_automations') })
const editing = ref<Automation | null>(null)
watch(
  () => [route.query.edit, automations.data.value] as const,
  ([id, list]) => {
    const found = typeof id === 'string' ? list?.find((entry) => entry.id === id) : undefined
    if (found) editing.value = found
  },
  { immediate: true },
)
const editOpen = computed({
  get: () => typeof route.query.edit === 'string' && editing.value?.id === route.query.edit,
  set: (open: boolean) => {
    if (!open && route.query.edit !== undefined) without('edit')
  },
})

const cardId = computed(() => (typeof route.query.card === 'string' ? route.query.card : null))
const cardOpen = computed({
  get: () => cardId.value !== null,
  set: (open: boolean) => {
    if (!open && cardId.value) without('card')
  },
})

const reminderOpen = ref(false)
const alertOpen = ref(false)
const salaryOpen = ref(false)
const shareOpen = ref(false)

/** `?new=reminder`, `alert`, `salary` and `save_share` open their own forms at once; `low_balance` is a scheme from its template. */
watch(
  () => route.query.new,
  (kind) => {
    if (kind === 'low_balance') return void router.replace({ name: 'workflow-new', query: { template: 'lowBalance' } })
    const opens = { reminder: reminderOpen, alert: alertOpen, salary: salaryOpen, save_share: shareOpen }[String(kind)]
    if (!opens) return
    opens.value = true
    without('new')
  },
  { immediate: true },
)

function handoff(to: WizardHandoff) {
  if (to.kind === 'reminder') reminderOpen.value = true
  else if (to.kind === 'alert') alertOpen.value = true
  else if (to.kind === 'scheme') void router.push({ name: 'workflow-new' })
  else void router.replace({ query: { ...route.query, new: to.starter } })
}
</script>

<template>
  <AutomationWizard v-model:open="wizardOpen" @handoff="handoff">
    <template #operation="{ storeId, nodes, initial, formKey, bind, submit, changeStore }">
      <TemplateFields :key="formKey" :ref="bind" :store-id="storeId" :initial="initial" @submit="submit">
        <template #store>
          <FormField for="automation-wizard-store" :label="t('quickAdd.store')" required>
            <StoreNodeSelect
              id="automation-wizard-store"
              :model-value="storeId"
              :nodes="nodes"
              class="w-full"
              @update:model-value="(id) => changeStore(String(id))"
            />
          </FormField>
        </template>
      </TemplateFields>
    </template>
  </AutomationWizard>
  <AutomationDialog v-model:open="editOpen" :automation="editing" />
  <AutomationDetailsSheet v-model:open="cardOpen" :automation-id="cardId" />
  <ReminderDialog v-model:open="reminderOpen" />
  <PriceAlertDialog v-model:open="alertOpen" />
  <SalaryDialog v-model:open="salaryOpen" />
  <SaveShareDialog v-model:open="shareOpen" />
</template>
