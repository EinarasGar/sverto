@file:OptIn(ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports.review

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.sverto_core.ImportCandidate
import uniffi.sverto_core.ImportStagedRow

const val NEW_TRANSACTION_CHOICE = "new"

@Composable
fun PossibleDuplicateCard(
    row: ImportStagedRow,
    progress: String?,
    selectedId: String?,
    enabled: Boolean,
    onSelect: (String) -> Unit,
    onConfirm: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val options =
        row.nearMiss.map { candidate ->
            QuestionOption(
                id = candidate.id,
                title = listOfNotNull(candidate.description ?: "No description", importDate(candidate.date)).joinToString(" · "),
                subtitle = listOfNotNull(candidate.amountDisplay, candidate.accountName).joinToString(" · "),
                note = candidateNote(candidate),
            )
        } + QuestionOption(NEW_TRANSACTION_CHOICE, "None of these — it's new")
    QuestionSurface(modifier) {
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 20.dp)
                    .padding(top = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("Is this already recorded?", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
            progress?.let { SectionCount(it) }
        }
        Surface(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 20.dp, vertical = 12.dp),
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.surfaceContainer,
        ) {
            Row(Modifier.padding(16.dp), verticalAlignment = Alignment.Top) {
                Column(Modifier.weight(1f)) {
                    Text(
                        text = row.description,
                        style = MaterialTheme.typography.bodyLarge,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                    DetailLine(listOf(importDate(row.date), row.accountName).filter { it.isNotBlank() }.joinToString(" · "))
                }
                Spacer(Modifier.width(12.dp))
                Text(text = row.amountDisplay, style = MaterialTheme.typography.bodyLarge)
            }
        }
        Text(
            text = if (row.nearMiss.size == 1) "Possible match in your ledger" else "Possible matches in your ledger",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 20.dp),
        )
        OptionList(options = options, selectedId = selectedId, enabled = enabled, onSelect = onSelect)
        CardActions(Modifier.padding(horizontal = 20.dp).padding(top = 8.dp)) {
            Button(
                onClick = onConfirm,
                enabled = enabled && selectedId != null,
                shapes = ButtonDefaults.shapes(),
                modifier =
                    Modifier.semantics {
                        contentDescription = "Confirm the choice for ${row.description}, ${row.amountDisplay}"
                    },
            ) { Text("Confirm") }
        }
    }
}

private fun candidateNote(candidate: ImportCandidate): String? =
    when {
        candidate.parts.isNotEmpty() ->
            "${candidate.parts.size} transactions in your ledger add up to it: " +
                candidate.parts.joinToString("; ") { part ->
                    listOfNotNull(importShortDate(part.date), part.description, part.amountDisplay).joinToString(" ")
                }
        !candidate.inLedger -> "From another file in this import"
        candidate.leavesMovementUnrecorded -> "Covers only part of this transaction"
        candidate.amend -> "Takes the statement's amount"
        else -> null
    }
