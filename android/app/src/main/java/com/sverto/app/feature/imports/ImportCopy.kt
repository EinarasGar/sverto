@file:Suppress("MatchingDeclarationName")

package com.sverto.app.feature.imports

import com.sverto.app.feature.imports.review.ImportTone
import uniffi.sverto_core.ImportAmendRefusal
import uniffi.sverto_core.ImportRowKind
import uniffi.sverto_core.ImportRowVerb
import uniffi.sverto_core.ImportSessionStatus
import uniffi.sverto_core.ImportSource
import uniffi.sverto_core.ImportSourceKind
import java.text.NumberFormat

data class SessionStatusCopy(
    val label: String,
    val tone: ImportTone,
    val summary: String,
)

fun sessionStatusCopy(status: ImportSessionStatus): SessionStatusCopy =
    when (status) {
        ImportSessionStatus.COLLECTING ->
            SessionStatusCopy(
                "Waiting for files",
                ImportTone.Ghost,
                "Add statements, exports, spreadsheets or screenshots. Sverto works out which accounts they belong to.",
            )
        ImportSessionStatus.PARSING ->
            SessionStatusCopy(
                "Reading files",
                ImportTone.Brand,
                "Sverto is reading the files. It keeps going in the background, so it is safe to leave this page.",
            )
        ImportSessionStatus.NEEDS_INPUT ->
            SessionStatusCopy(
                "Needs your input",
                ImportTone.Attention,
                "Something needs a decision from you before the import can continue.",
            )
        ImportSessionStatus.RECONCILING ->
            SessionStatusCopy(
                "Reconciling",
                ImportTone.Brand,
                "Comparing the imported transactions with your ledger.",
            )
        ImportSessionStatus.PROPOSAL_READY ->
            SessionStatusCopy(
                "Ready to review",
                ImportTone.Positive,
                "The proposed ledger changes are ready. Nothing is written until you approve them.",
            )
        ImportSessionStatus.COMPLETED ->
            SessionStatusCopy(
                "Completed",
                ImportTone.Positive,
                "The approved changes are in your ledger.",
            )
    }

fun applyBlocker(
    status: ImportSessionStatus,
    failedFiles: Boolean,
): String? =
    when (status) {
        ImportSessionStatus.COLLECTING -> "Add files first."
        ImportSessionStatus.PARSING -> "Wait until every file has been read."
        ImportSessionStatus.NEEDS_INPUT ->
            if (failedFiles) {
                "Answer the open questions and retry or remove the files that couldn't be read."
            } else {
                "Answer the open questions first."
            }
        ImportSessionStatus.RECONCILING -> "Wait until the transactions are categorized."
        ImportSessionStatus.PROPOSAL_READY, ImportSessionStatus.COMPLETED -> null
    }

const val CLASSIFICATION_FAILED_LABEL = "Couldn't categorize"
const val CLASSIFICATION_FAILED_SUMMARY = "The transactions could not be categorized."

fun kindNote(kind: ImportRowKind): String? =
    when (kind) {
        ImportRowKind.MOVEMENT -> null
        ImportRowKind.STARTING_BALANCE ->
            "Starting balance: added before the statement's first row so that its first balance matches your ledger."
        ImportRowKind.JOINED_TRANSFER ->
            "Moved between your accounts: the row going out and the row coming in, imported as one transfer."
        ImportRowKind.ADJUSTMENT ->
            "Adjustment: closes a difference between two statement balances that nothing in your ledger explains. " +
                "Left out unless you add it."
    }

fun amendRefusalNote(refusal: ImportAmendRefusal): String =
    when (refusal) {
        ImportAmendRefusal.TARGET_GONE ->
            "This correction can't be made: the ledger transaction it matched is gone. Undo the pin to decide again."
        ImportAmendRefusal.NOT_ONE_ENTRY ->
            "This correction can't be made: the ledger transaction doesn't have exactly one entry on this account " +
                "and asset. Undo the pin to decide again."
        ImportAmendRefusal.UNREADABLE ->
            "This correction can't be made: the ledger transaction can't be read. Undo the pin to decide again."
    }

fun verbLabel(verb: ImportRowVerb): String =
    when (verb) {
        ImportRowVerb.ADD -> "Add"
        ImportRowVerb.AMEND -> "Amend"
        ImportRowVerb.DELETE -> "Delete"
        ImportRowVerb.SKIP -> "Skip"
    }

fun verbTone(verb: ImportRowVerb): ImportTone =
    when (verb) {
        ImportRowVerb.ADD -> ImportTone.Positive
        ImportRowVerb.AMEND -> ImportTone.Brand
        ImportRowVerb.DELETE -> ImportTone.Negative
        ImportRowVerb.SKIP -> ImportTone.Ghost
    }

fun formatCount(count: Long): String = NumberFormat.getIntegerInstance().format(count)

fun countOf(
    count: Long,
    noun: String,
    plural: String = "${noun}s",
): String = if (count == 1L) "1 $noun" else "${formatCount(count)} $plural"

fun sourcesCount(
    sources: List<ImportSource>,
    uploads: Int = 0,
): String {
    val bankAccounts = sources.count { it.kind is ImportSourceKind.BankAccount }
    val files = sources.size - bankAccounts + uploads
    return listOfNotNull(
        files.takeIf { it > 0 }?.let { countOf(it.toLong(), "file") },
        bankAccounts.takeIf { it > 0 }?.let { countOf(it.toLong(), "bank account") },
    ).joinToString(", ")
}

const val FILES_FOOTNOTE =
    "Include every account you move money between, so transfers import as one transaction. " +
        "CSV, Excel, PDF, OFX and other statements up to 20 MB."
