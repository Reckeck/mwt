<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronLeft, ChevronRight, Star, X } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'

import { CurrencyPicker, useFavoriteCurrencies } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Label } from '@/shared/ui/label'
import { call } from '@/shared/api/ipc'
import { required, useFormValidation } from '@/shared/lib/form'
import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'
import { useVaultStore } from '@/shared/store/vault'

/** The favourite currencies and coins: add, remove (with undo) and order them. */
const { t } = useI18n()
const settingsStore = useSettingsStore()
const vaultStore = useVaultStore()
const queryClient = useQueryClient()
const favoriteOrder = useFavoriteCurrencies()

/** Currency staged in the picker, waiting for the Add button. */
const pickedFavorite = ref('')

/** The base currency is not listed: it always comes first anyway. */
const favorites = computed(() =>
  settingsStore.favoriteCurrencies.filter((code) => code.toLowerCase() !== settingsStore.baseCurrency),
)

/** A code the catalog does not list would be refused by the backend; an unloaded catalog checks nothing. */
function unknownCode(code: string): boolean {
  return settingsStore.currencies !== null && !settingsStore.allCurrencies.includes(code)
}

const favoriteCheck = useFormValidation(() => {
  const code = pickedFavorite.value.trim().toLowerCase()
  const taken = code === settingsStore.baseCurrency || favorites.value.some((entry) => entry.toLowerCase() === code)
  const unknown = code && unknownCode(code) ? t('settings.favoriteRejected', { code: code.toUpperCase() }) : null
  return { favorite: required(code, 'pick') ?? (taken ? t('validation.duplicateCurrency') : unknown) }
})

async function addFavorite() {
  const code = pickedFavorite.value.trim().toUpperCase()
  pickedFavorite.value = ''
  favoriteCheck.reset()
  try {
    await favoriteOrder.add(code)
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
    await settingsStore.loadApp().catch(() => {})
    return
  }
  // The backend drops a code it cannot use; «Сохранено» would then be a lie.
  if (!settingsStore.favoriteCurrencies.includes(code)) {
    notify.error(null, t('settings.favoriteRejected', { code }))
    return
  }
  notify.saved()
  if (settingsStore.cryptoCurrencies.includes(code.toLowerCase())) void priceNewCoin()
}

/** A new crypto favourite has no cached price yet: fetch one quietly, so its rate shows at once. */
async function priceNewCoin() {
  if (vaultStore.locked) return
  try {
    await call<RefreshResult>('refresh_all')
    await queryClient.invalidateQueries({ queryKey: ['fx-table'] })
  } catch {
    /* offline: the coin gets its rate with the next scheduled refresh */
  }
}

/** The order ranks the conversions under an amount: arrows move a favourite. */
async function moveFavorite(code: string, step: -1 | 1) {
  const from = favoriteOrder.codes.value.indexOf(code.toLowerCase())
  if (from < 0) return
  try {
    await favoriteOrder.move(from, from + step)
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
  }
}

async function saveList(list: string[]) {
  try {
    await favoriteOrder.save(list)
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
    await settingsStore.loadApp().catch(() => {})
  }
}

function removeFavorite(code: string) {
  const before = [...favoriteOrder.codes.value]
  void saveList(before.filter((entry) => entry !== code.toLowerCase()))
  notify.success(t('settings.favoriteRemoved', { code: code.toUpperCase() }), {
    action: { label: t('actions.undo'), onClick: () => void saveList(before) },
  })
}
</script>

<template>
    <div class="grid gap-1.5">
    <Label>{{ t('settings.favoriteCurrencies') }}</Label>
    <div v-if="favorites.length" class="flex flex-wrap gap-2">
      <span
        v-for="(code, index) in favorites"
        :key="code"
        class="inline-flex items-center gap-1 rounded-md bg-accent px-2 py-1 text-sm text-accent-foreground"
      >
        <Button
          v-if="index > 0"
          variant="ghost"
          size="icon-xs"
          :aria-label="t('settings.favoriteMoveLeft', { code })"
          :title="t('settings.favoriteMoveLeft', { code })"
          @click="moveFavorite(code, -1)"
        >
          <ChevronLeft class="size-3.5" />
        </Button>
        <Star class="size-3.5 fill-warning text-warning" aria-hidden="true" />
        {{ code }}
        <Button
          v-if="index < favorites.length - 1"
          variant="ghost"
          size="icon-xs"
          :aria-label="t('settings.favoriteMoveRight', { code })"
          :title="t('settings.favoriteMoveRight', { code })"
          @click="moveFavorite(code, 1)"
        >
          <ChevronRight class="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          class="text-muted-foreground hover:text-destructive"
          :aria-label="t('settings.favoriteCurrencyRemove', { code })"
          @click="removeFavorite(code)"
        >
          <X class="size-3.5" />
        </Button>
      </span>
    </div>
    <div class="flex items-end gap-2" :data-invalid="favoriteCheck.invalidAttr('favorite')">
      <CurrencyPicker
        v-model="pickedFavorite"
        class="flex-1"
        :placeholder="t('settings.favoriteCurrencyPlaceholder')"
      />
      <Button size="sm" class="h-9" @click="favoriteCheck.submit(addFavorite, $event)">
        {{ t('settings.favoriteCurrencyAdd') }}
      </Button>
    </div>
    <p v-if="favoriteCheck.firstMessage()" class="text-xs text-destructive" role="alert">{{ favoriteCheck.firstMessage() }}</p>
    <p v-else class="text-xs text-muted-foreground">{{ t('settings.favoriteCurrenciesHint') }}</p>
  </div>
</template>
