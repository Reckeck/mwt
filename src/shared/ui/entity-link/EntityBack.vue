<script setup lang="ts">
import { ArrowLeft } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'

import { useEntityNav } from '@/shared/lib/entity-link'
import { Button } from '@/shared/ui/button'

/**
 * `← Back` for a card opened over another one (`?open=a~b`): it drops the top
 * card and the one under it shows again. Nothing under it, nothing drawn.
 * One button for every card sheet (docs/ui-links.md, 24).
 */
const { t } = useI18n()
const nav = useEntityNav()
</script>

<template>
  <Button
    v-if="nav.stack.value.length > 1"
    variant="ghost"
    size="sm"
    class="touch-hit justify-self-start self-start"
    data-entity-back
    @click="nav.back()"
  >
    <ArrowLeft class="size-4" aria-hidden="true" />
    {{ t('entity.back') }}
  </Button>
</template>
