import { type Ref, computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import {
  CRYPTO_ACCOUNTS_KEY,
  CRYPTO_TRANSITION_KEY,
  CONNECTABLE_NETWORKS,
  defaultWalletName,
  isEvmAddress,
  isSecretRefusal,
  isStartDate,
  looksLikeSecret,
  networkLabel,
  useCryptoSyncStatus,
} from '@/entities/crypto-account'
import { call, parseError } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { localDay } from '@/shared/lib/datetime'
import { type FieldError, required, useFormValidation } from '@/shared/lib/form'
import { useSecurityState } from '@/shared/api/security-state'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { openRaiseWizard, useRaiseWizardState } from '@/shared/lib/raise-wizard-state'
import { STORE_TREE_QUERY_KEY } from '@/shared/lib/store-tree'

export type ConnectStep = 'consent' | 'address' | 'start'

/** Whether a start day is today (the user's own day, not UTC) or earlier: a date in the future has no history to read. */
export const notFuture = (date: string) => date <= localDay()

/**
 * The connect-an-address dialog: the one-time consent (asked only until it is given), the network
 * and the public address, then the date the history counts from.
 *
 * **There is no field for a seed phrase or a private key.** The address field runs every change
 * through `looksLikeSecret` before it is kept: a string that looks like one is dropped on the spot,
 * is never assigned to the form state, never sent, never put in a query key and never written to
 * the console, and the field is rebuilt so not even the DOM keeps it.
 */
export function useCryptoAddAddress(open: Ref<boolean>, onCreated: (storeId: string) => void) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const sync = useCryptoSyncStatus()
  const security = useSecurityState()
  const wizard = useRaiseWizardState()
  /** The vault is below level 3 for sure: the move to it runs first (spec E11). Unknown: the backend decides. */
  const needsRaise = computed(() => security.data.value !== undefined && security.data.value.level !== 'crypto')
  /** The wizard is mounted only for a move this dialog started, so it never doubles another screen's. */
  const raising = ref(false)
  /** The move reached level 3 (the backend archived inside it); the address follows once the wizard is closed. */
  const raised = ref(false)
  watch(
    () => wizard.open,
    (value) => {
      if (value) return
      raising.value = false
      // Not before: connecting leaves for the new wallet, and that would take the wizard down at step 5.
      if (raised.value) {
        raised.value = false
        connect.mutate()
      }
    },
  )
  const step = ref<ConnectStep>('address')
  const chain = ref<Chain>('ethereum')
  const address = ref('')
  /** What the user typed; `null` until they touch the field, so the default follows the network and the address. */
  const typedName = ref<string | null>(null)
  /** Names of the top-level wallets: a new one must not repeat any (`STORE_NAME_TAKEN`). */
  const tree = useStoreTree(open)
  const takenNames = computed(() => (tree.data.value ?? []).map((node) => node.store.name))
  const name = computed({
    get: () => typedName.value ?? defaultWalletName(networkLabel(chain.value), address.value, takenNames.value),
    set: (value: string) => {
      typedName.value = value
    },
  })
  const wholeHistory = ref(false)
  const startDate = ref('')
  /** The last paste was refused as a secret. Only the fact is kept, never the text. */
  const refused = ref(false)
  /** Bumped to rebuild the address input after a refusal, so the DOM drops the pasted text too. */
  const addressKey = ref(0)
  const serverError = ref<{ field: 'address' | 'name' | 'other'; message: string } | null>(null)

  watch(open, (value) => {
    if (!value) return
    step.value = sync.data.value?.consent === true ? 'address' : 'consent'
    chain.value = 'ethereum'
    address.value = ''
    typedName.value = null
    wholeHistory.value = false
    startDate.value = `${new Date().getFullYear()}-01-01`
    refused.value = false
    serverError.value = null
  })

  /** The only way text enters the address: through the guard. */
  function setAddress(value: string) {
    serverError.value = null
    if (looksLikeSecret(value)) {
      address.value = ''
      refused.value = true
      addressKey.value += 1
      void nextTick(() => document.getElementById('crypto-address')?.focus())
      return
    }
    refused.value = false
    address.value = value
  }

  const addressRule = (): FieldError => {
    if (refused.value) return t('cryptoTrack.connect.refused')
    const empty = required(address.value)
    if (empty) return empty
    return isEvmAddress(address.value) ? null : t('cryptoTrack.connect.addressInvalid')
  }
  const startRule = (): FieldError => {
    if (wholeHistory.value) return null
    const empty = required(startDate.value)
    if (empty) return empty
    return isStartDate(startDate.value) && notFuture(startDate.value) ? null : t('cryptoTrack.connect.startInvalid')
  }
  const form = useFormValidation(
    () =>
      step.value === 'address' ? { address: addressRule() } : step.value === 'start' ? { start: startRule() } : {},
    { resetOn: [step, open] },
  )

  const consent = useMutation({
    mutationFn: () => call<null>('crypto_set_tracking_consent', { consent: true }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: [...CRYPTO_ACCOUNTS_KEY, 'sync'] })
      step.value = 'address'
    },
    meta: { success: 'cryptoTrack.connect.consentSaved', error: 'cryptoTrack.connect.consentFailed' },
  })

  const connect = useMutation({
    mutationFn: () =>
      call<ConnectAddressResult>('crypto_connect_address', {
        input: {
          chain: chain.value,
          address: address.value.trim(),
          name: name.value.trim(),
          start_date: wholeHistory.value ? undefined : startDate.value,
        } satisfies ConnectAddressInput,
      }),
    onSuccess: (done) => {
      for (const queryKey of [CRYPTO_ACCOUNTS_KEY, ...LEDGER_KEYS, ...PLANNING_KEYS]) {
        void queryClient.invalidateQueries({ queryKey })
      }
      // Start reading at once; a device that does not run the tracking just answers no.
      void call('crypto_sync_now').catch(() => undefined)
      open.value = false
      onCreated(done.store_id)
    },
    onError: (error) => {
      const parsed = parseError(error)
      if (isSecretRefusal(error)) {
        address.value = ''
        refused.value = true
        addressKey.value += 1
        step.value = 'address'
      } else if (parsed.code === 'ALREADY_EXISTS') {
        serverError.value = { field: 'address', message: t('cryptoTrack.connect.alreadyFollowed') }
        step.value = 'address'
      } else if (parsed.code === 'STORE_NAME_TAKEN') {
        serverError.value = { field: 'name', message: t('cryptoTrack.connect.nameTaken') }
        step.value = 'address'
      } else serverError.value = { field: 'other', message: parsed.message }
    },
    // `silent`: the refusal is shown inside the dialog next to the field it concerns, so no toast for it.
    meta: { success: 'cryptoTrack.connect.connected', silent: 'the refusal is shown next to its field in the dialog' },
  })

  /**
   * Level 3 already, no wizard: a vault that got there without the archive (synced from another device,
   * an interrupted move) is finished off before the address is connected. Idempotent: no-op when done.
   */
  const archive = useMutation({
    mutationFn: () => call<CryptoArchiveReport>('crypto_archive_manual_wallets'),
    onSuccess: () => {
      for (const queryKey of [STORE_TREE_QUERY_KEY, CRYPTO_ACCOUNTS_KEY, CRYPTO_TRANSITION_KEY]) {
        void queryClient.invalidateQueries({ queryKey })
      }
    },
    // `silent`: a repair that usually changes nothing; the connection that follows reports its own outcome,
    // and a refusal is shown in the dialog (`startConnect`).
    meta: { silent: 'a quiet repair before the connection, which reports its own outcome' },
  })

  async function startConnect() {
    if (!needsRaise.value) {
      try {
        await archive.mutateAsync()
      } catch (error) {
        serverError.value = { field: 'other', message: parseError(error).message }
        return
      }
    }
    connect.mutate()
  }

  /** Below level 3 the wizard runs first; the address is connected when the move is done. */
  async function startRaise() {
    raising.value = true
    await nextTick()
    openRaiseWizard()
  }

  function next(event?: Event) {
    if (step.value === 'consent') return consent.mutate()
    if (step.value === 'address') return void form.submit(() => (step.value = 'start'), event)
    form.submit(() => (needsRaise.value ? void startRaise() : void startConnect()), event)
  }
  function back() {
    step.value = step.value === 'start' ? 'address' : step.value
  }

  const addressError = computed(() =>
    serverError.value?.field === 'address' ? serverError.value.message : form.error('address'),
  )

  return {
    step,
    chain,
    address,
    name,
    wholeHistory,
    startDate,
    refused,
    addressKey,
    serverError,
    form,
    consent,
    connect,
    raising,
    onRaised: () => (raised.value = true),
    networks: CONNECTABLE_NETWORKS,
    setAddress,
    next,
    back,
    addressError,
  }
}
