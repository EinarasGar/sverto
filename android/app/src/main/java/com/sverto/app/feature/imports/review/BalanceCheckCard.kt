@file:OptIn(ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports.review

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import com.sverto.app.core.ui.RowDivider
import com.sverto.app.feature.imports.countOf
import uniffi.sverto_core.ImportAsk
import uniffi.sverto_core.ImportBalanceCheck
import uniffi.sverto_core.ImportEnteredBalance
import uniffi.sverto_core.ImportRowVerb
import uniffi.sverto_core.ImportStagedRow
import uniffi.sverto_core.ImportUnlistedMovement

fun ImportBalanceCheck.title(): String = listOf(accountName, ticker).filter { it.isNotBlank() }.joinToString(" · ")

private fun ImportBalanceCheck.coverage(): String =
    "${countOf(balancesChecked.toLong(), "balance")} checked · ${importDateRange(firstDate, lastDate)}"

@Composable
fun BalancedAccountsCard(
    checks: List<ImportBalanceCheck>,
    modifier: Modifier = Modifier,
) {
    Surface(modifier = modifier.fillMaxWidth(), shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceBright) {
        Column {
            checks.forEachIndexed { index, check ->
                if (index > 0) RowDivider()
                Row(
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 64.dp)
                            .padding(horizontal = 20.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(check.title(), style = MaterialTheme.typography.bodyLarge)
                        DetailLine(check.coverage())
                    }
                    ToneChip(label = "Balanced", tone = ImportTone.Positive, icon = "check")
                }
            }
        }
    }
}

@Composable
fun BalanceCheckCard(
    check: ImportBalanceCheck,
    question: ImportAsk.NoCheckpoint?,
    entered: ImportEnteredBalance?,
    adjustments: List<ImportStagedRow>,
    enabled: Boolean,
    onEnterBalance: (askKey: String, quantity: String) -> Unit,
    onReviewGaps: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val added = adjustments.count { it.verb == ImportRowVerb.ADD }
    ReviewCard(modifier) {
        FlowRow(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalArrangement = Arrangement.spacedBy(6.dp),
            itemVerticalAlignment = Alignment.CenterVertically,
        ) {
            Column {
                Text(check.title(), style = MaterialTheme.typography.titleMedium)
                DetailLine(check.coverage())
            }
            BalanceChip(check)
        }
        balanceSummary(check, added)?.let { DetailLine(it) }
        question?.let { ask ->
            Text("What was the balance on ${importDate(ask.asOfDate)}?", style = MaterialTheme.typography.bodyLarge)
            BalanceEntryField(label = "Balance", suffix = ask.ticker, enabled = enabled, onSubmit = { onEnterBalance(ask.askKey, it) })
        }
        entered?.let { EnteredBalance(it, check.ticker, enabled, onEnterBalance) }
        if (check.changes.isNotEmpty() || check.unlisted.isNotEmpty() || added > 0) {
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .clickable(onClickLabel = "Review the gaps in ${check.title()}", onClick = onReviewGaps)
                        .heightIn(min = 48.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = if (check.changes.isEmpty()) "Review details" else "Review ${countOf(check.changes.size.toLong(), "gap")}",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.weight(1f),
                )
                Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
            }
        }
    }
}

@Composable
private fun BalanceChip(check: ImportBalanceCheck) {
    when {
        check.finalOffBy != 0.0 -> ToneChip(label = "Off by ${check.finalOffByDisplay}", tone = ImportTone.Negative)
        check.startingOffBy == 0.0 && check.changes.isEmpty() -> ToneChip(label = "Balanced", tone = ImportTone.Positive, icon = "check")
        else -> ToneChip(label = "Balanced at the end", tone = ImportTone.Ghost)
    }
}

private fun balanceSummary(
    check: ImportBalanceCheck,
    added: Int,
): String? {
    val parts =
        listOfNotNull(
            check.changes.size.takeIf { it > 0 }?.let {
                "${countOf(it.toLong(), "gap")} between statement balances that nothing in your ledger explains."
            },
            check.startingOffByDisplay.takeIf { check.startingOffBy != 0.0 }?.let {
                "Off by $it before the first balance, from history before this statement."
            },
            check.unlisted.count { !it.mayBookLater }.takeIf { it > 0 }?.let {
                "${countOf(it.toLong(), "transaction")} in your ledger ${if (it == 1) "isn't" else "aren't"} on this statement."
            },
            check.unlisted.count { it.deleting }.takeIf { it > 0 }?.let {
                "$it ${if (it == 1) "is" else "are"} deleted when you apply."
            },
            added.takeIf { it > 0 }?.let { "Includes ${countOf(it.toLong(), "adjustment")} you added." },
        )
    return parts.takeIf { it.isNotEmpty() }?.joinToString(" ")
}

@Composable
private fun EnteredBalance(
    entered: ImportEnteredBalance,
    ticker: String,
    enabled: Boolean,
    onEnterBalance: (askKey: String, quantity: String) -> Unit,
) {
    var changing by rememberSaveable(entered.askKey) { mutableStateOf(false) }
    if (changing) {
        Text("What was the balance on ${importDate(entered.asOfDate)}?", style = MaterialTheme.typography.bodyLarge)
        BalanceEntryField(
            label = "Balance",
            suffix = ticker,
            enabled = enabled,
            onSubmit = {
                changing = false
                onEnterBalance(entered.askKey, it)
            },
        )
    } else {
        ActionLine(
            text = "Checked against the balance you entered: ${entered.quantityDisplay} at the end of ${importDate(entered.date)}.",
            action = "Change",
            actionLabel = "Change the balance you entered for ${importDate(entered.date)}",
            enabled = enabled,
            onAction = { changing = true },
        )
    }
}

@Composable
internal fun UnlistedGroup(
    title: String,
    rows: List<ImportUnlistedMovement>,
    enabled: Boolean,
    onAnswer: ((askKey: String, delete: Boolean) -> Unit)?,
) {
    if (rows.isEmpty()) return
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(title, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        rows.groupBy { it.askKey ?: it.transactionId }.values.forEach { group ->
            val first = group.first()
            val subject = listOfNotNull(first.description, first.amountDisplay, importDate(first.date)).joinToString(", ")
            val askKey = first.askKey
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    group.forEach { row ->
                        Text(
                            text = listOfNotNull(importDate(row.date), row.description, row.amountDisplay).joinToString(" · "),
                            style = MaterialTheme.typography.bodyMedium,
                            textDecoration = if (row.deleting) TextDecoration.LineThrough else null,
                            color = if (row.deleting) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
                        )
                        if (row.explainsDifference) {
                            Text(
                                "Explains the difference",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                    if (first.deleting) {
                        Text("Deleted when you apply", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                    }
                }
                if (onAnswer != null && askKey != null) {
                    TextButton(
                        onClick = { onAnswer(askKey, !first.deleting) },
                        enabled = enabled,
                        shapes = ButtonDefaults.shapes(),
                        modifier =
                            Modifier.semantics {
                                contentDescription = if (first.deleting) "Keep $subject" else "Delete $subject when you apply"
                            },
                    ) { Text(if (first.deleting) "Keep" else "Delete on apply") }
                }
            }
        }
    }
}

@Composable
private fun ActionLine(
    text: String,
    action: String,
    actionLabel: String,
    enabled: Boolean,
    onAction: () -> Unit,
) {
    Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        DetailLine(text, Modifier.weight(1f))
        TextButton(
            onClick = onAction,
            enabled = enabled,
            shapes = ButtonDefaults.shapes(),
            modifier = Modifier.semantics { contentDescription = actionLabel },
        ) { Text(action) }
    }
}
