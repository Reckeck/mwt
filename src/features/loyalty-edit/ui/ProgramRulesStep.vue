<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { StoreNodeSelect } from '@/entities/store'
import { AmountInput } from '@/shared/ui/amount-input'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { useContacts } from '@/shared/api/queries'

import type { LoyaltyInfoState } from '../model/use-loyalty-info'
import type { ProgramField } from '../loyalty-edit.types'

/**
 * Step 2 of «Новая программа» (Ops-2 · 13, «Как начисляются?»): all optional —
 * the ticker made from the name, the other cards that earn, the login and what burns next.
 */
const props = defineProps<{
  state: LoyaltyInfoState
  error: (field: ProgramField) => string | undefined
  /** The wallet of step 1: it already earns, so it is not offered again. */
  wallet: string
}>()
const contactId = defineModel<string>('contactId', { required: true })
const { t } = useI18n()
const contacts = useContacts()
const others = computed(() => props.state.cards.value.filter((node) => node.store.id !== props.wallet))
</script>

<template>
  <div class="@container grid gap-4">
    <div class="grid gap-4 @sm:grid-cols-2">
      <FormField for="loyalty-symbol" :label="t('loyalty.symbol')" required :error="props.error('symbol')">
        <!-- The name of a programme's points, not a listed asset. -->
        <Input id="loyalty-symbol" v-model="props.state.symbol.value" class="uppercase" maxlength="16" data-picker-ok />
        <p class="text-xs text-muted-foreground">{{ t('bonuses.symbolHint') }}</p>
      </FormField>
      <FormField for="loyalty-sources" :label="t('bonuses.moreCards')">
        <StoreNodeSelect
          id="loyalty-sources"
          v-model:ids="props.state.sources.value"
          :nodes="others"
          flat
          multiple
          :empty-label="t('loyalty.noSource')"
        />
      </FormField>
      <FormField for="loyalty-login" :label="t('loyalty.login')">
        <Input id="loyalty-login" v-model="props.state.login.value" maxlength="64" />
        <p class="text-xs text-muted-foreground">{{ t('loyalty.loginHint') }}</p>
      </FormField>
      <FormField for="program-contact" :label="t('stores.contact')">
        <Select id="program-contact" v-model="contactId" :aria-label="t('stores.contact')">
          <SelectItem value="">{{ t('stores.contactNone') }}</SelectItem>
          <SelectItem
            v-for="contact in (contacts.data.value ?? []).filter((entry) => !entry.archived)"
            :key="contact.id"
            :value="contact.id"
          >
            {{ contact.name }}
          </SelectItem>
        </Select>
      </FormField>
      <FormField for="loyalty-expires" :label="t('loyalty.expiresAt')">
        <DateTimePicker id="loyalty-expires" v-model="props.state.expiresAt.value" />
      </FormField>
      <FormField for="loyalty-expiring" :label="t('loyalty.expiringPoints')" :error="props.error('expiring')">
        <AmountInput id="loyalty-expiring" v-model="props.state.expiring.value" />
      </FormField>
    </div>
  </div>
</template>
