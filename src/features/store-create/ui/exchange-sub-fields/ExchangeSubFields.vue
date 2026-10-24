<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Check } from 'lucide-vue-next'

import { AssetSelect } from '@/entities/asset'
import { tickerRef } from '@/shared/lib/asset-ref'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import type { ExchangeSubAim } from '../../model/use-exchange-sub'
import { useStoreCreate } from '../../model/use-store-create-state'

/**
 * An exchange's sub-account (Wallets-2 · 9): the name, what it is for, the
 * coin its margin is in — the exchange itself is known already.
 */
const { t } = useI18n()
const { props, name, nameTouched, aim, margin, withMargin, form } = useStoreCreate()
const AIMS: ExchangeSubAim[] = ['futures', 'earn', 'other']
/** The coins an exchange keeps futures margin in. */
const MARGIN_COINS = ['USDT', 'USDC', 'BTC', 'ETH'].map(tickerRef)

function pick(value: unknown) {
  if (value) aim.value = value as ExchangeSubAim
}
</script>

<template>
  <div class="grid gap-4" data-exchange-sub>
    <FormField for="store-name" :label="t('stores.name')" required :error="form.error('name')">
      <Input id="store-name" v-model="name" @input="nameTouched = true" />
    </FormField>
    <FormField :label="t('exchangeSub.aim')">
      <div>
        <ToggleGroup
          :model-value="aim"
          type="single"
          class="rounded-[10px] bg-muted p-0.75"
          :aria-label="t('exchangeSub.aim')"
          @update:model-value="pick"
        >
          <ToggleGroupItem
            v-for="entry in AIMS"
            :key="entry"
            :value="entry"
            class="h-8 rounded-lg px-2.5 text-sm text-muted-foreground data-[state=on]:bg-background data-[state=on]:text-foreground data-[state=on]:shadow-sm"
          >
            {{ t(`exchangeSub.${entry}`) }}
          </ToggleGroupItem>
        </ToggleGroup>
      </div>
    </FormField>
    <FormField v-if="withMargin" :label="t('exchangeSub.margin')" required :error="form.error('margin')">
      <AssetSelect v-model="margin" :assets="MARGIN_COINS" :invalid="Boolean(form.error('margin'))" />
    </FormField>
    <p class="flex gap-2.5 rounded-lg bg-muted px-3 py-2.5 text-caption">
      <Check class="mt-0.5 size-3.5 shrink-0" />
      {{ t('exchangeSub.hint', { exchange: props.parent?.name ?? '' }) }}
    </p>
  </div>
</template>
