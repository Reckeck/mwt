<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ArrowRightLeft } from 'lucide-vue-next'

import { ContactArchive, ContactEditor } from '@/features/contact-edit'
import { DebtPanel } from '@/features/debt-manage'
import { ContactTurnoverTable } from '@/entities/contact'
import { TransactionsList } from '@/entities/transaction'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'
import { Tabs } from '@/shared/ui/tabs'
import { call } from '@/shared/api/ipc'
import { txDebt } from '@/shared/lib/transactions'
import { emptyTxFilter } from '@/shared/lib/tx-filter'

/** One contact: its card to edit, and its debts, history and turnover. */
const props = defineProps<{ contact: Contact; balances: ContactBalance[]; canTransfer: boolean }>()
const emit = defineEmits<{ transfer: [effect: DebtEffect | null]; deleted: [] }>()

const { t } = useI18n()

const ownBalances = computed(() => props.balances.filter((row) => row.contact_id === props.contact.id))

const turnover = useQuery({
  queryKey: computed(() => ['contacts', 'turnover', props.contact.id]),
  queryFn: () => call<ContactTurnover>('get_contact_turnover', { input: { contact_id: props.contact.id, account_id: null } }),
})
const history = useQuery({
  queryKey: computed(() => ['transactions', 'contact', props.contact.id]),
  queryFn: () => call<Transaction[]>('list_transactions', { filter: emptyTxFilter({ contact_id: props.contact.id }) }),
})
const debtHistory = computed(() => (history.data.value ?? []).filter((tx) => txDebt(tx) !== null))

const detailTab = ref('debts')
const detailTabs = computed(() => [
  { value: 'debts', label: t('contacts.tab.debts') },
  { value: 'history', label: t('contacts.tab.history') },
  { value: 'turnover', label: t('contacts.tab.turnover') },
])
</script>

<template>
  <div class="grid gap-4">
    <Card :class="props.contact.archived && 'opacity-70'">
      <template #header>
        <div class="flex items-center justify-end gap-1">
          <Button
            variant="outline"
            size="sm"
            :disabled="!props.canTransfer || props.contact.archived"
            :title="props.contact.archived ? t('contacts.transferArchived') : props.canTransfer ? undefined : t('contacts.transferNoStore')"
            @click="emit('transfer', null)"
          >
            <ArrowRightLeft class="size-4" />
            {{ t('contacts.writeTransfer') }}
          </Button>
          <ContactArchive :contact="props.contact" @deleted="emit('deleted')" />
        </div>
      </template>
      <ContactEditor :key="props.contact.id" :contact="props.contact" />
    </Card>

    <Card>
      <Tabs v-model="detailTab" :tabs="detailTabs">
        <template #debts>
          <div class="grid gap-4 @container">
            <DebtPanel :contact="props.contact" :balances="ownBalances" @operation="(effect) => emit('transfer', effect)" />
            <TransactionsList v-if="debtHistory.length" :transactions="debtHistory" />
          </div>
        </template>
        <template #history>
          <TransactionsList v-if="history.data.value?.length" :transactions="history.data.value" />
          <EmptyState v-else compact :title="t('contacts.noHistory')" />
        </template>
        <template #turnover>
          <ContactTurnoverTable :turnover="turnover.data.value ?? null" />
        </template>
      </Tabs>
    </Card>
  </div>
</template>
