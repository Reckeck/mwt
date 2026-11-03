<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { RouterLink } from 'vue-router'
import { ScrollText } from 'lucide-vue-next'

import { TransactionsList } from '@/entities/transaction'
import { Button } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'
import { call } from '@/shared/api/ipc'
import { pluralForm } from '@/shared/lib/format'
import { openTxDetails } from '@/shared/lib/tx-details'
import { emptyTxFilter } from '@/shared/lib/tx-filter'

/** The operations with the contact, newest first; the journal link opens them filtered. */
const props = defineProps<{ contactId: string; transferBlocked: string | false }>()
const emit = defineEmits<{ transfer: [] }>()

const { t, locale } = useI18n()
const history = useQuery({
  // no placeholder: another contact's operations
  queryKey: computed(() => ['transactions', 'contact', props.contactId]),
  queryFn: () => call<Transaction[]>('list_transactions', { filter: emptyTxFilter({ contact_id: props.contactId }) }),
})
const count = computed(() => history.data.value?.length ?? 0)
</script>

<template>
  <SectionState
    :loading="history.isPending.value"
    :error="history.error.value"
    :empty="!count"
    :empty-icon="ScrollText"
    :empty-title="t('contacts.noHistory')"
    :empty-description="t('contacts.sheet.noHistoryHint')"
    skeleton="list"
    :rows="4"
    compact
    @retry="history.refetch()"
  >
    <template #empty>
      <Button :disabled="Boolean(props.transferBlocked)" @click="emit('transfer')">{{ t('contacts.writeTransfer') }}</Button>
    </template>
    <div class="grid grid-cols-1 gap-3">
      <TransactionsList :transactions="history.data.value ?? []" openable @open="openTxDetails" />
      <Button variant="link" class="justify-self-start px-0" as-child>
        <RouterLink :to="{ name: 'transactions', query: { contact: props.contactId } }">
          {{ t(`contacts.sheet.allInJournal.${pluralForm(count, locale)}`, { count }) }}
        </RouterLink>
      </Button>
    </div>
  </SectionState>
</template>
