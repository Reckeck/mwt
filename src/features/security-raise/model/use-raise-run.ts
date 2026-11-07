import { ref, type Ref } from 'vue'
import { useMutation } from '@tanstack/vue-query'

import { call, parseError } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useI18n } from 'vue-i18n'

import type { RaiseFailure } from '../security-raise.types'

interface RaiseRunOptions {
  level: Ref<SecurityLevel>
  pin: Ref<string>
  password: Ref<string>
  /** The run started: show the progress step. */
  onStart: () => void
  /** Done: the vault is at level 3. */
  onDone: (report: RaiseToCryptoReport) => void | Promise<void>
  /** The refusal belongs to a field of step 2 (a wrong PIN, a weak password): back to it, the message under the field. */
  onFieldError: (field: 'pin' | 'password', message: string) => void
}

/**
 * `raise_to_crypto_level` with its progress. A wrong PIN or a weak password goes back to the field it
 * concerns; any other refusal stays on the progress step as a failure that names the stage it stopped at.
 */
export function useRaiseRun(options: RaiseRunOptions) {
  const { t } = useI18n()
  const stage = ref<RaiseStage | null>(null)
  const failure = ref<RaiseFailure | null>(null)
  const report = ref<RaiseToCryptoReport | null>(null)

  useBackendEvent<RaiseProgress>('security-raise-progress', (progress) => {
    stage.value = progress.stage
  })

  const mutation = useMutation({
    mutationFn: () =>
      call<RaiseToCryptoReport>('raise_to_crypto_level', {
        input: {
          // A plain vault has no PIN to ask for: the backend ignores it.
          current_password: options.level.value === 'pin' ? options.pin.value : '',
          new_password: options.password.value,
          // The backup is made next to the vault file by the backend.
          backup_path: '',
        } satisfies RaiseToCryptoInput,
      }),
    onMutate: () => {
      stage.value = null
      failure.value = null
      options.onStart()
    },
    onSuccess: async (result) => {
      report.value = result
      await options.onDone(result)
    },
    onError: (err) => {
      const parsed = parseError(err)
      if (parsed.code === 'SECRET') return options.onFieldError('pin', t('securityRaise.password.wrongPin'))
      if (parsed.code === 'PIN_RETRY_LATER') return options.onFieldError('pin', codeErrorMessage(parsed, t))
      if (parsed.code === 'PASSWORD_TOO_WEAK') return options.onFieldError('password', codeErrorMessage(parsed, t))
      failure.value = { stage: stage.value ?? 'backup', message: codeErrorMessage(parsed, t) }
    },
    meta: { silent: 'the wizard shows the outcome itself: errors under the fields, a failure on the progress step' },
  })

  return { stage, failure, report, mutation }
}
