@file:Suppress("MatchingDeclarationName")
@file:OptIn(ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports.review

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sverto.app.core.icons.LucideIcon
import java.text.DecimalFormatSymbols

data class QuestionOption(
    val id: String,
    val title: String,
    val subtitle: String? = null,
    val note: String? = null,
)

@Composable
fun AccountQuestionCard(
    rawAccount: String,
    sourceNames: List<String>,
    options: List<QuestionOption>,
    selectedId: String?,
    canExclude: Boolean,
    sameAccount: Boolean?,
    enabled: Boolean,
    onSelect: (String) -> Unit,
    onChooseAnother: () -> Unit,
    onCreateAccount: () -> Unit,
    onSameAccountChange: (Boolean) -> Unit,
    onExclude: () -> Unit,
    onLink: () -> Unit,
    modifier: Modifier = Modifier,
) {
    QuestionSurface(modifier) {
        QuestionTitle(
            title = "Which account is “$rawAccount”?",
            supporting = sourceNames.takeIf { it.isNotEmpty() }?.let { "From ${it.joinToString(", ")}" },
        )
        OptionList(options = options, selectedId = selectedId, enabled = enabled, onSelect = onSelect)
        ExtraOptionRow(icon = "search", label = "Choose another account…", enabled = enabled, onClick = onChooseAnother)
        ExtraOptionRow(icon = "plus", label = "Create a new account", enabled = enabled, onClick = onCreateAccount)
        if (sameAccount != null) {
            SameAccountRow(checked = sameAccount, enabled = enabled, onCheckedChange = onSameAccountChange)
        }
        CardActions(Modifier.padding(horizontal = 20.dp).padding(top = 8.dp)) {
            if (canExclude) {
                TextButton(
                    onClick = onExclude,
                    enabled = enabled,
                    shapes = ButtonDefaults.shapes(),
                    modifier = Modifier.semantics { contentDescription = "Skip account “$rawAccount”" },
                ) { Text("Skip this account") }
            }
            Button(
                onClick = onLink,
                enabled = enabled && selectedId != null,
                shapes = ButtonDefaults.shapes(),
                modifier = Modifier.semantics { contentDescription = "Link “$rawAccount” to the selected account" },
            ) { Text("Link") }
        }
    }
}

@Composable
fun AssetQuestionCard(
    raw: String,
    options: List<QuestionOption>,
    selectedId: String?,
    canExclude: Boolean,
    enabled: Boolean,
    onSelect: (String) -> Unit,
    onSearch: () -> Unit,
    onLeaveOut: () -> Unit,
    onLink: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var confirmLeaveOut by remember { mutableStateOf(false) }
    QuestionSurface(modifier) {
        QuestionTitle(title = "Which asset is “$raw”?", supporting = null)
        OptionList(options = options, selectedId = selectedId, enabled = enabled, onSelect = onSelect)
        ExtraOptionRow(icon = "search", label = "Search assets…", enabled = enabled, onClick = onSearch)
        CardActions(Modifier.padding(horizontal = 20.dp).padding(top = 8.dp)) {
            if (canExclude) {
                TextButton(
                    onClick = { confirmLeaveOut = true },
                    enabled = enabled,
                    shapes = ButtonDefaults.shapes(),
                ) { Text("Leave out") }
            }
            Button(
                onClick = onLink,
                enabled = enabled && selectedId != null,
                shapes = ButtonDefaults.shapes(),
                modifier = Modifier.semantics { contentDescription = "Link “$raw” to the selected asset" },
            ) { Text("Link") }
        }
        if (confirmLeaveOut) {
            AlertDialog(
                onDismissRequest = { confirmLeaveOut = false },
                title = { Text("Leave out “$raw”?") },
                text = { Text("Rows that use this asset won't be imported.") },
                confirmButton = {
                    TextButton(onClick = {
                        confirmLeaveOut = false
                        onLeaveOut()
                    }) { Text("Leave out") }
                },
                dismissButton = {
                    TextButton(onClick = { confirmLeaveOut = false }) { Text("Cancel") }
                },
            )
        }
    }
}

@Composable
fun BalanceQuestionCard(
    accountName: String,
    ticker: String,
    asOfDate: Long,
    enabled: Boolean,
    onCheck: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    ReviewCard(modifier) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = listOf(accountName, ticker).filter { it.isNotBlank() }.joinToString(" · "),
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            ToneChip(label = "Not checked", tone = ImportTone.Ghost)
        }
        DetailLine(
            "No statement gives a balance to check against. Your bank or broker app shows the balance " +
                "after each transaction.",
        )
        Text("What was the balance on ${importDate(asOfDate)}?", style = MaterialTheme.typography.bodyLarge)
        BalanceEntryField(
            label = "Balance",
            suffix = ticker,
            enabled = enabled,
            onSubmit = onCheck,
        )
    }
}

@Composable
fun BalanceEntryField(
    label: String,
    suffix: String,
    enabled: Boolean,
    onSubmit: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    var text by rememberSaveable { mutableStateOf("") }
    val symbols = remember { DecimalFormatSymbols.getInstance() }
    val quantity = parseQuantity(text, symbols)
    val focusManager = LocalFocusManager.current
    val submit = {
        quantity?.let {
            focusManager.clearFocus()
            onSubmit(it)
        }
    }
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        OutlinedTextField(
            value = text,
            onValueChange = { text = it },
            label = { Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
            suffix = suffix.takeIf { it.isNotBlank() }?.let { { Text(it) } },
            singleLine = true,
            enabled = enabled,
            isError = text.isNotBlank() && quantity == null,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { submit() }),
            modifier = Modifier.weight(1f),
        )
        Button(
            onClick = { submit() },
            enabled = enabled && quantity != null,
            shapes = ButtonDefaults.shapes(),
        ) { Text("Check") }
    }
}

internal fun parseQuantity(
    raw: String,
    symbols: DecimalFormatSymbols,
): String? {
    val grouping = symbols.groupingSeparator
    val decimal = symbols.decimalSeparator
    val text =
        raw
            .trim()
            .replace(symbols.minusSign, '-')
            .let { if (Character.isSpaceChar(grouping)) it.replace(' ', grouping) else it }
    val group = Regex.escape(grouping.toString())
    val point = Regex.escape(decimal.toString())
    if (!Regex("-?(\\d+|\\d{1,3}($group\\d{3})+)($point\\d+)?").matches(text)) return null
    return text.replace(grouping.toString(), "").replace(decimal, '.')
}

@Composable
internal fun QuestionSurface(
    modifier: Modifier,
    content: @Composable () -> Unit,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(24.dp),
        color = MaterialTheme.colorScheme.surfaceBright,
    ) {
        Column(modifier = Modifier.padding(vertical = 16.dp)) { content() }
    }
}

@Composable
internal fun QuestionTitle(
    title: String,
    supporting: String?,
) {
    Column(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp)
                .padding(top = 4.dp, bottom = 8.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(text = title, style = MaterialTheme.typography.titleMedium)
        supporting?.let {
            Text(
                text = it,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.MiddleEllipsis,
            )
        }
    }
}

@Composable
internal fun OptionList(
    options: List<QuestionOption>,
    selectedId: String?,
    enabled: Boolean,
    onSelect: (String) -> Unit,
) {
    Column(Modifier.selectableGroup()) {
        options.forEach { option ->
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .selectable(
                            selected = option.id == selectedId,
                            enabled = enabled,
                            role = Role.RadioButton,
                            onClick = { onSelect(option.id) },
                        ).heightIn(min = 56.dp)
                        .padding(horizontal = 18.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                RadioButton(selected = option.id == selectedId, onClick = null, enabled = enabled)
                Column(Modifier.weight(1f)) {
                    Text(
                        text = option.title,
                        style = MaterialTheme.typography.bodyLarge,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    option.subtitle?.let {
                        Text(
                            text = it,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    option.note?.let {
                        Text(
                            text = it,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun ExtraOptionRow(
    icon: String,
    label: String,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
                .heightIn(min = 56.dp)
                .padding(horizontal = 20.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        LucideIcon(name = icon, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(20.dp))
        Text(text = label, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.primary)
    }
}

@Composable
private fun SameAccountRow(
    checked: Boolean,
    enabled: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .toggleable(value = checked, enabled = enabled, role = Role.Switch, onValueChange = onCheckedChange)
                .heightIn(min = 64.dp)
                .padding(horizontal = 20.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text("These are the same account", style = MaterialTheme.typography.bodyLarge)
            DetailLine(if (checked) "Your answer covers every file" else "Answer one file at a time")
        }
        Spacer(Modifier.size(16.dp))
        Switch(checked = checked, onCheckedChange = null, enabled = enabled)
    }
}
