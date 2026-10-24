<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'

import { AutomationWizard, SalaryDialog, SaveShareDialog, type WizardHandoff } from '@/features/automation-edit'
import { PriceAlertDialog } from '@/features/price-alert-edit'
import { ReminderDialog } from '@/features/reminder-edit'
import { TransactionForm } from '@/features/transaction-create'
import { StoreNodeSelect } from '@/entities/store'
import { FormField } from '@/shared/ui/form-field'

/**
 * `?new=blank` opens «Новая автоматизация» (Auto-1 · 6–8). What the wizard
 * does not make itself opens in its own form: a reminder, a price alert, a
 * scheme on the canvas, a ready-made start by its `?new=`. Step 2 gets the
 * short operation form from here: the wizard's feature cannot import it.
 */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const wizardOpen = computed({
  get: () => route.query.new === 'blank',
  set: (open: boolean) => {
    if (!open && route.query.new === 'blank') void router.replace({ query: { ...route.query, new: undefined } })
  },
})
const reminderOpen = ref(false)
const alertOpen = ref(false)
const salaryOpen = ref(false)
const shareOpen = ref(false)

/**
 * `?new=reminder`, `alert`, `salary` and `save_share` open their own forms straight
 * away (links from other screens); `low_balance` is a scheme from its template.
 */
watch(
  () => route.query.new,
  (kind) => {
    if (kind === 'low_balance') return void router.replace({ name: 'workflow-new', query: { template: 'lowBalance' } })
    const opens = { reminder: reminderOpen, alert: alertOpen, salary: salaryOpen, save_share: shareOpen }[String(kind)]
    if (!opens) return
    opens.value = true
    void router.replace({ query: { ...route.query, new: undefined } })
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
      <TransactionForm :key="formKey" :ref="bind" compact template :store-id="storeId" :initial="initial" @submit="submit">
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
      </TransactionForm>
    </template>
  </AutomationWizard>
  <ReminderDialog v-model:open="reminderOpen" />
  <PriceAlertDialog v-model:open="alertOpen" />
  <SalaryDialog v-model:open="salaryOpen" />
  <SaveShareDialog v-model:open="shareOpen" />
</template>
