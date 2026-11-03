<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { CalendarHeart } from 'lucide-vue-next'

import { useContactSharedEvents } from '@/entities/shared-event'
import { Button } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'

import ContactSharedEvents from './ContactSharedEvents.vue'

/** Events shared with the contact; without any, a first step to the shared-money section. */
const props = defineProps<{ contactId: string }>()

const { t } = useI18n()
const events = useContactSharedEvents(() => props.contactId)
</script>

<template>
  <SectionState
    :loading="events.isPending.value"
    :error="events.error.value"
    :empty="!events.data.value?.length"
    :empty-icon="CalendarHeart"
    :empty-title="t('contacts.sharedEvents.none')"
    :empty-description="t('contacts.sheet.noEventsHint')"
    skeleton="list"
    :rows="2"
    compact
    @retry="events.refetch()"
  >
    <template #empty>
      <Button as-child>
        <RouterLink :to="{ name: 'money-together' }">{{ t('contacts.sheet.toSharedMoney') }}</RouterLink>
      </Button>
    </template>
    <ContactSharedEvents :contact-id="props.contactId" hide-title />
  </SectionState>
</template>
