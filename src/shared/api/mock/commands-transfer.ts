/**
 * Commands: handing the vault to a new device (`crate::sync::handover` in
 * Rust). A scripted device plays the other side: receiving, «Тихий Банан»
 * asks to send a vault two seconds in; sending, «Сочный Апельсин» shows up
 * nearby, the password is `secret` and the code `482913`.
 */

import { emitMock } from '../mock-bus'
import { resolve } from './core'
import type { MockCommands } from './core'

const CODE = '482913'
const SIZE = 3 * 1_048_576
const PHONE: TransferNearby = { id: 'mock-phone._mwt-recv._tcp.local.', name: 'Сочный Апельсин', kind: 'mobile' }

let receive: TransferReceiveView | null = null
let sendView: TransferSendView | null = null
const timers: ReturnType<typeof setTimeout>[] = []

function later(ms: number, step: () => void) {
  timers.push(setTimeout(step, ms))
}

function pushReceive(edit: Partial<TransferReceiveView>) {
  if (!receive) return
  receive = { ...receive, ...edit }
  emitMock('transfer-receive', structuredClone(receive))
}

function pushSend(edit: Partial<TransferSendView>) {
  if (!sendView) return
  sendView = { ...sendView, ...edit }
  emitMock('transfer-send', structuredClone(sendView))
}

/** Bytes crossing in four steps, then `done`. */
function stream(
  push: (edit: { done_bytes: number; total_bytes: number; phase: TransferPhase }) => void,
  finish: () => void,
) {
  for (let step = 1; step <= 4; step++) {
    later(step * 400, () => push({ phase: 'transferring', done_bytes: (SIZE * step) / 4, total_bytes: SIZE }))
  }
  later(2000, finish)
}

export const transferCommands: MockCommands = {
  transfer_receive_start: () => {
    receive = {
      phase: 'waiting',
      device_name: 'Тихий Банан',
      expires_ms: Date.now() + 300_000,
      sender_name: null,
      code: null,
      done_bytes: 0,
      total_bytes: 0,
      saved_path: null,
      error: null,
    }
    later(2000, () => pushReceive({ phase: 'request', sender_name: 'Laptop' }))
    return resolve(receive)
  },
  transfer_receive_stop: () => {
    receive = null
    return resolve(null)
  },
  transfer_receive_decide: ({ args }) => {
    if (args?.accept) {
      pushReceive({ phase: 'code', code: CODE })
      later(3000, () =>
        stream(
          (edit) => pushReceive({ ...edit, code: null }),
          () => pushReceive({ phase: 'done', saved_path: '/tmp/mock/received.wallet' }),
        ),
      )
    } else pushReceive({ phase: 'waiting', sender_name: null, error: 'SYNC_TRANSFER_DECLINED' })
    return resolve(null)
  },
  transfer_browse_start: () => {
    later(1000, () => emitMock('transfer-nearby', [PHONE]))
    return resolve([])
  },
  transfer_browse_stop: () => resolve(null),
  transfer_send: ({ body }) => {
    const input = body as TransferSendInput
    if (input.password !== 'secret') return Promise.reject('SECRET: wrong password')
    sendView = {
      phase: 'waiting',
      device_name: PHONE.name,
      attempts_left: 3,
      wrong_code: false,
      done_bytes: 0,
      total_bytes: 0,
      error: null,
    }
    later(1500, () => pushSend({ phase: 'code' }))
    return resolve(sendView)
  },
  transfer_send_code: ({ args }) => {
    if (String(args?.code) !== CODE) {
      const left = (sendView?.attempts_left ?? 1) - 1
      if (left <= 0) pushSend({ phase: 'failed', error: 'SYNC_BAD_CODE' })
      else later(300, () => pushSend({ wrong_code: true, attempts_left: left }))
      return resolve(null)
    }
    stream(pushSend, () => pushSend({ phase: 'done' }))
    return resolve(null)
  },
  transfer_send_cancel: () => {
    for (const timer of timers.splice(0)) clearTimeout(timer)
    sendView = null
    return resolve(null)
  },
}
