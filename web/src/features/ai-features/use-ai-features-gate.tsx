import { useCallback, useRef, useState, type ReactNode } from "react"
import { useQueryClient } from "@tanstack/react-query"

import type { NormalizedError } from "@/lib/errors"
import {
  FALLBACK_MESSAGES,
  isAiFeaturesDisabled,
  normalizeError,
} from "@/lib/errors"
import type { UserId } from "@/lib/query"

import { AiFeaturesDialog } from "./ai-features-dialog"
import {
  setCachedAiFeaturesEnabled,
  useAiFeaturesEnabled,
  useSetAiFeatures,
} from "./api"

export type RunWithAiConsent = (action: () => unknown) => void

export interface AiFeaturesGate {
  readonly requireAiConsent: <T>(action: () => Promise<T>) => Promise<T>
  readonly runWithAiConsent: RunWithAiConsent
  readonly dialog: ReactNode
}

interface AwaitingConsent {
  readonly proceed: () => void
  readonly decline: () => void
}

const DECLINED: NormalizedError = {
  kind: "canceled",
  message: FALLBACK_MESSAGES.canceled,
}

export function useAiFeaturesGate(userId: UserId): AiFeaturesGate {
  const queryClient = useQueryClient()
  const aiFeaturesEnabled = useAiFeaturesEnabled()
  const turnOn = useSetAiFeatures(userId)
  const awaitingRef = useRef<AwaitingConsent[]>([])
  const [isAsking, setIsAsking] = useState(false)

  const requireAiConsent = useCallback(
    <T,>(action: () => Promise<T>): Promise<T> => {
      function askFirst(): Promise<T> {
        return new Promise<T>((resolve, reject) => {
          awaitingRef.current.push({
            proceed: () => {
              attempt().then(resolve, reject)
            },
            decline: () => {
              reject(DECLINED)
            },
          })
          setIsAsking(true)
        })
      }

      function attempt(): Promise<T> {
        return action().catch((error: unknown) => {
          if (!isAiFeaturesDisabled(normalizeError(error))) throw error
          setCachedAiFeaturesEnabled(queryClient, false)
          return askFirst()
        })
      }

      return aiFeaturesEnabled ? attempt() : askFirst()
    },
    [aiFeaturesEnabled, queryClient]
  )

  const runWithAiConsent = useCallback(
    (action: () => unknown) => {
      // The action reports its own failures, and a declined prompt has nothing left to do.
      requireAiConsent(async () => action()).catch(() => undefined)
    },
    [requireAiConsent]
  )

  const settle = (outcome: keyof AwaitingConsent) => {
    const awaiting = awaitingRef.current.splice(0)
    setIsAsking(false)
    for (const entry of awaiting) entry[outcome]()
  }

  const dialog = (
    <AiFeaturesDialog
      open={isAsking}
      isTurningOn={turnOn.isPending}
      error={turnOn.error}
      onTurnOn={() => {
        turnOn.mutate(true, {
          onSuccess: () => {
            settle("proceed")
          },
        })
      }}
      onDecline={() => {
        turnOn.reset()
        settle("decline")
      }}
    />
  )

  return { requireAiConsent, runWithAiConsent, dialog }
}
