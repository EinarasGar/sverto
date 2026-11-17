import { useState, type ReactNode } from "react"
import { RotateCw, Trash2 } from "lucide-react"
import { Link, useNavigate } from "@tanstack/react-router"

import type {
  ImportClassification,
  ImportSessionCounts,
  ImportSessionDetailResponse,
} from "@/api"
import { useUserId } from "@/auth"
import {
  focusRing,
  PageHeader,
  PageHeaderBackLink,
  Panel,
  PanelFootnote,
  PanelHeader,
  PanelTitle,
} from "@/components/primitives"
import { Button } from "@/components/ui/button"
import { useAiFeaturesGate } from "@/features/ai-features"
import { EM_DASH, formatCount, formatDateTimeStamp } from "@/lib/format"
import type { ImportSessionId } from "@/lib/query"
import { cn } from "@/lib/utils"

import type { ImportSessionParams } from "./api"
import {
  useImportFileViews,
  useImportSessionSuspense,
  useRefetchWithDetail,
  useRetryImportClassification,
} from "./api"
import { ImportDeleteDialog } from "./import-delete-dialog"
import { ImportFiles } from "./import-files"
import { type BalanceAsk, ImportItems } from "./import-items"
import { ImportPending } from "./import-pending"
import { ImportQuestions } from "./import-questions"
import { ImportStatusChip } from "./import-status-chip"
import { importSessionTitle, useAccountNames } from "./session-title"
import {
  acceptsFiles,
  CLASSIFICATION_COPY,
  isFinished,
  SESSION_STATUS_COPY,
} from "./status"

type CountKey = Exclude<keyof ImportSessionCounts, "staged_available">

const COUNT_LABELS: Record<CountKey, string> = {
  add: "To add",
  amend: "To amend",
  delete: "To delete",
  skip: "Skipped",
  unstageable: "Couldn't import",
  pending: "Waiting on you",
}

function FactRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="border-b border-border px-[18px] py-[13px] last:border-b-0">
      <div className="text-[9.5px] leading-none font-semibold tracking-[0.1em] text-ink-3 uppercase">
        {label}
      </div>
      <div className="mt-[7px] text-[12.5px] leading-[1.4] text-ink">
        {children}
      </div>
    </div>
  )
}

function ClassificationFact({
  params,
  classification,
}: {
  params: ImportSessionParams
  classification: ImportClassification
}) {
  const retry = useRetryImportClassification(params)
  const aiGate = useAiFeaturesGate(params.userId)
  const retryAsync = retry.mutateAsync
  const copy = CLASSIFICATION_COPY[classification.status]

  return (
    <FactRow label="Categories">
      <div data-slot="import-classification">
        {aiGate.dialog}
        <div className="flex items-start justify-between gap-3">
          <span className="text-ink-2">{copy.summary}</span>
          <ImportStatusChip tone={copy.tone} label={copy.label} />
        </div>
        {classification.status === "failed" ? (
          <div className="mt-2 flex items-start justify-between gap-3">
            <span className="text-[11px] leading-[1.5] text-pretty text-negative">
              {classification.error_message}
            </span>
            <button
              type="button"
              aria-label="Retry categorizing"
              disabled={retry.isPending}
              onClick={() => {
                aiGate.runWithAiConsent(() => retryAsync())
              }}
              className={cn(
                "flex h-7 flex-none items-center gap-1 rounded-button px-2 text-[11px] font-medium text-ink-2 outline-none hover:bg-surface-2 hover:text-ink disabled:opacity-50",
                focusRing.sm
              )}
            >
              <RotateCw aria-hidden className="size-3.5" />
              Retry
            </button>
          </div>
        ) : null}
      </div>
    </FactRow>
  )
}

function SessionFacts({
  params,
  detail,
  accountNames,
}: {
  params: ImportSessionParams
  detail: ImportSessionDetailResponse
  accountNames: readonly string[]
}) {
  const { session, counts } = detail
  const unavailable = (key: CountKey) =>
    !counts.staged_available && key !== "pending"
  const proposal = (Object.keys(COUNT_LABELS) as CountKey[]).filter(
    (key) => counts[key] > 0 || unavailable(key)
  )

  return (
    <Panel data-slot="import-facts">
      <PanelHeader>
        <PanelTitle>Import</PanelTitle>
      </PanelHeader>
      <FactRow label="Started">
        <span className="font-mono">
          {formatDateTimeStamp(session.created_at)}
        </span>
      </FactRow>
      <FactRow label="Accounts">
        {accountNames.length === 1 ? (
          <>
            This import is for your{" "}
            <strong className="font-semibold">{accountNames[0]}</strong> account
          </>
        ) : accountNames.length > 1 ? (
          accountNames.join(", ")
        ) : (
          <span className="text-ink-3">None linked yet</span>
        )}
      </FactRow>
      <ClassificationFact
        params={params}
        classification={session.classification}
      />
      {proposal.length > 0 ? (
        <FactRow label="Proposal">
          <dl className="grid grid-cols-[1fr_auto] gap-x-4 gap-y-1.5">
            {proposal.map((key) => (
              <div key={key} className="contents">
                <dt className="text-ink-2">{COUNT_LABELS[key]}</dt>
                <dd className="text-right font-mono tabular-nums">
                  {unavailable(key) ? EM_DASH : formatCount(counts[key])}
                </dd>
              </div>
            ))}
          </dl>
        </FactRow>
      ) : null}
      {isFinished(session.status) ? null : (
        <PanelFootnote>
          Accounts are linked after the files are read. Nothing is written to
          your ledger until you approve the proposal.
        </PanelFootnote>
      )}
    </Panel>
  )
}

export function ImportSessionDetail({
  sessionId,
}: {
  sessionId: ImportSessionId
}) {
  const userId = useUserId()
  const detail = useImportSessionSuspense({ userId, sessionId })
  useRefetchWithDetail({ userId, sessionId }, detail)
  const { session } = detail
  const accountNames = useAccountNames(session.accounts)
  const views = useImportFileViews(userId, detail.files)
  const status = SESSION_STATUS_COPY[session.status]
  const navigate = useNavigate()
  const [deleting, setDeleting] = useState(false)

  return (
    <>
      <PageHeader
        back={
          <PageHeaderBackLink render={<Link to="/imports" />}>
            Imports
          </PageHeaderBackLink>
        }
        eyebrow="Import"
        title={importSessionTitle(session.created_at, accountNames)}
        meta={<ImportStatusChip tone={status.tone} label={status.label} />}
        intro={status.summary}
        actions={
          <Button
            variant="outline"
            size="lg"
            onClick={() => {
              setDeleting(true)
            }}
          >
            <Trash2 data-icon="inline-start" />
            Delete import
          </Button>
        }
      />

      <div className="grid gap-[26px] xl:grid-cols-[minmax(0,1fr)_300px] xl:items-start">
        <div className="flex min-w-0 flex-col gap-[26px]">
          <ImportQuestions
            params={{ userId, sessionId }}
            asks={detail.open_asks}
            views={views}
            pending={detail.counts.pending}
          />
          <ImportPending
            params={{ userId, sessionId }}
            pending={detail.counts.pending}
          />
          <ImportItems
            params={{ userId, sessionId }}
            status={session.status}
            classification={session.classification}
            balanceAsks={detail.open_asks.filter(
              (ask): ask is BalanceAsk => ask.kind === "no_checkpoint"
            )}
          />
          <ImportFiles
            params={{ userId, sessionId }}
            files={detail.files}
            acceptsFiles={acceptsFiles(session.status)}
          />
        </div>
        <SessionFacts
          params={{ userId, sessionId }}
          detail={detail}
          accountNames={accountNames}
        />
      </div>

      <ImportDeleteDialog
        userId={userId}
        sessionId={sessionId}
        open={deleting}
        onOpenChange={setDeleting}
        onDeleted={() => {
          void navigate({ to: "/imports" })
        }}
      />
    </>
  )
}
