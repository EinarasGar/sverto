@file:OptIn(ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports.review

import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import uniffi.sverto_core.ImportStagedRow

@Composable
fun StartingBalanceCard(
    row: ImportStagedRow,
    enabled: Boolean,
    onAdd: () -> Unit,
    onLeaveOut: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val account = row.accountName.ifBlank { "this account" }
    ReviewCard(modifier) {
        Text(
            text = "Start $account on ${importDate(row.date)} at ${row.amountDisplay}?",
            style = MaterialTheme.typography.titleMedium,
        )
        DetailLine(
            "The statement's first balance needs this before its first row, on top of what your ledger " +
                "already holds. Add it, leave it out, or add the statement before this one.",
        )
        val subject = "the starting balance of $account on ${importDate(row.date)}, ${row.amountDisplay}"
        CardActions {
            TextButton(
                onClick = onLeaveOut,
                enabled = enabled,
                shapes = ButtonDefaults.shapes(),
                modifier = Modifier.semantics { contentDescription = "Leave out $subject" },
            ) { Text("Leave out") }
            FilledTonalButton(
                onClick = onAdd,
                enabled = enabled,
                shapes = ButtonDefaults.shapes(),
                modifier = Modifier.semantics { contentDescription = "Add $subject" },
            ) { Text("Add") }
        }
    }
}
