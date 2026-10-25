<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Link2Off } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'
import { ENTITY_ICONS, EntityBack, useEntityActions } from '@/shared/ui/entity-link'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/shared/ui/sheet'

import type { EntitySheetProps } from './entity-sheet.types'
import { useEntityName } from './model/use-entity-name'
import EntityRelations from './ui/EntityRelations.vue'

/** The card of any entity without a page of its own: name, main figure, links, actions. */
const props = withDefaults(defineProps<EntitySheetProps>(), { loading: false, error: undefined })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const phone = useIsPhone()
const nameOf = useEntityName()
const actionsOf = useEntityActions()

const title = computed(() => (props.card ? nameOf(props.card.target, props.card.title) : ''))
const actions = computed(() => (props.card && !props.card.missing ? actionsOf(props.card.target, title.value).filter((action) => !action.hidden) : []))
</script>

<template>
  <Sheet v-model:open="open">
    <SheetContent overlay="entity-card"
      :side="phone ? 'bottom' : 'right'"
      class="w-full overflow-y-auto sm:max-w-md"
      :class="phone && 'max-h-[85dvh]'"
      data-tour="entity-card"
      :data-entity-card="props.card ? `${props.card.target.kind}:${props.card.target.id}` : undefined"
    >
      <SheetHeader>
        <SheetTitle class="flex min-w-0 items-center gap-2">
          <component :is="ENTITY_ICONS[props.card.target.kind]" v-if="props.card" class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
          <span class="truncate">{{ title || t('entity.loading') }}</span>
        </SheetTitle>
        <SheetDescription>
          {{ props.card ? t(`entity.kind.${props.card.target.kind}`) : '' }}
          <template v-if="props.card?.archived"> · {{ t('entity.archived') }}</template>
        </SheetDescription>
      </SheetHeader>

      <div class="grid gap-5 px-4 pb-6 text-sm">
        <EntityBack />

        <SectionState :loading="props.loading && !props.card" :error="props.error" skeleton="list" :rows="4" compact>
          <EmptyState
            v-if="props.card?.missing"
            :icon="Link2Off"
            :title="t('entity.missing.title')"
            :description="t('entity.missing.description')"
            compact
          />
          <template v-else-if="props.card">
            <p v-if="props.card.value && props.card.currency" class="text-2xl font-semibold tabular-nums">
              <Money :amount="props.card.value" :currency="props.card.currency" />
            </p>

            <div v-if="actions.length" class="flex flex-wrap gap-2" data-tour="entity-actions">
              <Button v-for="action in actions" :key="action.id" variant="outline" size="sm" :data-entity-action="action.id" @click="action.run?.()">
                <component :is="action.icon" class="size-4" />
                {{ action.label }}
              </Button>
            </div>

            <EntityRelations v-if="props.card.groups.length" :groups="props.card.groups" />
            <p v-else class="text-xs text-muted-foreground">{{ t('entity.noRelations') }}</p>
          </template>
        </SectionState>
      </div>
    </SheetContent>
  </Sheet>
</template>
