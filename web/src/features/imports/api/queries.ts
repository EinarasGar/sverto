import { useEffect, useRef } from "react"
import {
  useQueries,
  useQuery,
  useQueryClient,
  useSuspenseQuery,
} from "@tanstack/react-query"

import type {
  GetFileResponse,
  IdentifiableImportFile,
  IdentifiableImportPendingTransaction,
  IdentifiableImportSessionResponse,
  ImportItemsResponse,
  ImportSessionDetailResponse,
} from "@/api"
import { ImportsApiFactory } from "@/api"
import { fileQueryOptions } from "@/features/uploads"
import { api } from "@/lib/api"
import type { ImportSessionId, UserId } from "@/lib/query"
import { apiQueryOptions, queryKeys } from "@/lib/query"

export interface ImportSessionParams {
  readonly userId: UserId
  readonly sessionId: ImportSessionId
}

export interface ImportFileView {
  readonly file: IdentifiableImportFile
  readonly details: GetFileResponse | undefined
}

export function importSessionsQueryOptions(userId: UserId) {
  return apiQueryOptions({
    queryKey: queryKeys.user(userId).imports.list(),
    fetch: async ({ signal }): Promise<IdentifiableImportSessionResponse[]> => {
      const response = await api(ImportsApiFactory).listImportSessions(userId, {
        signal,
      })
      return response.data
    },
    meta: { errorContext: "Your imports could not be loaded" },
  })
}

const READING_REFETCH_MS = 3000

export function readingRefetchInterval(
  detail: ImportSessionDetailResponse | undefined
): number | false {
  const reading = detail?.files.some(
    (file) => file.status === "pending" || file.status === "processing"
  )
  const classification = detail?.session.classification.status
  const categorizing =
    classification === "waiting" || classification === "running"
  return reading === true || categorizing ? READING_REFETCH_MS : false
}

export function importSessionQueryOptions({
  userId,
  sessionId,
}: ImportSessionParams) {
  return apiQueryOptions({
    queryKey: queryKeys.user(userId).imports.detail(sessionId),
    fetch: async ({ signal }): Promise<ImportSessionDetailResponse> => {
      const response = await api(ImportsApiFactory).getImportSession(
        userId,
        sessionId,
        { signal }
      )
      return response.data
    },
    refetchInterval: readingRefetchInterval,
    meta: { errorContext: "This import could not be loaded" },
  })
}

export function useImportSessionsSuspense(userId: UserId) {
  return useSuspenseQuery(importSessionsQueryOptions(userId)).data
}

export function useImportSessionSuspense(params: ImportSessionParams) {
  return useSuspenseQuery(importSessionQueryOptions(params)).data
}

export function useImportFileViews(
  userId: UserId,
  files: readonly IdentifiableImportFile[]
): ImportFileView[] {
  return useQueries({
    queries: files.map((file) => fileQueryOptions(userId, file.file_id)),
    combine: (results) =>
      files.map((file, index) => ({ file, details: results[index]?.data })),
  })
}

export function useImportPending(
  params: ImportSessionParams,
  enabled: boolean
) {
  const { userId, sessionId } = params
  return useQuery({
    ...apiQueryOptions({
      queryKey: queryKeys.user(userId).imports.pending(sessionId),
      fetch: async ({
        signal,
      }): Promise<IdentifiableImportPendingTransaction[]> => {
        const response = await api(ImportsApiFactory).listPending(
          userId,
          sessionId,
          { signal }
        )
        return response.data
      },
      enabled,
      meta: { errorContext: "The rows waiting on you could not be loaded" },
    }),
  })
}

export function useImportItems(params: ImportSessionParams, enabled: boolean) {
  const { userId, sessionId } = params
  return useQuery({
    ...apiQueryOptions({
      queryKey: queryKeys.user(userId).imports.items(sessionId),
      fetch: async ({ signal }): Promise<ImportItemsResponse> => {
        const response = await api(ImportsApiFactory).listItems(
          userId,
          sessionId,
          { signal }
        )
        return response.data
      },
      enabled,
      staleTime: 0,
      meta: { errorContext: "What the import would write could not be loaded" },
    }),
  })
}

export function useRefetchWithDetail(
  { userId, sessionId }: ImportSessionParams,
  detail: ImportSessionDetailResponse
) {
  const queryClient = useQueryClient()
  const previous = useRef(detail)
  useEffect(() => {
    if (previous.current === detail) return
    previous.current = detail
    const keys = queryKeys.user(userId).imports
    void queryClient.invalidateQueries({ queryKey: keys.pending(sessionId) })
    void queryClient.invalidateQueries({ queryKey: keys.items(sessionId) })
  }, [detail, queryClient, userId, sessionId])
}
