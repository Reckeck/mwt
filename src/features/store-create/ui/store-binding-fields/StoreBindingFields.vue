<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AssetInput } from '@/entities/asset'
import { CurrencyPicker } from '@/entities/currency'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import { useStoreCreate } from '../../model/use-store-create-state'

/** What a sub-account is locked to: one asset (a currency or a token) or a pair. */
const { t } = useI18n()
const { props, subInfo, token, currency, pairText, pair, form } = useStoreCreate()
</script>

<template>
  <template v-if="subInfo">
    <FormField
      v-if="subInfo.shape === 'asset'"
      :for="props.parent?.kind === 'crypto' ? undefined : 'store-binding-currency'"
      :label="props.parent?.kind === 'crypto' ? t('binding.token') : t('binding.currency')"
      required
      :error="form.error(props.parent?.kind === 'crypto' ? 'token' : 'currency')"
    >
      <AssetInput v-if="props.parent?.kind === 'crypto'" v-model="token" />
      <CurrencyPicker v-else id="store-binding-currency" v-model="currency" />
    </FormField>
    <FormField
      v-else-if="subInfo.shape === 'pair'"
      for="store-binding-pair"
      :label="t('binding.pair')"
      required
      :error="form.error('pair')"
      :hint="pair ? t('binding.pairParsed', pair) : pairText ? t('binding.pairInvalid') : t('subTypeHint.pair')"
    >
      <Input id="store-binding-pair" v-model="pairText" :placeholder="t('binding.pairPlaceholder')" autocomplete="off" />
    </FormField>
    <p v-else class="text-sm text-muted-foreground">{{ t(`subTypeHint.${subInfo.sub_type}`) }}</p>
  </template>
</template>
