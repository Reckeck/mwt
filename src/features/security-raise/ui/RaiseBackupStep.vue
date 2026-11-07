<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Archive } from 'lucide-vue-next'

/** Step 3: the backup is made next to the vault file by the backend; the user is told where it lies and who removes it. */
const props = defineProps<{ level: SecurityLevel }>()
const { t } = useI18n()
</script>

<template>
  <div class="grid gap-3">
    <p class="text-sm">{{ t('securityRaise.backup.lead') }}</p>
    <div class="flex items-start gap-3 rounded-lg border border-border bg-muted/40 p-3 text-sm">
      <Archive class="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      <p class="text-muted-foreground">{{ t(props.level === 'pin' ? 'securityRaise.backup.where' : 'securityRaise.backup.wherePlain') }}</p>
    </div>
  </div>
</template>
