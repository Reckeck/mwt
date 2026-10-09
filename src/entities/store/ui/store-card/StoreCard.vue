<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { Archive, ArchiveRestore, Trash2 } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { Card } from '@/shared/ui/card'
import { cn } from '@/shared/lib/utils'
import { getStoreIcon } from '@/shared/lib/store-icons'

import type { StoreCardProps } from './store-card.types'

const props = defineProps<StoreCardProps>()

const emit = defineEmits<{
  toggleArchive: [store: Store]
  delete: [id: string]
}>()

const { t } = useI18n()
</script>

<template>
  <Card :class="cn(props.store.archived && 'opacity-60')">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <div class="flex min-w-0 items-center gap-2">
          <span
            class="flex size-6 shrink-0 items-center justify-center rounded-md border"
            :style="{
              backgroundColor: `${props.store.color}1A`,
              color: props.store.color,
              borderColor: `${props.store.color}33`,
            }"
          >
            <component :is="getStoreIcon(props.store.kind)" class="size-3.5" />
          </span>
          <div class="min-w-0">
            <RouterLink
              :to="{ name: 'wallet', params: { id: props.store.id } }"
              class="block truncate font-medium hover:underline"
            >
              {{ props.store.name }}
            </RouterLink>
            <p v-if="props.parent" class="truncate text-xs text-muted-foreground">
              {{ t('stores.subWalletOf', { parent: props.parent.name }) }}
            </p>
          </div>
        </div>
        <div class="flex items-center gap-1">
          <Badge variant="secondary">{{ t(`stores.kind.${props.store.kind}`) }}</Badge>
        </div>
      </div>
    </template>
    <div class="flex items-center justify-between gap-2">
      <p class="min-w-0 truncate text-sm text-muted-foreground">{{ props.store.note }}</p>
      <div class="flex shrink-0 items-center gap-1">
        <Button
          variant="ghost"
          size="icon"
          class="size-8 text-muted-foreground"
          :title="props.store.archived ? t('stores.unarchive') : t('stores.archive')"
          :aria-label="props.store.archived ? t('stores.unarchive') : t('stores.archive')"
          @click="emit('toggleArchive', props.store)"
        >
          <ArchiveRestore v-if="props.store.archived" class="size-4" />
          <Archive v-else class="size-4" />
        </Button>
        <ConfirmButton
          :label="t('stores.delete')"
          :confirm-label="t('actions.confirmDelete')"
          @confirm="emit('delete', props.store.id)"
        >
          <Trash2 class="size-4" />
        </ConfirmButton>
      </div>
    </div>
  </Card>
</template>
