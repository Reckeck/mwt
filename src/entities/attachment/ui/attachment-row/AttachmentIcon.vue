<script setup lang="ts">
import { computed } from 'vue'
import { File, FileText, Image, Link } from 'lucide-vue-next'

import type { AttachmentLook } from '../../lib/attachment-format'

/** The 32 px tile in front of an attachment: what kind of thing it is; `bare` — the icon alone, for a chip. */
const props = defineProps<{ look: AttachmentLook; bare?: boolean }>()

const ICONS = { pdf: FileText, image: Image, document: File, link: Link } as const
const icon = computed(() => ICONS[props.look])
</script>

<template>
  <component :is="icon" v-if="props.bare" class="size-3.5 shrink-0 text-muted-foreground" />
  <span v-else class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-secondary text-muted-foreground">
    <component :is="icon" class="size-4" />
  </span>
</template>
