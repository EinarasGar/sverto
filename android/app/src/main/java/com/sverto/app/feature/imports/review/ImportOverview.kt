@file:OptIn(ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports.review

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.sverto.app.core.icons.LucideIcon
import com.sverto.app.core.ui.RowDivider
import com.sverto.app.feature.imports.formatCount
import uniffi.sverto_core.ImportStagedRow

data class BalanceLine(
    val text: String,
    val ok: Boolean,
)

data class OutcomeState(
    val headline: String,
    val breakdown: String?,
    val scope: String,
    val balance: BalanceLine?,
)

data class DecisionNavItem(
    val label: String,
    val progress: String?,
    val target: String,
)

@Composable
fun OutcomeCard(
    state: OutcomeState,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.secondaryContainer,
    ) {
        Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                text = state.headline,
                style = MaterialTheme.typography.headlineSmall,
                color = MaterialTheme.colorScheme.onSecondaryContainer,
                modifier = Modifier.semantics { heading() },
            )
            state.breakdown?.let {
                Text(it, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSecondaryContainer)
            }
            Text(
                text = state.scope,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSecondaryContainer,
            )
            state.balance?.let { line ->
                Spacer(Modifier.size(4.dp))
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    val tint = if (line.ok) MaterialTheme.colorScheme.onSecondaryContainer else MaterialTheme.colorScheme.error
                    LucideIcon(name = if (line.ok) "circle-check" else "circle-alert", tint = tint, modifier = Modifier.size(18.dp))
                    Text(
                        text = line.text,
                        style = MaterialTheme.typography.labelLarge,
                        color = tint,
                    )
                }
            }
        }
    }
}

@Composable
fun DecisionNavCard(
    items: List<DecisionNavItem>,
    onOpen: (DecisionNavItem) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            text = "Needs your decision",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
            modifier =
                Modifier
                    .padding(start = 4.dp)
                    .semantics { heading() },
        )
        Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceBright) {
            Column {
                items.forEachIndexed { index, item ->
                    if (index > 0) RowDivider()
                    Row(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clickable(onClickLabel = "Go to ${item.label}") { onOpen(item) }
                                .heightIn(min = 56.dp)
                                .padding(start = 20.dp, end = 12.dp, top = 8.dp, bottom = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Box(
                            Modifier
                                .size(8.dp)
                                .clip(RoundedCornerShape(50))
                                .background(MaterialTheme.colorScheme.tertiary),
                        )
                        Spacer(Modifier.width(16.dp))
                        Text(item.label, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                        item.progress?.let { SectionCount(it) }
                        Icon(
                            Icons.AutoMirrored.Filled.KeyboardArrowRight,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun QuestionsHeading(
    count: Int,
    modifier: Modifier = Modifier,
) {
    Text(
        text = if (count == 1) "1 question to finish this import" else "$count questions to finish this import",
        style = MaterialTheme.typography.titleLarge,
        modifier =
            modifier
                .padding(start = 4.dp, top = 8.dp, bottom = 4.dp)
                .semantics { heading() },
    )
}

@Composable
fun PreviewRowsCard(
    rows: List<ImportStagedRow>,
    total: Int,
    onRowClick: (ImportStagedRow) -> Unit,
    onViewAll: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SectionHeader(title = "Transactions", trailing = { SectionCount(formatCount(total.toLong())) })
        Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceBright) {
            Column {
                rows.forEachIndexed { index, row ->
                    if (index > 0) RowDivider()
                    ProposedTransactionRow(row = row, onClick = { onRowClick(row) }, showDate = true)
                }
                if (total > rows.size) {
                    RowDivider()
                    Row(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clickable(onClick = onViewAll)
                                .heightIn(min = 56.dp)
                                .padding(start = 16.dp, end = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = "View all $total transactions",
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.weight(1f),
                        )
                        Icon(
                            Icons.AutoMirrored.Filled.KeyboardArrowRight,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.primary,
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun ImportReceipt(
    lines: List<String>,
    onViewTransactions: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier =
            modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp, vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier =
                Modifier
                    .size(72.dp)
                    .clip(MaterialShapes.Cookie9Sided.toShape())
                    .background(MaterialTheme.colorScheme.secondaryContainer),
            contentAlignment = Alignment.Center,
        ) {
            LucideIcon(name = "check", tint = MaterialTheme.colorScheme.onSecondaryContainer, modifier = Modifier.size(32.dp))
        }
        Spacer(Modifier.size(16.dp))
        Text(
            text = "Import applied",
            style = MaterialTheme.typography.headlineSmall,
            textAlign = TextAlign.Center,
            modifier = Modifier.semantics { heading() },
        )
        Spacer(Modifier.size(6.dp))
        lines.forEach {
            Text(
                text = it,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
        }
        Spacer(Modifier.size(20.dp))
        Button(
            onClick = onViewTransactions,
            shapes = ButtonDefaults.shapes(),
            modifier = Modifier.heightIn(min = ButtonDefaults.MediumContainerHeight),
            contentPadding = ButtonDefaults.MediumContentPadding,
        ) {
            Text("View in Transactions", style = MaterialTheme.typography.titleMedium)
        }
    }
}
