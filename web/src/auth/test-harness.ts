import type { QueryClient } from "@tanstack/react-query"

import type { AuthMe } from "@/api"

import { authMeQueryKey } from "./auth-me"

export function seedAuthMe(
  queryClient: QueryClient,
  overrides: Partial<AuthMe> = {}
): QueryClient {
  queryClient.setQueryData<AuthMe>(authMeQueryKey, {
    user_id: "00000000-0000-0000-0000-000000000000",
    role: "user",
    onboarding_version: 0,
    default_asset: null,
    ai_features_enabled: false,
    ...overrides,
  })
  return queryClient
}
