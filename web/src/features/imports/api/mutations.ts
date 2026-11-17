import { useMutation, useQueryClient } from "@tanstack/react-query"

import type {
  IdentifiableImportFile,
  IdentifiableImportSessionResponse,
  ImportAnswer,
  ImportDateOrder,
  ImportItemVerbOverride,
  ImportSessionDetailResponse,
} from "@/api"
import { ImportsApiFactory } from "@/api"
import { api } from "@/lib/api"
import { normalizeError } from "@/lib/errors"
import type { FileId, ImportSessionId, UserId } from "@/lib/query"
import {
  mutationKeys,
  optimisticMutationOptions,
  optimisticUpdate,
  queryKeys,
  withNormalizedErrors,
} from "@/lib/query"

import type { ImportSessionParams } from "./queries"

export interface DeleteImportSessionVariables {
  readonly sessionId: ImportSessionId
}

export interface AttachImportFilesVariables {
  readonly fileIds: readonly FileId[]
}

export interface DetachImportFileVariables {
  readonly fileId: FileId
}

export interface RetryImportFileVariables {
  readonly fileId: FileId
}

export interface ReadImportFileDatesVariables {
  readonly fileId: FileId
  readonly dateOrder: ImportDateOrder
}

export function useCreateImportSession(userId: UserId) {
  const queryClient = useQueryClient()

  return useMutation({
    mutationKey: mutationKeys.user(userId).imports(),
    mutationFn: async (): Promise<IdentifiableImportSessionResponse> => {
      const response = await withNormalizedErrors(() =>
        api(ImportsApiFactory).createImportSession(userId)
      )
      return response.data
    },
    onSuccess: (session) => {
      queryClient.setQueryData<IdentifiableImportSessionResponse[]>(
        queryKeys.user(userId).imports.list(),
        (previous) =>
          previous === undefined ? previous : [session, ...previous]
      )
    },
    meta: { errorContext: "The import could not be started" },
  })
}

export function useDeleteImportSession(userId: UserId) {
  const queryClient = useQueryClient()
  const listKey = queryKeys.user(userId).imports.list()

  return useMutation({
    ...optimisticMutationOptions<void, DeleteImportSessionVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).imports(),
      mutationFn: async ({ sessionId }) => {
        await api(ImportsApiFactory).deleteImportSession(userId, sessionId)
      },
      updates: [
        optimisticUpdate<
          IdentifiableImportSessionResponse[],
          DeleteImportSessionVariables
        >(listKey, (previous, { sessionId }) =>
          previous?.filter((session) => session.id !== sessionId)
        ),
      ],
      meta: { errorContext: "The import could not be deleted" },
    }),
    onSuccess: (_data, { sessionId }) => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.user(userId).imports.detail(sessionId),
        refetchType: "none",
      })
    },
  })
}

export function useAttachImportFiles(params: ImportSessionParams) {
  const queryClient = useQueryClient()
  const { userId, sessionId } = params

  return useMutation({
    ...optimisticMutationOptions<
      IdentifiableImportFile[],
      AttachImportFilesVariables
    >({
      queryClient,
      mutationKey: mutationKeys.user(userId).imports(),
      mutationFn: async ({ fileIds }) => {
        const response = await api(ImportsApiFactory).attachFiles(
          userId,
          sessionId,
          { file_ids: [...fileIds] }
        )
        return response.data
      },
      updates: [],
      invalidate: [queryKeys.user(userId).imports.all()],
      meta: { errorContext: "The file could not be added to the import" },
    }),
    onSuccess: (attached) => {
      queryClient.setQueryData<ImportSessionDetailResponse>(
        queryKeys.user(userId).imports.detail(sessionId),
        (previous) => {
          if (previous === undefined) return previous
          const known = new Set(previous.files.map((file) => file.file_id))
          const added = attached.filter((file) => !known.has(file.file_id))
          return { ...previous, files: [...previous.files, ...added] }
        }
      )
    },
  })
}

export function useDetachImportFile(params: ImportSessionParams) {
  const queryClient = useQueryClient()
  const { userId, sessionId } = params
  const detailKey = queryKeys.user(userId).imports.detail(sessionId)

  return useMutation({
    ...optimisticMutationOptions<void, DetachImportFileVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).imports(),
      mutationFn: async ({ fileId }) => {
        await api(ImportsApiFactory).detachFile(userId, sessionId, fileId)
      },
      updates: [
        optimisticUpdate<
          ImportSessionDetailResponse,
          DetachImportFileVariables
        >(detailKey, (previous, { fileId }) =>
          previous === undefined
            ? previous
            : {
                ...previous,
                files: previous.files.filter((file) => file.file_id !== fileId),
              }
        ),
      ],
      invalidate: [queryKeys.user(userId).imports.list()],
      meta: { errorContext: "The file could not be removed from the import" },
    }),
    onSuccess: (_data, { fileId }) => {
      queryClient.removeQueries({
        queryKey: queryKeys.user(userId).files.detail(fileId),
      })
    },
  })
}

export function useRetryImportFile(params: ImportSessionParams) {
  const queryClient = useQueryClient()
  const { userId, sessionId } = params
  const detailKey = queryKeys.user(userId).imports.detail(sessionId)

  return useMutation({
    ...optimisticMutationOptions<void, RetryImportFileVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).imports(),
      mutationFn: async ({ fileId }) => {
        await api(ImportsApiFactory).retryFile(userId, sessionId, fileId)
      },
      updates: [
        optimisticUpdate<ImportSessionDetailResponse, RetryImportFileVariables>(
          detailKey,
          (previous, { fileId }) =>
            previous === undefined
              ? previous
              : {
                  ...previous,
                  files: previous.files.map((file): IdentifiableImportFile =>
                    file.file_id === fileId
                      ? {
                          ...file,
                          status: "pending",
                          error_message: null,
                          paused_until: null,
                        }
                      : file
                  ),
                }
        ),
      ],
      invalidate: [queryKeys.user(userId).imports.all()],
      meta: { errorContext: "The file could not be retried" },
    }),
  })
}

export function useReadImportFileDates(params: ImportSessionParams) {
  const queryClient = useQueryClient()
  const { userId, sessionId } = params
  const detailKey = queryKeys.user(userId).imports.detail(sessionId)

  return useMutation({
    ...optimisticMutationOptions<void, ReadImportFileDatesVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).imports(),
      mutationFn: async ({ fileId, dateOrder }) => {
        await api(ImportsApiFactory).readFileDates(userId, sessionId, fileId, {
          date_order: dateOrder,
        })
      },
      updates: [
        optimisticUpdate<
          ImportSessionDetailResponse,
          ReadImportFileDatesVariables
        >(detailKey, (previous, { fileId, dateOrder }) =>
          previous === undefined
            ? previous
            : {
                ...previous,
                files: previous.files.map((file): IdentifiableImportFile =>
                  file.file_id === fileId
                    ? { ...file, status: "pending", date_order: dateOrder }
                    : file
                ),
              }
        ),
      ],
      invalidate: [queryKeys.user(userId).imports.all()],
      meta: { errorContext: "The file could not be read again" },
    }),
  })
}

export function useRetryImportClassification(params: ImportSessionParams) {
  const queryClient = useQueryClient()
  const { userId, sessionId } = params
  const detailKey = queryKeys.user(userId).imports.detail(sessionId)

  return useMutation({
    ...optimisticMutationOptions<void, void>({
      queryClient,
      mutationKey: mutationKeys.user(userId).imports(),
      mutationFn: async () => {
        await api(ImportsApiFactory).retryClassification(userId, sessionId)
      },
      updates: [
        optimisticUpdate<ImportSessionDetailResponse, void>(
          detailKey,
          (previous) =>
            previous === undefined
              ? previous
              : {
                  ...previous,
                  session: {
                    ...previous.session,
                    classification: { status: "running", error_message: null },
                  },
                }
        ),
      ],
      invalidate: [queryKeys.user(userId).imports.all()],
      meta: { errorContext: "Categorizing could not be retried" },
    }),
  })
}

export interface ApplyImportSessionVariables {
  readonly fingerprint: string
}

export function useApplyImportSession(params: ImportSessionParams) {
  const queryClient = useQueryClient()
  const { userId, sessionId } = params

  return useMutation({
    ...optimisticMutationOptions<void, ApplyImportSessionVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).imports(),
      mutationFn: async ({ fingerprint }) => {
        await api(ImportsApiFactory).applyImportSession(userId, sessionId, {
          fingerprint,
        })
      },
      updates: [],
      invalidate: [
        queryKeys.user(userId).imports.all(),
        queryKeys.user(userId).transactions.all(),
        queryKeys.user(userId).accounts.all(),
        queryKeys.user(userId).portfolio.all(),
      ],
      meta: { errorContext: "The import could not be applied" },
    }),
  })
}

export interface PinImportItemsVariables {
  readonly itemIds: readonly string[]
  readonly verb: ImportItemVerbOverride
  readonly candidateId?: string
}

export function usePinImportItems(params: ImportSessionParams) {
  const queryClient = useQueryClient()
  const { userId, sessionId } = params

  return useMutation({
    ...optimisticMutationOptions<void, PinImportItemsVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).imports(),
      mutationFn: async ({ itemIds, verb, candidateId }) => {
        await api(ImportsApiFactory).setItemsVerb(userId, sessionId, {
          item_ids: [...itemIds],
          verb,
          ...(candidateId === undefined ? {} : { candidate_id: candidateId }),
        })
      },
      updates: [],
      invalidate: [queryKeys.user(userId).imports.all()],
      meta: { errorContext: "The row could not be changed" },
    }),
  })
}

export interface AskAnswer {
  readonly askKey: string
  readonly answer: ImportAnswer
}

export interface AnswerAsksVariables {
  readonly answers: readonly AskAnswer[]
}

export function useAnswerAsks(params: ImportSessionParams) {
  const queryClient = useQueryClient()
  const { userId, sessionId } = params

  return useMutation({
    mutationKey: mutationKeys.user(userId).imports(),
    mutationFn: async ({ answers }: AnswerAsksVariables) => {
      for (const [index, { askKey, answer }] of answers.entries()) {
        try {
          await api(ImportsApiFactory).answerAsk(
            userId,
            sessionId,
            askKey,
            answer
          )
        } catch (error) {
          const normalized = normalizeError(error)
          if (index === 0 || normalized.kind !== "notFound") throw normalized
        }
      }
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({
        queryKey: queryKeys.user(userId).imports.all(),
      })
    },
    meta: { errorContext: "The answer could not be saved" },
  })
}
