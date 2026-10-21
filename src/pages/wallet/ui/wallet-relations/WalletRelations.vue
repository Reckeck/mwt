<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { EntityRelations } from '@/widgets/entity-sheet'
import { call } from '@/shared/api/ipc'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

/** «Связи» of a wallet page: the same groups its card shows. */
const props = defineProps<{ storeId: string }>()

const { t } = useI18n()
const target = computed<EntityRef>(() => ({ kind: 'store', id: props.storeId }))
const card = useQuery({
  queryKey: computed(() => ['entity-card', 'store', props.storeId]),
  queryFn: () => call<EntityCard>('get_entity_card', { target: target.value }),
  placeholderData: keepPreviousData,
})
const groups = computed(() => card.data.value?.groups ?? [])
</script>

<template>
  <Card data-tour="wallet-relations">
    <SectionState
      :loading="card.isPending.value"
      :error="card.error.value"
      :empty="!groups.length"
      :empty-title="t('entity.noRelations')"
      :empty-description="t('wallet.relationsHint')"
      compact
      @retry="card.refetch()"
    >
      <EntityRelations :groups="groups" />
    </SectionState>
  </Card>
</template>
