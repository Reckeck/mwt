<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueryClient } from '@tanstack/vue-query'

import { SECURITY_STATE_QUERY_KEY, useSecurityState } from '@/shared/api/security-state'
import { useRaiseWizardState } from '@/shared/lib/raise-wizard-state'
import { cryptoPasswordOk, passwordStrength } from '@/shared/lib/vault-password'
import { firstError, required, useFormValidation } from '@/shared/lib/form'
import { useVaultStore } from '@/shared/store/vault'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { WizardSteps } from '@/shared/ui/wizard-steps'

import { useRaiseRun } from '../model/use-raise-run'
import {
  RAISE_STEPS,
  STEP_BACKUP,
  STEP_DONE,
  STEP_PASSWORD,
  STEP_RUN,
  STEP_WARNING,
} from '../security-raise.types'
import RaiseBackupStep from './RaiseBackupStep.vue'
import RaiseDoneStep from './RaiseDoneStep.vue'
import RaiseInterruptDialog from './RaiseInterruptDialog.vue'
import RaisePasswordStep from './RaisePasswordStep.vue'
import RaiseRunStep from './RaiseRunStep.vue'
import RaiseWarningStep from './RaiseWarningStep.vue'

/**
 * The move to level 3 as five steps (Security wizard 1): what changes, the new password, the backup,
 * the re-encryption with its stages, the result. It is a modal on a desktop and a bottom sheet on a
 * phone (`Dialog`). Closing before the re-encryption asks first; closing never loses data, because
 * nothing is written until step 4. A new run after an interruption starts at step 2.
 */
const emit = defineEmits<{ raised: [] }>()

const { t } = useI18n()
const wizard = useRaiseWizardState()
const vault = useVaultStore()
const queryClient = useQueryClient()
const security = useSecurityState()

/** Only a vault with a PIN asks for it; a plain one (level 1) goes straight to the new password. */
const level = computed<SecurityLevel>(() => (security.data.value?.level === 'ledger' ? 'ledger' : 'pin'))

const step = ref(STEP_WARNING)
const restarted = ref(false)
const confirmOpen = ref(false)
const pin = ref('')
const password = ref('')
const repeat = ref('')
/** The backend's refusals of step 2, shown under the field until it is edited. */
const serverErrors = ref<{ pin?: string; password?: string }>({})
watch(pin, () => (serverErrors.value = { ...serverErrors.value, pin: undefined }))
watch(password, () => (serverErrors.value = { ...serverErrors.value, password: undefined }))

const form = useFormValidation(
  () => {
    const report = passwordStrength(password.value)
    return {
      pin: level.value === 'pin' ? (required(pin.value) ?? serverErrors.value.pin ?? null) : null,
      password: firstError(
        required(password.value),
        report.long_enough ? null : t('securityRaise.password.tooShort'),
        cryptoPasswordOk(password.value) ? null : t('securityRaise.password.tooWeak'),
        serverErrors.value.password ?? null,
      ),
      repeat: required(repeat.value) ?? (repeat.value === password.value ? null : t('securityRaise.password.mismatch')),
    }
  },
  { resetOn: step },
)
const fieldErrors = computed(() => ({
  pin: form.error('pin'),
  password: form.error('password'),
  repeat: form.error('repeat'),
}))

function clearSecrets() {
  pin.value = ''
  password.value = ''
  repeat.value = ''
  serverErrors.value = {}
}

const { stage, failure, report, mutation } = useRaiseRun({
  level,
  pin,
  password,
  onStart: () => {
    step.value = STEP_RUN
  },
  onDone: async () => {
    clearSecrets()
    // The label «no password», the lock and the key state follow the new level.
    await vault.refreshStatus().catch(() => {})
    await queryClient.invalidateQueries({ queryKey: [SECURITY_STATE_QUERY_KEY] })
    step.value = STEP_DONE
    emit('raised')
  },
  onFieldError: async (field, message) => {
    serverErrors.value = { [field]: message }
    step.value = STEP_PASSWORD
    // Pressing the button once more shows the message under the field and puts the focus there.
    await nextTick()
    form.submit(() => {})
  },
})

const running = computed(() => step.value === STEP_RUN && !failure.value)

watch(
  () => wizard.open,
  (open) => {
    if (!open) return
    void security.refetch()
    restarted.value = wizard.interrupted
    step.value = wizard.interrupted ? STEP_PASSWORD : STEP_WARNING
    clearSecrets()
    failure.value = null
  },
)

/** The close button, Esc, the swipe and the system «Back» all arrive here. */
function requestClose(open: boolean) {
  if (open || running.value) return
  if (step.value < STEP_RUN) confirmOpen.value = true
  else finish()
}

function finish() {
  wizard.open = false
  wizard.interrupted = step.value !== STEP_DONE
  clearSecrets()
}

function stop() {
  confirmOpen.value = false
  finish()
}

function startRun() {
  mutation.mutate()
}

/** After a failure: the passwords are typed again, so the move restarts at step 2. */
function retry() {
  restarted.value = true
  failure.value = null
  clearSecrets()
  step.value = STEP_PASSWORD
}
</script>

<template>
  <Dialog
    overlay="security-raise"
    size="form"
    :open="wizard.open"
    :title="t('securityRaise.title')"
    @update:open="requestClose"
  >
    <div class="grid gap-4" data-raise-wizard :data-step="step">
      <WizardSteps :step="step" :titles="RAISE_STEPS.map((name) => t(`securityRaise.step.${name}`))" />
      <RaiseWarningStep v-if="step === STEP_WARNING" :level="level" />
      <form
        v-else-if="step === STEP_PASSWORD"
        id="raise-password-form"
        @submit.prevent="form.submit(() => (step = STEP_BACKUP), $event)"
      >
        <RaisePasswordStep
          v-model:pin="pin"
          v-model:password="password"
          v-model:repeat="repeat"
          :level="level"
          :restarted="restarted"
          :errors="fieldErrors"
        />
      </form>
      <RaiseBackupStep v-else-if="step === STEP_BACKUP" :level="level" />
      <RaiseRunStep v-else-if="step === STEP_RUN" :level="level" :stage="stage" :failure="failure" />
      <RaiseDoneStep
        v-else
        :minutes="security.data.value?.autolock.minutes ?? null"
        :backup-path="report?.backup_path ?? ''"
      />
    </div>

    <template #footer>
      <template v-if="step === STEP_WARNING">
        <Button variant="ghost" type="button" @click="requestClose(false)">{{ t('actions.cancel') }}</Button>
        <Button type="button" data-raise-next @click="step = STEP_PASSWORD">{{ t('securityRaise.next') }}</Button>
      </template>
      <template v-else-if="step === STEP_PASSWORD">
        <Button variant="ghost" type="button" @click="step = STEP_WARNING">{{ t('securityRaise.back') }}</Button>
        <Button type="submit" form="raise-password-form" data-raise-next>{{ t('securityRaise.next') }}</Button>
      </template>
      <template v-else-if="step === STEP_BACKUP">
        <Button variant="ghost" type="button" @click="step = STEP_PASSWORD">{{ t('securityRaise.back') }}</Button>
        <Button type="button" data-raise-start @click="startRun">{{ t('securityRaise.start') }}</Button>
      </template>
      <template v-else-if="running">
        <Button variant="ghost" type="button" disabled>{{ t('securityRaise.back') }}</Button>
        <Button type="button" disabled loading>{{ t('securityRaise.start') }}</Button>
      </template>
      <template v-else-if="step === STEP_RUN">
        <Button variant="ghost" type="button" @click="requestClose(false)">{{ t('securityRaise.close') }}</Button>
        <Button type="button" data-raise-retry @click="retry">{{ t('securityRaise.retry') }}</Button>
      </template>
      <Button v-else type="button" data-raise-finish @click="requestClose(false)">{{ t('securityRaise.finish') }}</Button>
    </template>
  </Dialog>
  <RaiseInterruptDialog v-model:open="confirmOpen" @stop="stop" />
</template>
