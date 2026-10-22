import type { QueryClient } from "@tanstack/react-query"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import type { AuthMe } from "@/api"
import { UsersApiFactory } from "@/api"
import { authMeQueryKey, authMeQueryOptions } from "@/auth"
import { api } from "@/lib/api"
import type { UserId } from "@/lib/query"
import { mutationKeys, withNormalizedErrors } from "@/lib/query"

import { AI_FEATURES_SAVE_FAILED } from "./copy"

export function setCachedAiFeaturesEnabled(
  queryClient: QueryClient,
  enabled: boolean
): void {
  queryClient.setQueryData<AuthMe>(authMeQueryKey, (previous) =>
    previous === undefined
      ? previous
      : { ...previous, ai_features_enabled: enabled }
  )
}

export function useAiFeaturesEnabled(): boolean {
  const { data } = useQuery({
    ...authMeQueryOptions(),
    enabled: false,
    select: (me) => me.ai_features_enabled,
  })
  return data === true
}

export function useSetAiFeatures(userId: UserId) {
  const queryClient = useQueryClient()

  return useMutation({
    mutationKey: mutationKeys.user(userId).account(),
    mutationFn: async (enabled: boolean) => {
      await withNormalizedErrors(() =>
        api(UsersApiFactory).postAiFeatures(userId, { enabled })
      )
    },
    onSuccess: (_data, enabled) => {
      setCachedAiFeaturesEnabled(queryClient, enabled)
    },
    meta: { errorContext: AI_FEATURES_SAVE_FAILED },
  })
}
