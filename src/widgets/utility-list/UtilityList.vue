<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'

import { UtilityDialog } from '@/features/utility-edit'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'

import UtilityCard from './UtilityCard.vue'
import type { UtilityListProps } from './utility-list.types'

const props = defineProps<UtilityListProps>()
const { t } = useI18n()

const items = useQuery({ queryKey: ['utilities'], queryFn: () => call<UtilityItem[]>('list_utility_items') })
/** Utilities live inside their property: only this one's bills. */
const own = computed(() => (items.data.value ?? []).filter((item) => item.property_id === props.propertyId))
const createOpen = ref(false)
</script>

<template>
  <div class="grid gap-4">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-muted-foreground">{{ t('utilities.intro') }}</p>
      <Button @click="createOpen = true">
        <Plus class="size-4" />
        {{ t('utilities.new') }}
      </Button>
    </div>
    <EmptyState v-if="items.isSuccess.value && !own.length" compact :title="t('utilities.empty')">
      <Button @click="createOpen = true"><Plus class="size-4" />{{ t('utilities.new') }}</Button>
    </EmptyState>
    <div class="grid gap-4 @wide:grid-cols-2">
      <UtilityCard v-for="item in own" :key="item.id" :item="item" />
    </div>
    <UtilityDialog v-model:open="createOpen" :property-id="props.propertyId" />
  </div>
</template>
