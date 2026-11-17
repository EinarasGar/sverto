import type { Query, QueryKey } from "@tanstack/react-query"
import { queryOptions } from "@tanstack/react-query"

import { normalizeError } from "@/lib/errors"

import { STALE_TIMES } from "./client"
import type { SvertoQueryMeta } from "./error-reporter"

interface ApiQueryContext {
  signal: AbortSignal
}

export interface ApiQueryConfig<TData, TKey extends QueryKey> {
  queryKey: TKey
  fetch: (context: ApiQueryContext) => Promise<TData>
  staleTime?: number
  gcTime?: number
  enabled?: boolean
  meta?: SvertoQueryMeta
  refetchInterval?: (data: TData | undefined) => number | false
}

export function apiQueryOptions<TData, TKey extends QueryKey>(
  config: ApiQueryConfig<TData, TKey>
) {
  return queryOptions({
    queryKey: config.queryKey,
    queryFn: ({ signal }) =>
      withNormalizedErrors(() => config.fetch({ signal })),
    staleTime: config.staleTime ?? STALE_TIMES.short,
    ...(config.gcTime === undefined ? {} : { gcTime: config.gcTime }),
    ...(config.enabled === undefined ? {} : { enabled: config.enabled }),
    ...(config.meta === undefined ? {} : { meta: config.meta }),
    ...(config.refetchInterval === undefined
      ? {}
      : {
          refetchInterval: (query: Query<TData, Error, TData, TKey>) =>
            config.refetchInterval?.(query.state.data) ?? false,
        }),
  })
}

export async function withNormalizedErrors<TData>(
  run: () => Promise<TData>
): Promise<TData> {
  try {
    return await run()
  } catch (error) {
    throw normalizeError(error)
  }
}
