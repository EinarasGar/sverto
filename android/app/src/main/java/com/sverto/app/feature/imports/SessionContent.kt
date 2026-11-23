@file:Suppress("MatchingDeclarationName")
@file:OptIn(ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sverto.app.core.ui.RowDivider
import com.sverto.app.feature.imports.review.AccountQuestionCard
import com.sverto.app.feature.imports.review.AssetQuestionCard
import com.sverto.app.feature.imports.review.BalanceCheckCard
import com.sverto.app.feature.imports.review.BalanceQuestionCard
import com.sverto.app.feature.imports.review.BalancedAccountsCard
import com.sverto.app.feature.imports.review.DecisionNavCard
import com.sverto.app.feature.imports.review.DetailLine
import com.sverto.app.feature.imports.review.ExpandableCard
import com.sverto.app.feature.imports.review.ImportReceipt
import com.sverto.app.feature.imports.review.ImportStatusHeader
import com.sverto.app.feature.imports.review.OutcomeCard
import com.sverto.app.feature.imports.review.PossibleDuplicateCard
import com.sverto.app.feature.imports.review.PreviewRowsCard
import com.sverto.app.feature.imports.review.QuestionOption
import com.sverto.app.feature.imports.review.QuestionsHeading
import com.sverto.app.feature.imports.review.SectionCount
import com.sverto.app.feature.imports.review.SectionHeader
import com.sverto.app.feature.imports.review.StartingBalanceCard
import com.sverto.app.feature.imports.review.importDate
import uniffi.sverto_core.ImportAsk
import uniffi.sverto_core.ImportAssetCandidate
import uniffi.sverto_core.ImportDateOrder
import uniffi.sverto_core.ImportItemVerb
import uniffi.sverto_core.ImportSessionStatus
import uniffi.sverto_core.ImportStagedRow

private const val MAX_LISTED_ROWS = 100

class SessionActions(
    val onRetryCategorizing: () -> Unit,
    val onChooseAccount: (rawAccount: String, accountId: String) -> Unit,
    val onPickAccount: (rawAccount: String) -> Unit,
    val onCreateAccount: (rawAccount: String) -> Unit,
    val onSameAccount: (rawAccount: String, same: Boolean) -> Unit,
    val onAnswerAccounts: (askKeys: List<String>, accountId: String?) -> Unit,
    val onChooseAsset: (askKey: String, asset: ImportAssetCandidate) -> Unit,
    val onSearchAsset: (ImportAsk.AssetMapping) -> Unit,
    val onAnswerAsset: (askKey: String, assetId: Int?) -> Unit,
    val onEnterBalance: (askKey: String, quantity: String) -> Unit,
    val onSetVerb: (itemIds: List<String>, verb: ImportItemVerb, candidateId: String?) -> Unit,
    val onChooseDuplicate: (rowId: String, choice: String) -> Unit,
    val onConfirmDuplicate: (rowId: String) -> Unit,
    val onReviewGaps: (blockKey: String) -> Unit,
    val onRowClick: (ImportStagedRow) -> Unit,
    val onNavigate: (target: String) -> Unit,
    val onViewTransactions: () -> Unit,
    val onAddFiles: () -> Unit,
    val onRetryFile: (String) -> Unit,
    val onRemoveFile: (String) -> Unit,
    val onDateOrder: (String, ImportDateOrder) -> Unit,
    val onRetryUpload: (String) -> Unit,
    val onDismissUpload: (String) -> Unit,
    val onOpenAllRows: () -> Unit,
)

@Composable
fun SessionContent(
    blocks: List<SessionBlock>,
    state: ImportSessionUiState,
    actions: SessionActions,
    listState: LazyListState,
    expanded: Set<String>,
    onToggle: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 40.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        items(blocks, key = { it.key }, contentType = { it::class }) { block ->
            SessionBlockItem(
                block = block,
                state = state,
                actions = actions,
                expanded = block.key in expanded,
                onToggle = { onToggle(block.key) },
                modifier = Modifier.animateItem(),
            )
        }
    }
}

@Suppress("CyclomaticComplexMethod", "LongMethod")
@Composable
private fun SessionBlockItem(
    block: SessionBlock,
    state: ImportSessionUiState,
    actions: SessionActions,
    expanded: Boolean,
    onToggle: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val enabled = !state.working
    when (block) {
        is SessionBlock.Hero ->
            ImportStatusHeader(
                state = block.state,
                retryLabel = if (block.retryCategorizing) "Retry categorizing" else null,
                onRetry = actions.onRetryCategorizing,
                modifier = modifier.padding(bottom = 8.dp),
            )
        is SessionBlock.Receipt -> ImportReceipt(block.lines, actions.onViewTransactions, modifier.padding(bottom = 8.dp))
        is SessionBlock.QuestionsHeading -> QuestionsHeading(block.count, modifier)
        is SessionBlock.Section ->
            SectionHeader(
                title = block.title,
                modifier = modifier.padding(top = 16.dp),
                trailing = {
                    when {
                        block.addAllIds.isNotEmpty() ->
                            OutlinedButton(
                                onClick = { actions.onSetVerb(block.addAllIds, ImportItemVerb.ADD, null) },
                                enabled = enabled,
                                shapes = ButtonDefaults.shapes(),
                                modifier = Modifier.semantics { contentDescription = "Add all ${block.addAllIds.size} starting balances" },
                            ) { Text("Add all") }
                        block.count != null -> SectionCount(block.count)
                    }
                },
            )
        is SessionBlock.DecisionNav -> DecisionNavCard(block.items, { actions.onNavigate(it.target) }, modifier)
        is SessionBlock.Outcome -> OutcomeCard(block.state, modifier)
        is SessionBlock.Rows ->
            PreviewRowsCard(
                rows = block.rows,
                total = block.total,
                onRowClick = actions.onRowClick,
                onViewAll = actions.onOpenAllRows,
                modifier = modifier,
            )
        is SessionBlock.AccountQuestion -> AccountQuestion(block, state, actions, modifier)
        is SessionBlock.AssetQuestion -> AssetQuestion(block.ask, state, actions, modifier)
        is SessionBlock.BalanceQuestion ->
            BalanceQuestionCard(
                accountName = block.ask.accountName,
                ticker = block.ask.ticker,
                asOfDate = block.ask.asOfDate,
                enabled = enabled,
                onCheck = { actions.onEnterBalance(block.ask.askKey, it) },
                modifier = modifier,
            )
        is SessionBlock.Files -> {
            val detail = state.detail ?: return
            ImportFilesSection(
                sources = detail.sources,
                uploads = state.uploads,
                acceptsFiles = detail.summary.status != ImportSessionStatus.COMPLETED,
                enabled = enabled,
                collapsible = block.collapsible,
                expanded = expanded,
                onToggle = onToggle,
                onAddFiles = actions.onAddFiles,
                onRetryFile = actions.onRetryFile,
                onRemoveFile = actions.onRemoveFile,
                onDateOrder = actions.onDateOrder,
                onRetryUpload = actions.onRetryUpload,
                onDismissUpload = actions.onDismissUpload,
                modifier = modifier.padding(top = if (block.collapsible) 16.dp else 0.dp),
            )
        }
        is SessionBlock.Pending ->
            ExpandableCard(
                title = "${countOf(block.count, "transaction")} ${if (block.count == 1L) "waits" else "wait"} on your answers",
                expanded = expanded,
                onToggle = onToggle,
                modifier = modifier,
            ) {
                state.pending.take(MAX_LISTED_ROWS).forEach { row ->
                    RowDivider()
                    SimpleRow(
                        title = row.description ?: row.accountRef,
                        detail = listOf(importDate(row.date), row.accountRef).joinToString(" · "),
                        trailing = row.amountsDisplay,
                    )
                }
                if (state.pending.size > MAX_LISTED_ROWS) {
                    RowDivider()
                    DetailLine(
                        "Showing $MAX_LISTED_ROWS of ${formatCount(state.pending.size.toLong())} transactions",
                        Modifier.padding(horizontal = 20.dp, vertical = 12.dp),
                    )
                }
            }
        is SessionBlock.Duplicate ->
            PossibleDuplicateCard(
                row = block.row,
                progress = block.progress,
                selectedId = state.duplicateChoices[block.row.id],
                enabled = enabled,
                onSelect = { actions.onChooseDuplicate(block.row.id, it) },
                onConfirm = { actions.onConfirmDuplicate(block.row.id) },
                modifier = modifier,
            )
        is SessionBlock.StartingBalance ->
            StartingBalanceCard(
                row = block.row,
                enabled = enabled,
                onAdd = { actions.onSetVerb(listOf(block.row.id), ImportItemVerb.ADD, null) },
                onLeaveOut = { actions.onSetVerb(listOf(block.row.id), ImportItemVerb.SKIP, null) },
                modifier = modifier,
            )
        is SessionBlock.BalancedAccounts -> BalancedAccountsCard(block.checks, modifier)
        is SessionBlock.BalanceCheck ->
            BalanceCheckCard(
                check = block.check,
                question = block.question,
                entered = block.entered,
                adjustments = block.adjustments,
                enabled = enabled,
                onEnterBalance = actions.onEnterBalance,
                onReviewGaps = { actions.onReviewGaps(block.key) },
                modifier = modifier,
            )
        is SessionBlock.Note -> DetailLine(block.text, modifier.padding(horizontal = 4.dp))
        is SessionBlock.Unstageable ->
            ExpandableCard(
                title = "${countOf(block.count.toLong(), "transaction")} couldn't be imported",
                expanded = expanded,
                onToggle = onToggle,
                modifier = modifier,
            ) {
                state.items?.unstageable?.forEach { row ->
                    RowDivider()
                    SimpleRow(
                        title = row.description ?: "No description",
                        detail = "${importDate(row.date)} · ${row.reason}",
                        trailing = null,
                    )
                }
            }
    }
}

@Composable
private fun AssetQuestion(
    ask: ImportAsk.AssetMapping,
    state: ImportSessionUiState,
    actions: SessionActions,
    modifier: Modifier,
) {
    val chosen = state.assetChoices[ask.askKey]
    val options =
        ask.candidates.map { QuestionOption(it.id.toString(), it.ticker, it.name) } +
            listOfNotNull(
                chosen
                    ?.takeIf { pick -> ask.candidates.none { it.id == pick.id } }
                    ?.let { QuestionOption(it.id.toString(), it.ticker, it.name) },
            )
    val selected = chosen?.id ?: ask.candidates.firstOrNull()?.id
    AssetQuestionCard(
        raw = ask.raw,
        options = options,
        selectedId = selected?.toString(),
        canExclude = ask.canExclude,
        enabled = !state.working,
        onSelect = { id ->
            (ask.candidates.firstOrNull { it.id.toString() == id } ?: chosen)?.let {
                actions.onChooseAsset(ask.askKey, it)
            }
        },
        onSearch = { actions.onSearchAsset(ask) },
        onLeaveOut = { actions.onAnswerAsset(ask.askKey, null) },
        onLink = { selected?.let { actions.onAnswerAsset(ask.askKey, it) } },
        modifier = modifier,
    )
}

@Composable
private fun AccountQuestion(
    block: SessionBlock.AccountQuestion,
    state: ImportSessionUiState,
    actions: SessionActions,
    modifier: Modifier,
) {
    val raw = block.rawAccount
    val asks = block.asks
    val coversAll = asks.any { it.answerCoversAllSources }
    val splittable = asks.size > 1 && !coversAll
    val same = raw !in state.separateAccounts
    val targets = if (coversAll || !same) asks.take(1) else asks
    val candidates = asks.flatMap { it.candidates }.distinctBy { it.id }
    val chosen = state.accountChoices[raw]
    val extra =
        chosen
            ?.takeIf { id -> candidates.none { it.id == id } }
            ?.let { id -> state.accounts.orEmpty().firstOrNull { it.id == id } }
            ?.let { QuestionOption(it.id, it.name) }
    val selected = chosen ?: candidates.firstOrNull()?.id
    val sourceNames =
        state.detail
            ?.sources
            .orEmpty()
            .associate { it.id to sourceLabel(it) }
    AccountQuestionCard(
        rawAccount = raw,
        sourceNames = (if (coversAll) asks else targets).mapNotNull { sourceNames[it.sourceId]?.takeIf(String::isNotBlank) }.distinct(),
        options = candidates.map { QuestionOption(it.id, it.name) } + listOfNotNull(extra),
        selectedId = selected,
        canExclude = asks.all { it.canExclude },
        sameAccount = if (splittable) same else null,
        enabled = !state.working,
        onSelect = { actions.onChooseAccount(raw, it) },
        onChooseAnother = { actions.onPickAccount(raw) },
        onCreateAccount = { actions.onCreateAccount(raw) },
        onSameAccountChange = { actions.onSameAccount(raw, it) },
        onExclude = { actions.onAnswerAccounts(targets.map { it.askKey }, null) },
        onLink = { selected?.let { id -> actions.onAnswerAccounts(targets.map { it.askKey }, id) } },
        modifier = modifier,
    )
}

@Composable
private fun SimpleRow(
    title: String,
    detail: String,
    trailing: String?,
) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 12.dp)) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            DetailLine(detail)
        }
        trailing?.let {
            Spacer(Modifier.width(12.dp))
            Text(it, style = MaterialTheme.typography.bodyLarge)
        }
    }
}
