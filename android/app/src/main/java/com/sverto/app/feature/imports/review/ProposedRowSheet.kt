@file:OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports.review

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.sverto.app.core.ui.RowDivider
import com.sverto.app.feature.imports.amendRefusalNote
import com.sverto.app.feature.imports.kindNote
import com.sverto.app.feature.transactions.signedSecondary
import uniffi.sverto_core.ImportItemVerb
import uniffi.sverto_core.ImportRowKind
import uniffi.sverto_core.ImportRowVerb
import uniffi.sverto_core.ImportStagedRow

@Composable
fun ProposedRowSheet(
    row: ImportStagedRow,
    onVerb: (ImportItemVerb) -> Unit,
    onKeep: (askKey: String) -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        modifier = modifier,
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 24.dp)
                    .padding(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            SheetHeader(row)
            SheetDetails(row)
            listOfNotNull(
                kindNote(row.kind),
                row.nearMiss.takeIf { it.isNotEmpty() }?.let {
                    "Possible duplicate: settle it on the import screen."
                },
            ).forEach { DetailLine(it) }
            row.amendRefusal?.let {
                Text(
                    text = amendRefusalNote(it),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                )
            }
            SheetActions(row, onVerb, onKeep)
        }
    }
}

@Composable
private fun SheetHeader(row: ImportStagedRow) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Box(
            modifier =
                Modifier
                    .size(56.dp)
                    .clip(MaterialShapes.Cookie9Sided.toShape())
                    .background(MaterialTheme.colorScheme.secondaryContainer),
            contentAlignment = Alignment.Center,
        ) {
            ProposedRowGlyph(row, Modifier.size(28.dp), MaterialTheme.colorScheme.onSecondaryContainer)
        }
        Column(Modifier.weight(1f)) {
            Text(text = row.description, style = MaterialTheme.typography.titleLarge)
            Text(text = row.amountDisplay, style = MaterialTheme.typography.headlineSmall)
            row.secondaryAmountDisplay?.let {
                Text(
                    text = signedSecondary(it),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun SheetDetails(row: ImportStagedRow) {
    Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceBright) {
        Column {
            DetailRow("Date", importDate(row.date))
            RowDivider()
            DetailRow("Account", row.accountName)
            RowDivider()
            DetailRow(if (row.categoryName != null) "Category" else "Type", row.categoryName ?: row.typeLabel)
        }
    }
}

@Composable
private fun DetailRow(
    label: String,
    value: String,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = 52.dp)
                .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        DetailLine(label)
        Text(
            text = value,
            style = MaterialTheme.typography.bodyLarge,
            modifier = Modifier.weight(1f),
            textAlign = TextAlign.End,
        )
    }
}

@Composable
private fun SheetActions(
    row: ImportStagedRow,
    onVerb: (ImportItemVerb) -> Unit,
    onKeep: (askKey: String) -> Unit,
) {
    val askKey = row.askKey
    if (row.verb == ImportRowVerb.DELETE) {
        if (askKey != null) {
            DetailLine("This ledger transaction isn't on the statement, so the import deletes it.")
            FilledTonalButton(
                onClick = { onKeep(askKey) },
                shapes = ButtonDefaults.shapes(),
                modifier = Modifier.fillMaxWidth(),
            ) { Text("Keep it") }
        }
        return
    }
    Text(
        text = if (row.verbLocked) "In this import · your choice" else "In this import",
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    val colors = ToggleButtonDefaults.toggleButtonColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHighest)
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween),
    ) {
        listOf(ImportRowVerb.ADD to ImportItemVerb.ADD, ImportRowVerb.SKIP to ImportItemVerb.SKIP)
            .forEachIndexed { index, (rowVerb, itemVerb) ->
                ToggleButton(
                    checked = row.verb == rowVerb,
                    onCheckedChange = { if (row.verb != rowVerb) onVerb(itemVerb) },
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
                            .semantics { role = Role.RadioButton },
                ) { Text(if (rowVerb == ImportRowVerb.ADD) "Add" else "Skip") }
            }
    }
    if (row.kind == ImportRowKind.JOINED_TRANSFER) {
        OutlinedButton(
            onClick = { onVerb(ImportItemVerb.APART) },
            shapes = ButtonDefaults.shapes(),
            modifier = Modifier.fillMaxWidth(),
        ) { Text("Keep apart") }
    }
    if (row.verbLocked) {
        TextButton(
            onClick = { onVerb(ImportItemVerb.AUTO) },
            shapes = ButtonDefaults.shapes(),
            modifier = Modifier.fillMaxWidth(),
        ) { Text("Undo my choice") }
    }
}
