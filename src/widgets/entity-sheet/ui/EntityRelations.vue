<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { entityKey } from '@/shared/lib/entity-link'
import { formatDate } from '@/shared/lib/format'
import { EntityLink } from '@/shared/ui/entity-link'

import type { EntityRelationsProps } from '../entity-sheet.types'
import { useEntityName } from '../model/use-entity-name'

/** `Links`: everything that points at the entity, one block per kind of link. */
const props = defineProps<EntityRelationsProps>()

const { t } = useI18n()
const nameOf = useEntityName()
</script>

<template>
  <div class="grid gap-4" data-tour="entity-relations">
    <section v-for="group in props.groups" :key="group.kind" class="grid gap-1.5">
      <p class="text-xs font-medium text-muted-foreground">
        {{ t(`entity.relation.${group.kind}`) }}
        <span v-if="group.total > 1" class="tabular-nums">· {{ group.total }}</span>
      </p>
      <ul class="grid gap-1 text-sm">
        <li v-for="item in group.items" :key="entityKey(item.target)" class="flex min-w-0 items-baseline justify-between gap-3">
          <EntityLink :target="item.target" :label="nameOf(item.target, item.label)" :archived="item.archived" />
          <span class="flex shrink-0 items-baseline gap-2 text-xs text-muted-foreground tabular-nums">
            <span v-if="item.when">{{ formatDate(item.when) }}</span>
            <Money v-if="item.value && item.currency" :amount="item.value" :currency="item.currency" />
          </span>
        </li>
      </ul>
      <p v-if="group.total > group.items.length" class="text-xs text-muted-foreground">
        {{ t('entity.more', { count: group.total - group.items.length }) }}
      </p>
    </section>
  </div>
</template>
