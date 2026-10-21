<script setup lang="ts">
import { computed, useAttrs } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink, useRoute } from 'vue-router'

import { entityLocation } from '@/shared/lib/entity-link'
import { cn } from '@/shared/lib/utils'
import { ContextMenu } from '@/shared/ui/context-menu'

import { ENTITY_ICONS, ENTITY_LINK_CLASS } from './entity-link.constants'
import type { EntityLinkProps } from './entity-link.types'
import { useEntityActions } from './use-entity-actions'

/**
 * A mention of an entity: a click opens its card (or page), the right click
 * lists what can be done with it. On touch the same actions are in the card.
 */
defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<EntityLinkProps>(), { label: '', archived: false, plain: false })
const attrs = useAttrs()

const { t } = useI18n()
const route = useRoute()
const actionsOf = useEntityActions()

const text = computed(() => props.label || t(`entity.kind.${props.target.kind}`))
const to = computed(() => entityLocation(props.target, route.query))
// The caller's classes (a muted colour, a size) go on the link, not on the menu wrapper.
const linkClass = computed(() =>
  cn(ENTITY_LINK_CLASS, props.archived && 'text-muted-foreground', attrs.class as string | undefined),
)
const actions = computed(() => actionsOf(props.target, text.value))
</script>

<template>
  <ContextMenu :actions="actions">
    <RouterLink
      :to="to"
      :class="linkClass"
      :data-entity="`${props.target.kind}:${props.target.id}`"
    >
      <component
        :is="ENTITY_ICONS[props.target.kind]"
        v-if="!props.plain"
        class="size-3.5 shrink-0 text-muted-foreground"
        aria-hidden="true"
      />
      <span class="truncate">{{ text }}</span>
    </RouterLink>
  </ContextMenu>
</template>
