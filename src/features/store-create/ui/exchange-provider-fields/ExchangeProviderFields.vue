<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { cn } from '@/shared/lib/utils'

import { useStoreCreate } from '../../model/use-store-create-state'
import { PICK_CARD_CLASS } from '../../store-create.constants'

/** An exchange: which one, and an optional read-only key that sets its accounts up right away. */
const { t } = useI18n()
const { catalog, provider, apiKey, apiSecret, form } = useStoreCreate()
</script>

<template>
  <FormField :label="t('provider.title')" required :error="form.error('provider')">
    <div class="grid gap-2 @sm:grid-cols-3">
      <Button
        v-for="info in catalog.data.value?.providers ?? []"
        :key="info.provider"
        variant="outline"
        :disabled="!info.supported"
        :aria-pressed="provider === info.provider"
        :class="cn(PICK_CARD_CLASS, 'items-center justify-between', provider === info.provider && 'border-primary bg-primary/5')"
        @click="provider = info.provider"
      >
        <span class="text-sm font-medium">{{ t(`provider.${info.provider}`) }}</span>
        <span v-if="!info.supported" class="text-xs font-normal text-muted-foreground">{{ t('provider.soon') }}</span>
      </Button>
    </div>
  </FormField>

  <fieldset v-if="provider === 'bybit'" class="grid gap-3" data-tour="store-bybit-key">
    <p class="text-xs text-muted-foreground">{{ t('storeWizard.bybitKeyHint') }}</p>
    <div class="grid gap-3 @sm:grid-cols-2">
      <FormField for="store-bybit-key" :label="t('wallet.bybit.apiKey')" :error="form.error('apiKey')">
        <Input id="store-bybit-key" v-model="apiKey" autocomplete="off" spellcheck="false" />
      </FormField>
      <FormField for="store-bybit-secret" :label="t('wallet.bybit.apiSecret')" :error="form.error('apiSecret')">
        <Input id="store-bybit-secret" v-model="apiSecret" type="password" autocomplete="new-password" spellcheck="false" />
      </FormField>
    </div>
  </fieldset>
</template>
