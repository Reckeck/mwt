<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { WizardSteps } from '@/shared/ui/wizard-steps'

import type { WizardHandoff, WizardOperationSlot } from '../../automation-edit.types'
import { provideAutomationWizard } from '../../model/use-automation-wizard'
import { WizardDo } from '../wizard-do'
import { WizardWhat } from '../wizard-what'
import { WizardWhen } from '../wizard-when'

/**
 * «Новая автоматизация» (Auto-1 · 6–8) in three steps. Step 2 stays mounted
 * behind step 3, so «Назад» finds the operation as it was typed. The operation
 * form itself comes from the page through `#operation`: features do not import
 * each other.
 */
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ handoff: [to: WizardHandoff]; created: [automation: Automation] }>()
defineSlots<{ operation(props: WizardOperationSlot): unknown }>()

const { t } = useI18n()
const { step, stepNumber, create, goNext, goBack } = provideAutomationWizard(
  open,
  (to) => emit('handoff', to),
  (automation) => emit('created', automation),
)

const titles = computed(() => [t('automationWizard.step.what'), t('automationWizard.step.do'), t('automationWizard.step.when')])
const isLast = computed(() => step.value === 'when')
</script>

<template>
  <Dialog v-model:open="open" :title="t('automationWizard.title')" size="wizard" data-automation-wizard>
    <div class="grid gap-4">
      <WizardSteps :step="stepNumber" :titles="titles" />
      <WizardWhat v-if="step === 'what'" />
      <WizardDo v-if="step !== 'what'" v-show="step === 'do'">
        <template #operation="operation"><slot name="operation" v-bind="operation" /></template>
      </WizardDo>
      <WizardWhen v-if="step === 'when'" />
    </div>

    <template #footer>
      <Button variant="outline" class="max-md:h-12" @click="step === 'what' ? (open = false) : goBack()">
        {{ step === 'what' ? t('actions.cancel') : t('actions.back') }}
      </Button>
      <Button
        class="max-md:h-12 max-md:flex-1"
        :loading="create.isPending.value"
        :data-wizard-next="!isLast || undefined"
        :data-wizard-save="isLast || undefined"
        @click="goNext"
      >
        {{ isLast ? t('automationWizard.create') : t('actions.next') }}
      </Button>
    </template>
  </Dialog>
</template>
