package com.sverto.app.feature.imports

import com.sverto.app.feature.imports.review.BalanceLine
import com.sverto.app.feature.imports.review.DecisionNavItem
import com.sverto.app.feature.imports.review.ImportTone
import com.sverto.app.feature.imports.review.OutcomeState
import com.sverto.app.feature.imports.review.StatusHeaderState
import com.sverto.app.feature.imports.review.importDateRange
import uniffi.sverto_core.ImportAsk
import uniffi.sverto_core.ImportBalanceCheck
import uniffi.sverto_core.ImportClassificationStatus
import uniffi.sverto_core.ImportEnteredBalance
import uniffi.sverto_core.ImportFile
import uniffi.sverto_core.ImportFileStatus
import uniffi.sverto_core.ImportItems
import uniffi.sverto_core.ImportRowKind
import uniffi.sverto_core.ImportRowVerb
import uniffi.sverto_core.ImportSessionDetail
import uniffi.sverto_core.ImportSessionStatus
import uniffi.sverto_core.ImportSourceKind
import uniffi.sverto_core.ImportStagedRow

private const val INLINE_ROWS = 8
private const val PREVIEW_ROWS = 5
const val ROW_TARGET = "row:"
const val FILES_KEY = "files"

enum class SessionPhase { Working, NeedsInput, Ready, Completed }

val ImportSessionDetail.files: List<ImportFile>
    get() = sources.mapNotNull { (it.kind as? ImportSourceKind.File)?.file }

data class AppliedCounts(
    val added: Int,
    val corrected: Int,
    val deleted: Int,
    val skipped: Int,
)

sealed interface SessionBlock {
    val key: String

    data class Hero(
        val state: StatusHeaderState,
        val retryCategorizing: Boolean,
    ) : SessionBlock {
        override val key = "hero"
    }

    data class Receipt(
        val lines: List<String>,
    ) : SessionBlock {
        override val key = "receipt"
    }

    data class QuestionsHeading(
        val count: Int,
    ) : SessionBlock {
        override val key = "questions"
    }

    data class Section(
        val title: String,
        val count: String? = null,
        val addAllIds: List<String> = emptyList(),
    ) : SessionBlock {
        override val key = "section-$title"
    }

    data class DecisionNav(
        val items: List<DecisionNavItem>,
    ) : SessionBlock {
        override val key = "decisions"
    }

    data class Outcome(
        val state: OutcomeState,
    ) : SessionBlock {
        override val key = "outcome"
    }

    data class Rows(
        val rows: List<ImportStagedRow>,
        val total: Int,
    ) : SessionBlock {
        override val key = "rows"
    }

    data class AccountQuestion(
        val rawAccount: String,
        val asks: List<ImportAsk.AccountMapping>,
    ) : SessionBlock {
        override val key = "account-$rawAccount"
    }

    data class AssetQuestion(
        val ask: ImportAsk.AssetMapping,
    ) : SessionBlock {
        override val key = "asset-${ask.askKey}"
    }

    data class BalanceQuestion(
        val ask: ImportAsk.NoCheckpoint,
    ) : SessionBlock {
        override val key = "balance-ask-${ask.askKey}"
    }

    data class Files(
        val collapsible: Boolean,
    ) : SessionBlock {
        override val key = FILES_KEY
    }

    data class Pending(
        val count: Long,
    ) : SessionBlock {
        override val key = "pending"
    }

    data class Duplicate(
        val row: ImportStagedRow,
        val progress: String?,
    ) : SessionBlock {
        override val key = duplicateKey(row.id)
    }

    data class StartingBalance(
        val row: ImportStagedRow,
    ) : SessionBlock {
        override val key = startKey(row.id)
    }

    data class BalancedAccounts(
        val checks: List<ImportBalanceCheck>,
    ) : SessionBlock {
        override val key = "balanced"
    }

    data class BalanceCheck(
        val check: ImportBalanceCheck,
        val question: ImportAsk.NoCheckpoint?,
        val entered: ImportEnteredBalance?,
        val adjustments: List<ImportStagedRow>,
    ) : SessionBlock {
        override val key = "check-${check.accountId}-${check.assetId}"
    }

    data class Note(
        val text: String,
    ) : SessionBlock {
        override val key = "note-$text"
    }

    data class Unstageable(
        val count: Int,
    ) : SessionBlock {
        override val key = "unstageable"
    }
}

private fun duplicateKey(id: String) = "duplicate-$id"

private fun startKey(id: String) = "start-$id"

sealed interface ApplyBarState {
    data class Apply(
        val label: String,
        val summary: String?,
        val confirm: ApplyConfirm?,
    ) : ApplyBarState

    data class Navigate(
        val label: String,
        val target: String,
    ) : ApplyBarState

    data class Disabled(
        val text: String,
    ) : ApplyBarState
}

data class ApplyConfirm(
    val summary: String,
    val warnings: List<String>,
    val risky: Boolean,
)

fun sessionPhase(
    detail: ImportSessionDetail,
    items: ImportItems?,
): SessionPhase {
    val status = detail.summary.status
    val questions =
        detail.asks.any { it is ImportAsk.AccountMapping || it is ImportAsk.AssetMapping } ||
            (items?.ready != true && detail.asks.any { it is ImportAsk.NoCheckpoint })
    return when {
        status == ImportSessionStatus.COMPLETED -> SessionPhase.Completed
        questions -> SessionPhase.NeedsInput
        status == ImportSessionStatus.PROPOSAL_READY && items?.ready == true -> SessionPhase.Ready
        else -> SessionPhase.Working
    }
}

fun sessionSubtitle(detail: ImportSessionDetail): String {
    val label = sessionStatusCopy(detail.summary.status).label
    return if (detail.sources.isEmpty()) label else "$label · ${sourcesCount(detail.sources)}"
}

fun sessionBlocks(
    detail: ImportSessionDetail,
    items: ImportItems?,
    seenDuplicates: Set<String>,
    applied: AppliedCounts?,
): List<SessionBlock> {
    val ready = items?.takeIf { it.ready }
    val anyFailed = detail.files.any { it.status == ImportFileStatus.FAILED }
    return when (sessionPhase(detail, items)) {
        SessionPhase.Working -> listOf(heroBlock(detail), SessionBlock.Files(collapsible = false))
        SessionPhase.NeedsInput -> questionBlocks(detail, ready) + SessionBlock.Files(collapsible = !anyFailed)
        SessionPhase.Ready ->
            readyBlocks(detail, ready ?: return emptyList(), seenDuplicates) + SessionBlock.Files(collapsible = !anyFailed)
        SessionPhase.Completed -> listOf(SessionBlock.Receipt(receiptLines(detail, applied)), SessionBlock.Files(collapsible = true))
    }
}

private fun questionBlocks(
    detail: ImportSessionDetail,
    items: ImportItems?,
): List<SessionBlock> {
    val accountGroups = detail.asks.filterIsInstance<ImportAsk.AccountMapping>().groupBy { it.rawAccount }
    val assetAsks = detail.asks.filterIsInstance<ImportAsk.AssetMapping>()
    val balanceAsks = if (items == null) detail.asks.filterIsInstance<ImportAsk.NoCheckpoint>() else emptyList()
    val blocks = mutableListOf<SessionBlock>()
    blocks += SessionBlock.QuestionsHeading(accountGroups.size + assetAsks.size + balanceAsks.size)
    accountGroups.forEach { (raw, asks) -> blocks += SessionBlock.AccountQuestion(raw, asks) }
    assetAsks.forEach { blocks += SessionBlock.AssetQuestion(it) }
    balanceAsks.forEach { blocks += SessionBlock.BalanceQuestion(it) }
    if (detail.counts.pending > 0) blocks += SessionBlock.Pending(detail.counts.pending)
    return blocks
}

private fun readyBlocks(
    detail: ImportSessionDetail,
    items: ImportItems,
    seenDuplicates: Set<String>,
): List<SessionBlock> {
    val blocks = mutableListOf<SessionBlock>()
    val duplicates = items.rows.filter { it.nearMiss.isNotEmpty() }
    val total = (seenDuplicates + duplicates.map { it.id }).size
    val resolved = total - duplicates.size
    val nav = decisionNav(items, duplicates, resolved, total)
    if (nav.isNotEmpty()) blocks += SessionBlock.DecisionNav(nav)
    blocks += SessionBlock.Outcome(outcome(detail, items))
    if (items.rows.isNotEmpty()) blocks += SessionBlock.Rows(previewRows(items.rows), items.rows.size)
    if (duplicates.isNotEmpty()) {
        blocks += SessionBlock.Section("Possible duplicates", formatCount(duplicates.size.toLong()))
        duplicates.forEachIndexed { index, row ->
            blocks += SessionBlock.Duplicate(row, "${resolved + index + 1} of $total".takeIf { total > 1 })
        }
    }
    val starts = items.rows.filter(::isUndecidedStart)
    if (starts.isNotEmpty()) {
        blocks += SessionBlock.Section("Starting balances", addAllIds = if (starts.size > 1) starts.map { it.id } else emptyList())
        starts.forEach { blocks += SessionBlock.StartingBalance(it) }
    }
    blocks += balanceBlocks(detail, items)
    if (items.unstageable.isNotEmpty()) blocks += SessionBlock.Unstageable(items.unstageable.size)
    return blocks
}

private fun decisionNav(
    items: ImportItems,
    duplicates: List<ImportStagedRow>,
    resolved: Int,
    total: Int,
): List<DecisionNavItem> {
    val starts = items.rows.filter(::isUndecidedStart)
    val refusals = items.rows.filter { it.amendRefusal != null }
    return listOfNotNull(
        duplicates.firstOrNull()?.let {
            DecisionNavItem(
                countOf(duplicates.size.toLong(), "possible duplicate"),
                "${resolved + 1}/$total".takeIf { total > 1 },
                duplicateKey(it.id),
            )
        },
        starts.firstOrNull()?.let { DecisionNavItem(countOf(starts.size.toLong(), "starting balance"), null, startKey(it.id)) },
        refusals.firstOrNull()?.let {
            DecisionNavItem(countOf(refusals.size.toLong(), "correction to fix", "corrections to fix"), null, ROW_TARGET + it.id)
        },
    )
}

private fun balanceBlocks(
    detail: ImportSessionDetail,
    items: ImportItems,
): List<SessionBlock> {
    val balanceAsks = detail.asks.filterIsInstance<ImportAsk.NoCheckpoint>()
    val checks =
        items.balanceChecks?.takeIf { it.checks.isNotEmpty() || it.unchecked > 0u || balanceAsks.isNotEmpty() }
            ?: return emptyList()
    val blocks = mutableListOf<SessionBlock>(SessionBlock.Section("Balances"))
    val adjustments = items.rows.filter { it.kind == ImportRowKind.ADJUSTMENT }
    val asked = mutableSetOf<String>()
    val detailed =
        checks.checks.map { check ->
            val sameAccount = { accountId: String, assetId: Int -> accountId == check.accountId && assetId == check.assetId }
            val question = balanceAsks.firstOrNull { sameAccount(it.accountId, it.assetId) }
            question?.let { asked += it.askKey }
            SessionBlock.BalanceCheck(
                check = check,
                question = question,
                entered = checks.entered.firstOrNull { sameAccount(it.accountId, it.assetId) },
                adjustments = adjustments.filter { sameAccount(it.accountId, it.assetId) },
            )
        }
    val (quiet, attention) =
        detailed.partition { block ->
            block.question == null &&
                block.entered == null &&
                isBalanced(block.check) &&
                block.adjustments.none { it.verb == ImportRowVerb.ADD }
        }
    if (quiet.isNotEmpty()) blocks += SessionBlock.BalancedAccounts(quiet.map { it.check })
    blocks += attention
    balanceAsks.filterNot { it.askKey in asked }.forEach { blocks += SessionBlock.BalanceQuestion(it) }
    val unchecked = checks.unchecked.toLong() - balanceAsks.size
    if (unchecked > 0) {
        blocks +=
            SessionBlock.Note(
                "${countOf(unchecked, "account balance")} couldn't be checked: the statements give nothing to compare with.",
            )
    }
    return blocks
}

fun isBalanced(check: ImportBalanceCheck): Boolean =
    check.startingOffBy == 0.0 && check.finalOffBy == 0.0 && check.changes.isEmpty() && check.unlisted.none { it.askKey != null }

private fun importance(row: ImportStagedRow): Int =
    when {
        row.amendRefusal != null -> 0
        row.nearMiss.isNotEmpty() -> 1
        row.verb == ImportRowVerb.DELETE -> 2
        row.verb == ImportRowVerb.AMEND -> 3
        row.kind == ImportRowKind.JOINED_TRANSFER -> 4
        row.verb == ImportRowVerb.SKIP -> 5
        else -> 6
    }

private fun previewRows(rows: List<ImportStagedRow>): List<ImportStagedRow> =
    if (rows.size <= INLINE_ROWS) {
        rows.sortedByDescending { it.date }
    } else {
        rows.sortedWith(compareBy<ImportStagedRow> { importance(it) }.thenByDescending { it.date }).take(PREVIEW_ROWS)
    }

private fun verbCount(
    items: ImportItems,
    verb: ImportRowVerb,
): Int = items.rows.count { it.verb == verb }

private fun outcome(
    detail: ImportSessionDetail,
    items: ImportItems,
): OutcomeState {
    val add = verbCount(items, ImportRowVerb.ADD)
    val amend = verbCount(items, ImportRowVerb.AMEND)
    val delete = verbCount(items, ImportRowVerb.DELETE)
    val skip = verbCount(items, ImportRowVerb.SKIP)
    val addsOnly = amend == 0 && delete == 0
    val headline =
        when {
            addsOnly && add > 0 -> "${countOf(add.toLong(), "transaction")} to add"
            addsOnly -> "Nothing new to add"
            else -> countOf((add + amend + delete).toLong(), "change")
        }
    val breakdown =
        if (addsOnly) {
            skip.takeIf { it > 0 }?.let { "${formatCount(it.toLong())} already in your ledger" }
        } else {
            changeParts(add, amend, delete, skip)
        }
    val dates = items.rows.map { it.date }
    val scope =
        listOfNotNull(
            detail.summary.title,
            dates.minOrNull()?.let { first -> importDateRange(first, dates.max()) },
        ).joinToString(" · ")
    val checks = items.balanceChecks?.checks.orEmpty()
    val off = offBalanceCount(items)
    val balance =
        when {
            checks.isEmpty() -> null
            off == 0 -> BalanceLine(if (checks.size == 1) "Matches the statement" else "Matches the statements", ok = true)
            else ->
                BalanceLine(
                    "${countOf(off.toLong(), "balance")} won't match the ${if (off == 1) "statement" else "statements"}",
                    ok = false,
                )
        }
    return OutcomeState(headline = headline, breakdown = breakdown, scope = scope, balance = balance)
}

private fun changeParts(
    add: Int,
    amend: Int,
    delete: Int,
    skip: Int,
): String =
    listOfNotNull(
        add.takeIf { it > 0 }?.let { "adds ${formatCount(it.toLong())}" },
        amend.takeIf { it > 0 }?.let { "corrects ${formatCount(it.toLong())}" },
        delete.takeIf { it > 0 }?.let { "deletes ${formatCount(it.toLong())}" },
        skip.takeIf { it > 0 }?.let { "skips ${formatCount(it.toLong())}" },
    ).joinToString(" · ").replaceFirstChar { it.uppercase() }

fun appliedCounts(items: ImportItems): AppliedCounts =
    AppliedCounts(
        added = verbCount(items, ImportRowVerb.ADD),
        corrected = verbCount(items, ImportRowVerb.AMEND),
        deleted = verbCount(items, ImportRowVerb.DELETE),
        skipped = verbCount(items, ImportRowVerb.SKIP),
    )

private fun receiptLines(
    detail: ImportSessionDetail,
    applied: AppliedCounts?,
): List<String> {
    if (applied == null) {
        val recorded = detail.sources.sumOf { it.recordedTransactions }
        return listOf("${countOf(recorded, "transaction")} recorded from ${sourcesCount(detail.sources)}")
    }
    return listOfNotNull(
        applied.added.takeIf { it > 0 }?.let { "${formatCount(it.toLong())} added" },
        applied.corrected.takeIf { it > 0 }?.let { "${formatCount(it.toLong())} corrected" },
        applied.deleted.takeIf { it > 0 }?.let { "${formatCount(it.toLong())} deleted" },
        applied.skipped.takeIf { it > 0 }?.let { "${formatCount(it.toLong())} skipped as already recorded" },
    ).ifEmpty { listOf("Nothing new was written") }
}

fun applyBarState(
    detail: ImportSessionDetail,
    items: ImportItems,
): ApplyBarState {
    applyBlocker(detail.summary.status, detail.files.any { it.status == ImportFileStatus.FAILED })
        ?.let { return ApplyBarState.Disabled(it) }
    val duplicates = items.rows.filter { it.nearMiss.isNotEmpty() }
    duplicates.firstOrNull()?.let {
        return ApplyBarState.Navigate("Review ${countOf(duplicates.size.toLong(), "possible duplicate")}", duplicateKey(it.id))
    }
    val starts = items.rows.filter(::isUndecidedStart)
    starts.firstOrNull()?.let {
        return ApplyBarState.Navigate("Decide ${countOf(starts.size.toLong(), "starting balance")}", startKey(it.id))
    }
    val refusals = items.rows.filter { it.amendRefusal != null }
    refusals.firstOrNull()?.let {
        return ApplyBarState.Navigate("Fix ${countOf(refusals.size.toLong(), "correction")}", ROW_TARGET + it.id)
    }
    val add = verbCount(items, ImportRowVerb.ADD)
    val amend = verbCount(items, ImportRowVerb.AMEND)
    val delete = verbCount(items, ImportRowVerb.DELETE)
    val skip = verbCount(items, ImportRowVerb.SKIP)
    val addsOnly = amend == 0 && delete == 0
    val label =
        when {
            addsOnly && add > 0 -> "Add ${countOf(add.toLong(), "transaction")}"
            addsOnly -> "Finish import"
            else -> "Apply ${countOf((add + amend + delete).toLong(), "change")}"
        }
    val summary = if (addsOnly) null else changeParts(add, amend, delete, 0)
    val off = offBalanceCount(items)
    val warnings =
        listOfNotNull(
            off.takeIf { it > 0 }?.let { "${countOf(it.toLong(), "balance")} won't match the statements." },
            delete.takeIf { it > 0 }?.let {
                "${countOf(it.toLong(), "transaction")} in your ledger will be deleted."
            },
        )
    val confirm =
        if (addsOnly && off == 0) {
            null
        } else {
            ApplyConfirm(changeParts(add, amend, delete, skip).ifEmpty { "Nothing new to write" }, warnings, risky = warnings.isNotEmpty())
        }
    return ApplyBarState.Apply(label, summary, confirm)
}

fun isUndecidedStart(row: ImportStagedRow): Boolean = row.kind == ImportRowKind.STARTING_BALANCE && !row.verbLocked

private fun heroBlock(detail: ImportSessionDetail): SessionBlock.Hero {
    val status = detail.summary.status
    val failed = detail.files.count { it.status == ImportFileStatus.FAILED }
    if (failed > 0) {
        return SessionBlock.Hero(
            StatusHeaderState(
                label = "${countOf(failed.toLong(), "file")} ${if (failed == 1) "needs" else "need"} attention",
                summary = "Retry reading them, or remove the files that aren't statements.",
                tone = ImportTone.Negative,
                icon = "file-x",
                working = false,
            ),
            retryCategorizing = false,
        )
    }
    if (detail.summary.classificationStatus == ImportClassificationStatus.FAILED) {
        return SessionBlock.Hero(
            StatusHeaderState(
                label = CLASSIFICATION_FAILED_LABEL,
                summary = CLASSIFICATION_FAILED_SUMMARY,
                tone = ImportTone.Negative,
                icon = "circle-alert",
                working = false,
                error = detail.classificationError,
            ),
            retryCategorizing = true,
        )
    }
    val copy = sessionStatusCopy(status)
    val collecting = status == ImportSessionStatus.COLLECTING
    return SessionBlock.Hero(
        StatusHeaderState(
            label = copy.label,
            summary = copy.summary,
            tone = if (collecting) ImportTone.Ghost else ImportTone.Brand,
            icon = if (collecting) "file-up" else "loader",
            working = !collecting,
        ),
        retryCategorizing = false,
    )
}

fun offBalanceCount(items: ImportItems): Int =
    items.balanceChecks?.checks?.count { it.startingOffBy != 0.0 || it.finalOffBy != 0.0 || it.changes.isNotEmpty() }
        ?: 0
