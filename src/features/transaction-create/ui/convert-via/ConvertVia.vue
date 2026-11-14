<script setup lang="ts">
import { X } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'

import { useTxForm } from '../../model/use-tx-form'
import { AccountRefPicker } from '../account-ref-picker'

/**
 * The optional "through whom" of an exchange (a P2P seller, an exchanger): a quiet link
 * that opens the account picker, and a small cross that takes it away again.
 */
const { t } = useI18n()
const { form, check, openVia, closeVia } = useTxForm()
</script>

<template>
  <Button
    v-if="!form.viaOpen"
    type="button"
    variant="link"
    class="touch-min h-auto self-start px-0 py-0"
    data-tour="tx-via"
    data-tx-add-via
    @click="openVia"
  >{{ t('tx.p2p.add') }}</Button>
  <FormField v-else class="mt-2" :label="t('tx.p2p.via')" required :error="check.error('via')" :hint="t('tx.p2p.hint')">
    <div class="flex items-start gap-2">
      <div class="min-w-0 flex-1"><AccountRefPicker v-model="form.via" :stores="false" /></div>
      <Button type="button" variant="ghost" size="icon" :aria-label="t('tx.p2p.remove')" data-tx-remove-via @click="closeVia">
        <X class="size-4" />
      </Button>
    </div>
  </FormField>
</template>
