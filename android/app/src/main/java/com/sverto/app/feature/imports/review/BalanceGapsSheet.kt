@file:OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports.review

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.sverto.app.core.ui.RowDivider
import uniffi.sverto_core.ImportBalanceChange
import uniffi.sverto_core.ImportBalanceCheck
import uniffi.sverto_core.ImportRowVerb
import uniffi.sverto_core.ImportStagedRow

private const val SECONDS_PER_DAY = 86_400L

private fun utcDay(epochSeconds: Long): Long = Math.floorDiv(epochSeconds, SECONDS_PER_DAY)

fun adjustmentOffer(
    change: ImportBalanceChange,
    adjustments: List<ImportStagedRow>,
): ImportStagedRow? =
    if (change.explained || change.after == change.through) {
        null
    } else {
        adjustments.firstOrNull { it.verb != ImportRowVerb.ADD && utcDay(it.date) == utcDay(change.through) }
    }

@Composable
fun BalanceGapsSheet(
    check: ImportBalanceCheck,
    adjustments: List<ImportStagedRow>,
    enabled: Boolean,
    onSetAdjustments: (rowIds: List<String>, add: Boolean) -> Unit,
    onAnswerUnlisted: (askKey: String, delete: Boolean) -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val offers = check.changes.mapNotNull { change -> adjustmentOffer(change, adjustments)?.let { change to it } }
    val added = adjustments.filter { it.verb == ImportRowVerb.ADD }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        modifier = modifier,
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        LazyColumn(
            modifier = Modifier.fillMaxWidth(),
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item(key = "head") {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(check.title(), style = MaterialTheme.typography.headlineSmall)
                    if (check.changes.isNotEmpty()) {
                        DetailLine(
                            "Nothing in your ledger explains these changes between statement balances. Add the statements " +
                                "for those dates, or add an adjustment that closes the gap.",
                        )
                    }
                    if (offers.size > 1) {
                        FilledTonalButton(
                            onClick = { onSetAdjustments(offers.map { it.second.id }, true) },
                            enabled = enabled,
                            shapes = ButtonDefaults.shapes(),
                            modifier = Modifier.semantics { contentDescription = "Add all ${offers.size} adjustments" },
                        ) { Text("Add all ${offers.size}") }
                    }
                }
            }
            if (check.changes.isNotEmpty()) {
                item(key = "gaps") {
                    Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceBright) {
                        Column {
                            check.changes.forEachIndexed { index, change ->
                                if (index > 0) RowDivider()
                                GapRow(change, adjustmentOffer(change, adjustments), enabled) { onSetAdjustments(listOf(it), true) }
                            }
                        }
                    }
                }
            }
            item(key = "unlisted") {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    UnlistedGroup(
                        title = "In your ledger but not on this statement",
                        rows = check.unlisted.filterNot { it.mayBookLater },
                        enabled = enabled,
                        onAnswer = onAnswerUnlisted,
                    )
                    UnlistedGroup(
                        title = "May book later: dated in the statement's last days, so the bank may book these on the next statement",
                        rows = check.unlisted.filter { it.mayBookLater },
                        enabled = enabled,
                        onAnswer = null,
                    )
                }
            }
            if (added.isNotEmpty()) {
                item(key = "added-title") {
                    Text(
                        text = "Adjustments you added",
                        style = MaterialTheme.typography.titleSmall,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                items(added, key = { it.id }) { row ->
                    AddedAdjustment(row, enabled) { onSetAdjustments(listOf(row.id), false) }
                }
            }
        }
    }
}

@Composable
private fun GapRow(
    change: ImportBalanceChange,
    offer: ImportStagedRow?,
    enabled: Boolean,
    onAdd: (String) -> Unit,
) {
    val period = importDateRange(change.after, change.through)
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = 64.dp)
                .padding(start = 20.dp, end = 8.dp, top = 10.dp, bottom = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                text = if (change.after == change.through) "${importDate(change.through)}: two statements disagree" else period,
                style = MaterialTheme.typography.bodyLarge,
            )
            Text(change.changeDisplay, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            if (change.awaitsAnswer) {
                Text(
                    "A possible duplicate on these dates is still open; it may settle this.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        offer?.let { row ->
            TextButton(
                onClick = { onAdd(row.id) },
                enabled = enabled,
                shapes = ButtonDefaults.shapes(),
                modifier = Modifier.semantics { contentDescription = "Add adjustment of ${row.amountDisplay}, $period" },
            ) { Text("Add") }
        }
    }
}

@Composable
private fun AddedAdjustment(
    row: ImportStagedRow,
    enabled: Boolean,
    onRemove: () -> Unit,
) {
    Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceBright) {
        FlowRow(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 56.dp)
                    .padding(start = 20.dp, end = 8.dp, top = 8.dp, bottom = 8.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            itemVerticalAlignment = Alignment.CenterVertically,
        ) {
            Text("${row.amountDisplay} on ${importDate(row.date)}", style = MaterialTheme.typography.bodyLarge)
            TextButton(
                onClick = onRemove,
                enabled = enabled,
                shapes = ButtonDefaults.shapes(),
                modifier =
                    Modifier.semantics {
                        contentDescription = "Remove adjustment of ${row.amountDisplay} on ${importDate(
                            row.date,
                        )}"
                    },
            ) { Text("Remove") }
        }
    }
}
