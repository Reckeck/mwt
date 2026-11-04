<script setup lang="ts">
import { Monitor, MonitorSmartphone } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'

import type { ScopeTagKind } from './scope-tag.types'

/**
 * Where a setting lives: `device` stays on this device, `group` is shared by
 * every device paired with the vault. A line of its own under the setting's
 * hint; the optional slot replaces the stock wording.
 */
const props = defineProps<{ kind: ScopeTagKind }>()
const { t } = useI18n()
</script>

<template>
  <p class="flex items-center gap-1 text-xs text-muted-foreground" :data-scope="props.kind">
    <Monitor v-if="props.kind === 'device'" class="size-3 shrink-0" aria-hidden="true" />
    <MonitorSmartphone v-else class="size-3 shrink-0" aria-hidden="true" />
    <span><slot>{{ t(`scope.${props.kind}`) }}</slot></span>
  </p>
</template>
