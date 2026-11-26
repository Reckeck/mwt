<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { call } from '@/shared/api/ipc'
import { useContacts } from '@/shared/api/queries'

import { automationLabel } from '../../model/automation-label'
import { useUtilityForm } from '../../model/use-utility-form'

/**
 * Step 2 of the rent (Wallets-3 · 17): the month's amount and the deposit, the
 * day it is due, and the automation that pays it — then the bill leaves it out.
 * A fixed item asks for its amount only.
 */
const { t } = useI18n()
const { kind, amount, deposit, dueDay, automationId, currency, nodes, form } = useUtilityForm()
const rent = computed(() => kind.value === 'housing')

const automations = useQuery({ queryKey: ['automations'], queryFn: () => call<Automation[]>('list_automations'), enabled: rent })
const contacts = useContacts(rent)
const storeName = (id: string) => nodes.value.find((node) => node.store.id === id)?.store.name
const contactName = (id: string) => contacts.data.value?.find((contact) => contact.id === id)?.name
const options = computed(() =>
  (automations.data.value ?? [])
    .filter((entry) => entry.active || entry.id === automationId.value)
    .map((entry) => ({ id: entry.id, label: automationLabel(entry, t, storeName, contactName) })),
)
const code = computed(() => currency.value.toUpperCase())
</script>

<template>
  <FormField for="utility-amount" :label="rent ? t('utilities.rent') : t('utilityEdit.monthly')" required :error="form.error('amount')">
    <AmountInput id="utility-amount" v-model="amount">
      <template #suffix><Badge variant="secondary" class="font-normal">{{ code }}</Badge></template>
    </AmountInput>
    <p v-if="!rent" class="text-xs text-muted-foreground">{{ t('utilityEdit.staticHint') }}</p>
  </FormField>
  <template v-if="rent">
    <div class="grid items-start gap-3 @lg:grid-cols-2">
      <FormField for="utility-deposit" :label="t('utilities.deposit')" :error="form.error('deposit')">
        <AmountInput id="utility-deposit" v-model="deposit">
          <template #suffix><Badge variant="secondary" class="font-normal">{{ code }}</Badge></template>
        </AmountInput>
        <p class="text-xs text-muted-foreground">{{ t('utilityEdit.depositHint') }}</p>
      </FormField>
      <FormField for="utility-due" :label="t('utilities.dueDay')" required :error="form.error('dueDay')">
        <Input id="utility-due" v-model="dueDay" inputmode="numeric" />
      </FormField>
    </div>
    <div class="grid gap-1.5">
      <Label for="utility-automation">{{ t('utilityEdit.automation') }}</Label>
      <Select id="utility-automation" v-model="automationId">
        <SelectItem value="">{{ t('utilities.noAutomation') }}</SelectItem>
        <SelectItem v-for="entry in options" :key="entry.id" :value="entry.id">{{ entry.label }}</SelectItem>
      </Select>
      <p class="text-xs text-muted-foreground">{{ t('utilityEdit.automationHint') }}</p>
    </div>
  </template>
</template>
