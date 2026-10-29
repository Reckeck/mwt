<script setup lang="ts">
import type { Component } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowLeftRight, Bell, Repeat2 } from 'lucide-vue-next'

import { WIZARD_ACTIONS, type WizardAction } from '../../automation-edit.constants'
import type { WizardOperationSlot } from '../../automation-edit.types'
import { useAutomationWizard } from '../../model/use-automation-wizard'
import { WizardOption } from '../wizard-option'

/**
 * Step 2 (Auto-1 · 7): what it does. `Record a transaction` is the short form of
 * an operation — the same tabs as Ctrl N, put in by the page through
 * `#operation`; `Remind` and `Transfer a share` open their own forms.
 */
const { t } = useI18n()
defineSlots<{ operation(props: WizardOperationSlot): unknown }>()
const { storeId, nodes, initial, form, formKey, changeStore, goNext, handOff } = useAutomationWizard()

/** The form is the wizard's: step 3 reads what was typed from it. */
const bindForm = (element: unknown) => {
  form.value = element as typeof form.value
}
const ICONS: Record<WizardAction, Component> = { operation: Repeat2, remind: Bell, share: ArrowLeftRight }

function pick(action: WizardAction) {
  if (action === 'remind') handOff({ kind: 'reminder' })
  else if (action === 'share') handOff({ kind: 'starter', starter: 'save_share' })
}
</script>

<template>
  <div class="@container grid gap-4" data-wizard-do>
    <div class="grid gap-3 md:grid-cols-3">
      <WizardOption
        v-for="entry in WIZARD_ACTIONS"
        :key="entry"
        :icon="ICONS[entry]"
        :title="t(`automationWizard.action.${entry}.title`)"
        :hint="t(`automationWizard.action.${entry}.hint`)"
        :selected="entry === 'operation'"
        :data-wizard-action="entry"
        @click="pick(entry)"
      />
    </div>
    <div class="grid gap-2">
      <p class="text-sm font-medium">{{ t('automationWizard.operation') }}</p>
      <slot
        v-if="storeId"
        name="operation"
        :store-id="storeId"
        :nodes="nodes"
        :initial="initial"
        :form-key="formKey"
        :bind="bindForm"
        :submit="goNext"
        :change-store="changeStore"
      />
    </div>
  </div>
</template>
