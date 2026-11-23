import { useMemo } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import type {
  DecideHeldTransactionRequest,
  GetHeldTransactionsResponse,
  RequiredIdentifiableTransaction,
} from "@/api"
import { HeldTransactionsApiFactory } from "@/api"
import { providerName } from "@/features/settings/api"
import { api } from "@/lib/api"
import { normalizeError } from "@/lib/errors"
import type { HeldTransactionId, UserId } from "@/lib/query"
import {
  apiQueryOptions,
  mutationKeys,
  optimisticMutationOptions,
  optimisticUpdate,
  queryKeys,
  STALE_TIMES,
} from "@/lib/query"

import {
  assetUnitsOf,
  nativeAmount,
  resolveAccount,
  resolveAsset,
  toLookupIndex,
  toTransactionRow,
} from "../../api"
import type { HeldTransaction } from "./types"

function toHeldTransactions(
  response: GetHeldTransactionsResponse
): HeldTransaction[] {
  const lookup = toLookupIndex(response.lookup_tables)
  return response.held_transactions.map((held) => {
    const asset = resolveAsset(lookup, held.asset_id)
    return {
      id: held.id,
      providerName: providerName(held.provider_kind),
      account: resolveAccount(lookup, held.account_id),
      date: new Date(held.date),
      description: held.description,
      amount: nativeAmount(held.amount, asset),
      candidates: held.candidates.map((candidate) => {
        const row = toTransactionRow(
          candidate as RequiredIdentifiableTransaction,
          lookup
        )
        const units = [...row.legs, ...row.fees]
          .filter(
            (part) =>
              part.account.accountId === held.account_id &&
              part.amount.asset.assetId === held.asset_id
          )
          .map((part) => assetUnitsOf(part.amount))
        return {
          row,
          amount:
            units.length === 0
              ? row.primaryAmount
              : nativeAmount(
                  units.reduce((sum, value) => sum + value, 0),
                  asset
                ),
        }
      }),
    }
  })
}

export function heldTransactionsQueryOptions(userId: UserId) {
  return apiQueryOptions({
    queryKey: queryKeys.user(userId).transactions.held(),
    staleTime: STALE_TIMES.short,
    fetch: async ({ signal }): Promise<GetHeldTransactionsResponse> => {
      const response = await api(
        HeldTransactionsApiFactory
      ).listHeldTransactions(userId, { signal })
      return response.data
    },
    meta: { errorContext: "Possible duplicates could not be loaded" },
  })
}

export function useHeldTransactions(
  userId: UserId
): readonly HeldTransaction[] {
  const { data } = useQuery(heldTransactionsQueryOptions(userId))
  return useMemo(
    () => (data === undefined ? [] : toHeldTransactions(data)),
    [data]
  )
}

export interface DecideHeldTransactionVariables {
  readonly heldTransactionId: HeldTransactionId
  readonly decision: DecideHeldTransactionRequest
}

type HeldDecisionOutcome = "recorded" | "outdated"

export function useDecideHeldTransaction(userId: UserId) {
  const queryClient = useQueryClient()
  const user = queryKeys.user(userId)

  return useMutation(
    optimisticMutationOptions<
      HeldDecisionOutcome,
      DecideHeldTransactionVariables
    >({
      queryClient,
      mutationKey: mutationKeys.user(userId).transactions(),
      mutationFn: async (variables) => {
        try {
          await api(HeldTransactionsApiFactory).decideHeldTransaction(
            userId,
            variables.heldTransactionId,
            variables.decision
          )
          return "recorded"
        } catch (error) {
          const normalized = normalizeError(error)
          if (normalized.kind === "conflict") return "outdated"
          throw normalized
        }
      },
      updates: [
        optimisticUpdate<
          GetHeldTransactionsResponse,
          DecideHeldTransactionVariables
        >(user.transactions.held(), (previous, variables) =>
          previous === undefined
            ? previous
            : {
                ...previous,
                held_transactions: previous.held_transactions.filter(
                  (held) => held.id !== variables.heldTransactionId
                ),
              }
        ),
      ],
      invalidate: [
        user.transactions.all(),
        user.accounts.all(),
        user.portfolio.all(),
      ],
      meta: { errorContext: "That possible duplicate could not be settled" },
    })
  )
}
