import {
  ArrowRight,
  FileCheck2,
  Files,
  FileSpreadsheet,
  FileText,
  Plus,
} from "lucide-react"
import { Link, useNavigate } from "@tanstack/react-router"

import type { IdentifiableImportSessionResponse } from "@/api"
import { useUserId } from "@/auth"
import { PanelBoundary } from "@/components/layout/route-boundaries"
import { focusRing, PageHeader, SectionHeader } from "@/components/primitives"
import { Button } from "@/components/ui/button"
import { useAiFeaturesGate } from "@/features/ai-features"
import { countOf, formatDateTimeStamp } from "@/lib/format"
import { cn } from "@/lib/utils"

import { useCreateImportSession, useImportSessionsSuspense } from "./api"
import { ImportStatusChip } from "./import-status-chip"
import { importSessionTitle, useAccountNames } from "./session-title"
import { isFinished, SESSION_STATUS_COPY } from "./status"

const NEW_IMPORT = "New import"

function useStartImport() {
  const userId = useUserId()
  const create = useCreateImportSession(userId)
  const navigate = useNavigate()
  const aiGate = useAiFeaturesGate(userId)
  const createAsync = create.mutateAsync

  return {
    pending: create.isPending,
    dialog: aiGate.dialog,
    start: () => {
      aiGate.runWithAiConsent(async () => {
        const session = await createAsync(undefined)
        await navigate({
          to: "/imports/$sessionId",
          params: { sessionId: session.id },
        })
      })
    },
  }
}

function IntakeHero({
  pending,
  onNew,
}: {
  pending: boolean
  onNew: () => void
}) {
  return (
    <section className="grid overflow-hidden rounded-panel border border-border bg-surface md:grid-cols-[minmax(0,1fr)_240px]">
      <div className="px-5 py-6 sm:px-7 sm:py-7">
        <p className="font-mono text-[9.5px] leading-none font-semibold tracking-[0.12em] text-brand uppercase">
          New intake
        </p>
        <h2 className="mt-3 max-w-[520px] text-[20px] leading-[1.15] font-bold tracking-[-0.02em] text-balance text-ink">
          Bring in history without rebuilding it by hand.
        </h2>
        <p className="mt-3 max-w-[560px] text-[11.5px] leading-[1.65] text-pretty text-ink-3">
          Start with statements, exports, spreadsheets, scans, or a mixed batch.
          Sverto works out which accounts they belong to, and the result stays a
          proposal until you approve it.
        </p>
        <Button className="mt-5" size="lg" disabled={pending} onClick={onNew}>
          <Plus data-icon="inline-start" />
          {NEW_IMPORT}
        </Button>
      </div>
      <div className="grid grid-cols-3 border-t border-border bg-surface-2 md:grid-cols-1 md:border-t-0 md:border-l">
        {[
          ["01", "Add files"],
          ["02", "Answer questions"],
          ["03", "Review changes"],
        ].map(([number, label], index) => (
          <div
            key={number}
            className={cn(
              "flex min-w-0 items-center gap-2.5 px-3 py-3.5 md:px-5",
              index > 0 && "border-l border-border md:border-t md:border-l-0"
            )}
          >
            <span className="font-mono text-[9px] font-semibold text-brand">
              {number}
            </span>
            <span className="text-[10px] leading-[1.3] font-medium text-ink-2">
              {label}
            </span>
          </div>
        ))}
      </div>
    </section>
  )
}

function SessionRow({
  session,
}: {
  session: IdentifiableImportSessionResponse
}) {
  const accountNames = useAccountNames(session.accounts)
  const status = SESSION_STATUS_COPY[session.status]

  return (
    <Link
      to="/imports/$sessionId"
      params={{ sessionId: session.id }}
      data-slot="import-session-row"
      className={cn(
        "group block w-full rounded-panel border border-border bg-surface px-4 py-4 text-left transition-colors duration-base hover:border-border-strong hover:bg-surface-2/55 sm:px-5",
        focusRing.panel
      )}
    >
      <div className="flex items-start gap-3.5">
        <span className="flex size-10 flex-none items-center justify-center rounded-panel border border-border bg-surface-2 text-ink-2">
          <Files className="size-[17px]" strokeWidth={1.7} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-2.5 gap-y-2">
            <h3 className="min-w-0 truncate text-[13px] leading-none font-semibold text-ink">
              {importSessionTitle(session.created_at, accountNames)}
            </h3>
            <ImportStatusChip tone={status.tone} label={status.label} />
          </div>
          <p className="mt-2.5 text-[10.5px] leading-[1.5] text-ink-3">
            Started {formatDateTimeStamp(session.created_at)}
          </p>
        </div>
        <ArrowRight className="mt-1 size-4 flex-none text-ink-3 transition-transform duration-base group-hover:translate-x-0.5 group-hover:text-brand" />
      </div>
    </Link>
  )
}

function SessionGroup({
  label,
  sessions,
}: {
  label: string
  sessions: readonly IdentifiableImportSessionResponse[]
}) {
  return (
    <section>
      <SectionHeader label={label} note={countOf(sessions.length, "import")} />
      <div className="grid gap-2.5">
        {sessions.map((session) => (
          <SessionRow key={session.id} session={session} />
        ))}
      </div>
    </section>
  )
}

function SessionLists() {
  const sessions = useImportSessionsSuspense(useUserId())
  const open = sessions.filter((session) => !isFinished(session.status))
  const finished = sessions.filter((session) => isFinished(session.status))

  if (sessions.length === 0) return null

  return (
    <>
      {open.length > 0 ? (
        <SessionGroup label="In progress" sessions={open} />
      ) : null}
      {finished.length > 0 ? (
        <SessionGroup label="Finished" sessions={finished} />
      ) : null}
    </>
  )
}

const FORMATS = [
  {
    icon: FileSpreadsheet,
    title: "Structured exports",
    body: "CSV, TSV, Excel, and plain-text reports",
  },
  {
    icon: FileText,
    title: "Statements",
    body: "PDF statements and account summaries",
  },
  {
    icon: FileCheck2,
    title: "Scans and photos",
    body: "Screenshots, photographed pages, and mixed batches",
  },
] as const

function FormatCards() {
  return (
    <section className="grid gap-2.5 sm:grid-cols-3">
      {FORMATS.map(({ icon: Icon, title, body }) => (
        <div
          key={title}
          className="rounded-panel border border-border px-4 py-4"
        >
          <Icon className="size-4 text-ink-3" strokeWidth={1.7} />
          <h3 className="mt-3 text-[11px] leading-none font-semibold text-ink">
            {title}
          </h3>
          <p className="mt-2 text-[10px] leading-[1.5] text-ink-3">{body}</p>
        </div>
      ))}
    </section>
  )
}

export function ImportsIndex() {
  const { pending, start, dialog } = useStartImport()

  return (
    <>
      {dialog}
      <PageHeader
        eyebrow="Bring your history in"
        title="Imports"
        intro="Turn statements, exports, and document batches into reviewed ledger proposals."
        actions={
          <Button size="lg" disabled={pending} onClick={start}>
            <Plus data-icon="inline-start" />
            {NEW_IMPORT}
          </Button>
        }
      />
      <div className="flex flex-col gap-7">
        <IntakeHero pending={pending} onNew={start} />
        <PanelBoundary>
          <SessionLists />
        </PanelBoundary>
        <FormatCards />
      </div>
    </>
  )
}
