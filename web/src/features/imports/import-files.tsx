import { FileImage, FileSpreadsheet, FileText, RotateCw, X } from "lucide-react"

import type { ImportDateOrder, ImportFileSource } from "@/api"
import {
  focusRing,
  Panel,
  PanelHeader,
  PanelTitle,
} from "@/components/primitives"
import { Button } from "@/components/ui/button"
import { useAiFeaturesGate } from "@/features/ai-features"
import {
  discardUploadedFile,
  Dropzone,
  formatBytes,
  IMPORT_POLICY,
  isImageType,
  UPLOAD_PHASES,
  UploadList,
  useUploadQueue,
} from "@/features/uploads"
import { countOf, formatDateTimeStamp } from "@/lib/format"
import { cn } from "@/lib/utils"

import type { ImportFileView, ImportSessionParams } from "./api"
import {
  useAttachImportFiles,
  useDetachImportFile,
  useImportFileViews,
  useReadImportFileDates,
  useRetryImportFile,
} from "./api"
import { ImportStatusChip } from "./import-status-chip"
import { fileStatusLabel, fileStatusTone } from "./status"

function FileGlyph({ mimeType }: { mimeType: string | undefined }) {
  const Icon =
    mimeType === undefined
      ? FileText
      : isImageType(mimeType)
        ? FileImage
        : mimeType.includes("csv") ||
            mimeType.includes("spreadsheet") ||
            mimeType.includes("excel") ||
            mimeType.includes("tab-separated")
          ? FileSpreadsheet
          : FileText
  return (
    <span className="flex size-8 flex-none items-center justify-center rounded-button bg-surface-2 text-ink-2">
      <Icon aria-hidden className="size-3.5" strokeWidth={1.8} />
    </span>
  )
}

const DATE_ORDER_COPY: Record<
  ImportDateOrder,
  { reading: string; other: ImportDateOrder; label: string }
> = {
  day_first: {
    reading: "day first, so 03/11 is 3 November",
    other: "month_first",
    label: "month first",
  },
  month_first: {
    reading: "month first, so 03/11 is 11 March",
    other: "day_first",
    label: "day first",
  },
}

function AttachedFileRow({
  view,
  removable,
  onRemove,
  retryable,
  onRetry,
  onReadDates,
}: {
  view: ImportFileView
  removable: boolean
  onRemove: () => void
  retryable: boolean
  onRetry: () => void
  onReadDates?: (dateOrder: ImportDateOrder) => void
}) {
  const { file, details } = view
  const name = details?.original_name ?? "Loading file details…"
  const dates =
    file.date_order === null || file.date_order === undefined
      ? undefined
      : DATE_ORDER_COPY[file.date_order]

  return (
    <li
      data-slot="import-file"
      className="flex items-start gap-3 px-[18px] py-3"
    >
      <FileGlyph mimeType={details?.mime_type} />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1.5">
          <p className="min-w-0 truncate text-[12px] leading-[1.3] font-medium text-ink">
            {name}
          </p>
          <ImportStatusChip
            tone={fileStatusTone(file)}
            label={fileStatusLabel(file)}
          />
        </div>
        <p className="mt-1 font-mono text-[10px] leading-[1.4] text-ink-3">
          {details === undefined ? "" : `${formatBytes(details.size_bytes)} · `}
          added {formatDateTimeStamp(file.created_at)}
        </p>
        {file.recorded_transactions > 0 ? (
          <p className="mt-1 text-[11px] leading-[1.5] text-ink-2">
            Recorded on{" "}
            {countOf(file.recorded_transactions, "ledger transaction")}
          </p>
        ) : null}
        {file.error_message === null ||
        file.error_message === undefined ? null : (
          <p className="mt-1.5 text-[11px] leading-[1.5] text-pretty text-negative">
            {file.error_message}
          </p>
        )}
        {dates === undefined ? null : (
          <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] leading-[1.5] text-ink-3">
            <span className="text-pretty">
              Every date in this file reads both ways and nothing in it says
              which. They are read {dates.reading}.
            </span>
            {onReadDates === undefined ? null : (
              <Button
                size="xs"
                variant="ghost"
                aria-label={`Read the dates in ${name} ${dates.label}`}
                onClick={() => {
                  onReadDates(dates.other)
                }}
              >
                Read {dates.label} instead
              </Button>
            )}
          </div>
        )}
      </div>
      {retryable ? (
        <button
          type="button"
          aria-label={`Retry ${name}`}
          onClick={onRetry}
          className={cn(
            "flex h-7 flex-none items-center gap-1 rounded-button px-2 text-[11px] font-medium text-ink-2 outline-none hover:bg-surface-2 hover:text-ink",
            focusRing.sm
          )}
        >
          <RotateCw aria-hidden className="size-3.5" />
          Retry
        </button>
      ) : null}
      {removable ? (
        <button
          type="button"
          aria-label={`Remove ${name}`}
          onClick={onRemove}
          className={cn(
            "flex size-7 flex-none items-center justify-center rounded-button text-ink-3 outline-none hover:bg-negative-dim hover:text-negative",
            focusRing.sm
          )}
        >
          <X aria-hidden className="size-3.5" />
        </button>
      ) : null}
    </li>
  )
}

export function ImportFiles({
  params,
  files,
  acceptsFiles,
}: {
  params: ImportSessionParams
  files: readonly ImportFileSource[]
  acceptsFiles: boolean
}) {
  const views = useImportFileViews(params.userId, files)
  const attachAsync = useAttachImportFiles(params).mutateAsync
  const detach = useDetachImportFile(params)
  const retry = useRetryImportFile(params)
  const readDatesAsync = useReadImportFileDates(params).mutateAsync
  const aiGate = useAiFeaturesGate(params.userId)
  const retryAsync = retry.mutateAsync
  const queue = useUploadQueue({
    userId: params.userId,
    policy: IMPORT_POLICY,
    onUploaded: async (fileId) => {
      await aiGate.requireAiConsent(() => attachAsync({ fileIds: [fileId] }))
    },
    onRemoved: (fileId) => {
      void discardUploadedFile(params.userId, fileId)
    },
  })
  const uploads = queue.items.filter(
    (item) => item.phase !== UPLOAD_PHASES.done
  )

  return (
    <Panel data-slot="import-files">
      {aiGate.dialog}
      <PanelHeader>
        <PanelTitle>Files</PanelTitle>
        <span className="text-[11px] leading-none font-medium text-ink-3">
          {countOf(files.length, "file")}
        </span>
      </PanelHeader>

      {views.length > 0 ? (
        <ul
          aria-label="Files in this import"
          className="divide-y divide-border"
        >
          {views.map((view) => (
            <AttachedFileRow
              key={view.file.id}
              view={view}
              removable={acceptsFiles}
              onRemove={() => {
                detach.mutate({ fileId: view.file.file_id })
              }}
              retryable={acceptsFiles && view.file.status === "failed"}
              onRetry={() => {
                aiGate.runWithAiConsent(() =>
                  retryAsync({ fileId: view.file.file_id })
                )
              }}
              onReadDates={
                acceptsFiles && view.file.status === "parsed"
                  ? (dateOrder) => {
                      aiGate.runWithAiConsent(() =>
                        readDatesAsync({ fileId: view.file.file_id, dateOrder })
                      )
                    }
                  : undefined
              }
            />
          ))}
        </ul>
      ) : null}

      {uploads.length > 0 ? (
        <div className="border-t border-border px-[18px] py-3 first:border-t-0">
          <UploadList items={uploads} queue={queue} label="Uploads" />
        </div>
      ) : null}

      {acceptsFiles ? (
        <div
          className={cn(
            "p-[18px]",
            (views.length > 0 || uploads.length > 0) && "border-t border-border"
          )}
        >
          <Dropzone
            policy={IMPORT_POLICY}
            onFiles={queue.add}
            headline={
              files.length === 0 ? "Add the files to import" : "Add more files"
            }
            body="Include every account you move money between, so each move imports as one transfer"
          />
        </div>
      ) : views.length === 0 ? (
        <p className="px-[18px] py-4 text-[12px] leading-[1.6] text-ink-3">
          This import has no files.
        </p>
      ) : null}
    </Panel>
  )
}
