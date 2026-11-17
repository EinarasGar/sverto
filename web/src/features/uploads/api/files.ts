import type { GetFileResponse } from "@/api"
import { FilesApiFactory } from "@/api"
import { api } from "@/lib/api"
import type { FileId, UserId } from "@/lib/query"
import { apiQueryOptions, queryKeys, STALE_TIMES } from "@/lib/query"

export function fileQueryOptions(userId: UserId, fileId: FileId) {
  return apiQueryOptions({
    queryKey: queryKeys.user(userId).files.detail(fileId),
    staleTime: STALE_TIMES.reference,
    fetch: async ({ signal }): Promise<GetFileResponse> => {
      const response = await api(FilesApiFactory).getFile(userId, fileId, {
        signal,
      })
      return response.data
    },
    meta: { errorContext: "File details could not be loaded" },
  })
}
