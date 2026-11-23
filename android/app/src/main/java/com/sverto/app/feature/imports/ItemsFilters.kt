@file:OptIn(ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.sverto.app.core.icons.LucideIcon
import uniffi.sverto_core.ImportRowKind
import uniffi.sverto_core.ImportRowVerb
import uniffi.sverto_core.ImportStagedRow

enum class ItemsFilter(
    val label: String,
    val verb: ImportRowVerb?,
) {
    All("All", null),
    Add("Add", ImportRowVerb.ADD),
    Amend("Amend", ImportRowVerb.AMEND),
    Skip("Skip", ImportRowVerb.SKIP),
    Delete("Delete", ImportRowVerb.DELETE),
    ;

    companion object {
        fun from(value: String?): ItemsFilter = entries.firstOrNull { it.name.equals(value, ignoreCase = true) } ?: All
    }
}

data class AccountChoice(
    val id: String,
    val name: String,
    val count: Int,
)

fun needsALook(row: ImportStagedRow): Boolean =
    row.verb == ImportRowVerb.AMEND ||
        row.verb == ImportRowVerb.DELETE ||
        row.kind == ImportRowKind.JOINED_TRANSFER ||
        row.nearMiss.isNotEmpty() ||
        (row.typeKey == "regular" && (row.categoryName == null || row.categoryName == "Miscellaneous"))

fun accountChoices(rows: List<ImportStagedRow>): List<AccountChoice> =
    rows
        .groupBy { it.accountId }
        .map { (id, accountRows) -> AccountChoice(id, accountRows.first().accountName.substringBefore(" → "), accountRows.size) }
        .sortedByDescending { it.count }

fun matchesQuery(
    row: ImportStagedRow,
    query: String,
): Boolean {
    val text = query.trim()
    if (text.isEmpty()) return true
    if (row.description.contains(text, ignoreCase = true)) return true
    val digits = text.filter { it.isDigit() || it == '.' }
    return digits.any { it.isDigit() } && row.amountDisplay.filter { it.isDigit() || it == '.' }.contains(digits)
}

@Composable
fun ItemsFilterBar(
    needsLookCount: Int,
    needsLook: Boolean,
    onNeedsLook: (Boolean) -> Unit,
    accounts: List<AccountChoice>,
    accountId: String?,
    onAccount: (String?) -> Unit,
    verbs: List<ItemsFilter>,
    verb: ItemsFilter,
    onVerb: (ItemsFilter) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (needsLookCount > 0 || accounts.size > 1) {
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .padding(horizontal = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (needsLookCount > 0) {
                    FilterChip(
                        selected = needsLook,
                        onClick = { onNeedsLook(!needsLook) },
                        label = { Text("Needs a look · $needsLookCount") },
                        leadingIcon = { LucideIcon(name = "eye", modifier = Modifier.size(FilterChipDefaults.IconSize)) },
                    )
                }
                if (accounts.size > 1) {
                    accounts.forEach { account ->
                        FilterChip(
                            selected = accountId == account.id,
                            onClick = { onAccount(if (accountId == account.id) null else account.id) },
                            label = { Text("${account.name} · ${account.count}") },
                        )
                    }
                }
            }
        }
        if (verbs.size > 2) {
            Row(
                modifier = Modifier.padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text("Show", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                VerbGroup(verbs, verb, onVerb, Modifier.weight(1f))
            }
        }
    }
}

@Composable
private fun VerbGroup(
    filters: List<ItemsFilter>,
    selected: ItemsFilter,
    onSelect: (ItemsFilter) -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(modifier = modifier, horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
        val colors = ToggleButtonDefaults.toggleButtonColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHighest)
        filters.forEachIndexed { index, entry ->
            ToggleButton(
                checked = entry == selected,
                onCheckedChange = { onSelect(entry) },
                modifier =
                    Modifier
                        .weight(1f)
                        .semantics { role = Role.RadioButton },
                shapes =
                    when (index) {
                        0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                        filters.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                        else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                    },
                colors = colors,
                contentPadding = PaddingValues(horizontal = 8.dp),
            ) { Text(entry.label, maxLines = 1) }
        }
    }
}
