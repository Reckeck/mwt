<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { TriangleAlert } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { useFormValidation } from '@/shared/lib/form'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import type { CryptoWipeDialogProps } from './crypto-wipe-dialog.types'

/**
 * The confirmation of a wipe: what goes, in numbers, and the word that has to be typed for it.
 *
 * The word is the whole point of the window — a wipe cannot be taken back, only restored from the
 * copy it makes first — so the button stays active and says what is wrong under the field (rule 13),
 * and the word itself is translated: what reaches the vault is the fixed token.
 */
const props = defineProps<CryptoWipeDialogProps>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ confirm: [] }>()

const { t } = useI18n()
const typed = ref('')
const word = computed(() => t('cryptoTrack.wipe.word'))
const matches = computed(() => typed.value.trim().toLocaleUpperCase() === word.value.toLocaleUpperCase())

const form = useFormValidation(
  () => ({ word: matches.value ? null : t('cryptoTrack.wipe.wordError', { word: word.value }) }),
  { resetOn: open },
)

const counts = computed(() => props.preview.counts)
</script>

<template>
  <Dialog
    v-model:open="open"
    overlay="crypto-wipe"
    size="confirm"
    :title="t('cryptoTrack.wipe.askTitle')"
    :description="t('cryptoTrack.wipe.askLead')"
  >
    <form id="crypto-wipe-form" class="flex flex-col gap-4" @submit.prevent="form.submit(() => emit('confirm'), $event)">
      <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
        <dt class="text-muted-foreground">{{ t('cryptoTrack.wipe.rowAccounts') }}</dt>
        <dd class="text-right tabular-nums">{{ counts.accounts }}</dd>
        <dt class="text-muted-foreground">{{ t('cryptoTrack.wipe.rowEvents') }}</dt>
        <dd class="text-right tabular-nums">{{ counts.events }}</dd>
        <dt class="text-muted-foreground">{{ t('cryptoTrack.wipe.rowTokens') }}</dt>
        <dd class="text-right tabular-nums">{{ counts.tokens }}</dd>
        <dt class="text-muted-foreground">{{ t('cryptoTrack.wipe.rowWallets') }}</dt>
        <dd class="text-right tabular-nums">{{ props.preview.wallets.length }}</dd>
        <dt class="text-muted-foreground">{{ t('cryptoTrack.wipe.rowCapital') }}</dt>
        <dd class="text-right">
          <Money :amount="props.preview.value_delta" :currency="props.preview.base_currency" colored />
        </dd>
      </dl>

      <ul class="flex flex-col gap-1 text-sm text-muted-foreground">
        <li>{{ t('cryptoTrack.wipe.walletsStay') }}</li>
        <li>{{ t('cryptoTrack.wipe.backupFirst') }}</li>
        <li v-if="props.preview.device_secrets">{{ t('cryptoTrack.wipe.secretsGo') }}</li>
        <li>{{ t('cryptoTrack.wipe.biometricGoes') }}</li>
      </ul>

      <Alert variant="destructive">
        <TriangleAlert />
        <AlertTitle>{{ t('cryptoTrack.wipe.noUndoTitle') }}</AlertTitle>
        <AlertDescription>{{ t('cryptoTrack.wipe.noUndoBody') }}</AlertDescription>
      </Alert>

      <FormField :label="t('cryptoTrack.wipe.wordLabel', { word })" for="crypto-wipe-word" required :error="form.error('word')">
        <!-- biome-ignore lint/a11y/noAutofocus: the word is the only thing this window asks for. -->
        <Input id="crypto-wipe-word" v-model="typed" autofocus autocomplete="off" :placeholder="word" />
      </FormField>
    </form>

    <template #footer>
      <Button variant="outline" type="button" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button
        form="crypto-wipe-form"
        type="submit"
        variant="destructive"
        :loading="props.pending"
        :disabled="props.pending"
      >
        {{ t('cryptoTrack.wipe.confirmAction') }}
      </Button>
    </template>
  </Dialog>
</template>
