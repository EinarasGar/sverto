package com.sverto.app.feature.transactions.quickupload

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.sverto.app.feature.imports.review.importDate
import com.sverto.app.feature.transactions.create.SectionCard
import com.sverto.app.feature.transactions.held.ComparedTransaction

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LedgerMatchCard(
    matches: LedgerMatches,
    onShowNext: () -> Unit,
    onMergeChange: (Boolean) -> Unit,
) {
    val candidate = matches.selected ?: return
    val count = matches.candidates.size
    SectionCard(label = "Already in your ledger") {
        ComparedTransaction(
            label =
                if (count > 1) {
                    "Same amount · ${matches.candidates.indexOf(candidate) + 1} of $count"
                } else {
                    "Same amount"
                },
            icon = candidate.categoryIcon ?: "receipt",
            description = candidate.description ?: "No description",
            detail = "${candidate.accountName} · ${importDate(candidate.date)}",
            amount = candidate.amountDisplay,
        )
        if (count > 1) {
            TextButton(onClick = onShowNext) { Text("Compare with another (${count - 1})") }
        } else {
            Spacer(Modifier.height(12.dp))
        }
        Row(
            modifier = Modifier.fillMaxWidth().height(IntrinsicSize.Min),
            horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween),
        ) {
            val colors = ToggleButtonDefaults.toggleButtonColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHighest)
            listOf(true to "Merge into it", false to "It's a different purchase").forEachIndexed { index, (merge, label) ->
                ToggleButton(
                    checked = matches.merge == merge,
                    onCheckedChange = { onMergeChange(merge) },
                    shapes =
                        if (index == 0) {
                            ButtonGroupDefaults.connectedLeadingButtonShapes()
                        } else {
                            ButtonGroupDefaults.connectedTrailingButtonShapes()
                        },
                    colors = colors,
                    modifier =
                        Modifier
                            .weight(1f)
                            .fillMaxHeight()
                            .semantics { role = Role.RadioButton },
                ) { Text(label, textAlign = TextAlign.Center) }
            }
        }
        Text(
            text =
                if (matches.merge) {
                    "The receipt's lines and date replace it; its account stays."
                } else {
                    "The receipt is saved as a new transaction."
                },
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 4.dp, top = 8.dp),
        )
    }
}
