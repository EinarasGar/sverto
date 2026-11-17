import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { toast } from "@/components/ui/toast"
import type { ImportSessionId, UserId } from "@/lib/query"

import { useDeleteImportSession } from "./api"

export function ImportDeleteDialog({
  userId,
  sessionId,
  open,
  onOpenChange,
  onDeleted,
}: {
  userId: UserId
  sessionId: ImportSessionId
  open: boolean
  onOpenChange: (open: boolean) => void
  onDeleted: () => void
}) {
  const remove = useDeleteImportSession(userId)

  return (
    <AlertDialog
      open={open}
      onOpenChange={(next) => {
        if (!remove.isPending) onOpenChange(next)
      }}
    >
      <AlertDialogContent className="rounded-sheet border border-border-strong bg-surface sm:max-w-md">
        <AlertDialogHeader>
          <AlertDialogTitle className="text-[14px] leading-[1.3] font-semibold">
            Delete this import?
          </AlertDialogTitle>
        </AlertDialogHeader>
        <div className="rounded-panel border border-negative-dim bg-surface px-[18px] pt-4 pb-[17px]">
          <AlertDialogDescription className="text-[11.5px] leading-[1.6] text-pretty text-ink-2 sm:text-left">
            The import and the files uploaded to it are deleted.
          </AlertDialogDescription>
          <p className="mt-[7px] text-[11.5px] leading-[1.6] text-pretty text-ink-3 sm:text-left">
            Nothing in your ledger changes, including transactions this import
            already applied.
          </p>
        </div>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={remove.isPending}>
            Keep import
          </AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            disabled={remove.isPending}
            onClick={() => {
              remove.mutate(
                { sessionId },
                {
                  onSuccess: () => {
                    toast.add({
                      type: "success",
                      timeout: 6000,
                      title: "Import deleted",
                    })
                    onDeleted()
                  },
                  onSettled: () => {
                    onOpenChange(false)
                  },
                }
              )
            }}
          >
            {remove.isPending ? "Deleting…" : "Delete import"}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
