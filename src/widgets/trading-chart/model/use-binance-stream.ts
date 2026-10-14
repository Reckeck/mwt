import { type Ref, onBeforeUnmount, ref } from 'vue'
import type { UTCTimestamp } from 'lightweight-charts'

import { tfLabel } from '../lib/timeframe'
import type { ChartBar } from './use-chart-core'

export type StreamStatus = 'connecting' | 'live' | 'down'

/**
 * The live candle from Binance's public kline stream: reconnects with
 * back-off (up to 30 s) and reports Live / Connecting / Offline.
 */
export function useBinanceStream(symbol: () => string, tf: Ref<number>, onBar: (bar: ChartBar) => void) {
  const status = ref<StreamStatus>('down')
  let ws: WebSocket | null = null
  let retry: ReturnType<typeof setTimeout> | null = null
  let attempts = 0
  let alive = true

  function disconnect() {
    if (retry) {
      clearTimeout(retry)
      retry = null
    }
    if (ws) {
      ws.onclose = null
      ws.onerror = null
      ws.close()
      ws = null
    }
  }

  function scheduleReconnect() {
    if (!alive) return
    status.value = 'connecting'
    const delay = Math.min(1000 * 2 ** attempts, 30_000)
    attempts++
    retry = setTimeout(connect, delay)
  }

  function connect() {
    disconnect()
    if (!alive || !symbol()) return
    const url = `wss://stream.binance.com:9443/ws/${symbol().toLowerCase()}usdt@kline_${tfLabel(tf.value)}`
    try {
      ws = new WebSocket(url)
    } catch {
      scheduleReconnect()
      return
    }
    ws.onopen = () => {
      attempts = 0
      status.value = 'live'
    }
    ws.onerror = () => ws?.close()
    ws.onclose = () => {
      if (ws) {
        ws = null
        status.value = 'down'
        scheduleReconnect()
      }
    }
    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(String(event.data)) as {
          k?: { t: number; o: string; h: string; l: string; c: string; v: string }
        }
        const kline = msg.k
        if (!kline) return
        status.value = 'live'
        onBar({
          time: Math.floor(kline.t / 1000) as UTCTimestamp,
          open: Number(kline.o),
          high: Number(kline.h),
          low: Number(kline.l),
          close: Number(kline.c),
          volume: Number(kline.v),
        })
      } catch {
        // A malformed frame is skipped; the next one carries the bar again.
      }
    }
  }

  /** Start over for a new symbol or timeframe. */
  function restart() {
    attempts = 0
    connect()
  }

  onBeforeUnmount(() => {
    alive = false
    disconnect()
  })

  return { status, connect, restart }
}
