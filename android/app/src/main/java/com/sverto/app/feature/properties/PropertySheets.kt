package com.sverto.app.feature.properties

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SelectableDates
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.sverto.app.feature.accounts.OwnershipSection
import com.sverto.app.feature.accounts.components.accountTypeLabel
import com.sverto.app.feature.assets.components.LabeledPickerField
import com.sverto.app.feature.portfolio.ChartPoint
import uniffi.sverto_core.AccountListItem
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.util.Locale

private val dateFormatter = DateTimeFormatter.ofPattern("MMM d, yyyy", Locale.US)

fun todayUtcMillis(): Long =
    LocalDate
        .now()
        .atStartOfDay(ZoneOffset.UTC)
        .toInstant()
        .toEpochMilli()

// Today becomes now: east of UTC, today's UTC midnight can still be in the future (the server
// rejects a future purchase date), and a value dated at midnight would sit behind the rate the
// property got when it was created.
fun pickedDateSeconds(dateMillis: Long): Long = if (dateMillis == todayUtcMillis()) System.currentTimeMillis() / 1000 else dateMillis / 1000

@Composable
fun RetryMessage(
    message: String,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(modifier = modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(
            text = message,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error,
            modifier = Modifier.weight(1f),
        )
        TextButton(onClick = onRetry) { Text("Retry") }
    }
}

private fun localDateUtcMillis(epochSeconds: Long): Long =
    Instant
        .ofEpochSecond(epochSeconds)
        .atZone(ZoneId.systemDefault())
        .toLocalDate()
        .atStartOfDay(ZoneOffset.UTC)
        .toInstant()
        .toEpochMilli()

/** [dateMillis] is the UTC midnight a Material date picker selects. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PastDateField(
    label: String,
    dateMillis: Long?,
    onSelect: (Long) -> Unit,
    modifier: Modifier = Modifier,
) {
    var showDialog by remember { mutableStateOf(false) }
    LabeledPickerField(
        label = label,
        value =
            when (dateMillis) {
                null -> "Not set"
                todayUtcMillis() -> "Today"
                else ->
                    Instant
                        .ofEpochMilli(dateMillis)
                        .atZone(ZoneOffset.UTC)
                        .toLocalDate()
                        .format(dateFormatter)
            },
        onClick = { showDialog = true },
        modifier = modifier,
    )
    if (showDialog) {
        val today = remember { todayUtcMillis() }
        val state =
            rememberDatePickerState(
                initialSelectedDateMillis = dateMillis ?: today,
                selectableDates =
                    object : SelectableDates {
                        override fun isSelectableDate(utcTimeMillis: Long) = utcTimeMillis <= today

                        override fun isSelectableYear(year: Int) = year <= LocalDate.now().year
                    },
            )
        DatePickerDialog(
            onDismissRequest = { showDialog = false },
            confirmButton = {
                TextButton(onClick = {
                    state.selectedDateMillis?.let(onSelect)
                    showDialog = false
                }) { Text("OK") }
            },
            dismissButton = {
                TextButton(onClick = { showDialog = false }) { Text("Cancel") }
            },
        ) {
            DatePicker(state = state)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ValuationSheet(
    title: String,
    currencyTicker: String,
    onSubmit: (date: Long, value: Double) -> Unit,
    onDismiss: () -> Unit,
    valuation: ChartPoint? = null,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val initialDateMillis = remember { valuation?.let { localDateUtcMillis(it.date) } ?: todayUtcMillis() }
    var valueText by remember {
        mutableStateOf(
            valuation
                ?.value
                ?.toBigDecimal()
                ?.stripTrailingZeros()
                ?.toPlainString()
                .orEmpty(),
        )
    }
    var dateMillis by remember { mutableLongStateOf(initialDateMillis) }
    val value = valueText.trim().toDoubleOrNull()
    val canSave = value != null && value > 0.0

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(Modifier.fillMaxWidth().padding(24.dp)) {
            Text(title, style = MaterialTheme.typography.headlineSmall)
            Spacer(Modifier.height(16.dp))
            OutlinedTextField(
                value = valueText,
                onValueChange = { valueText = it },
                label = { Text("Value") },
                suffix = { Text(currencyTicker) },
                singleLine = true,
                isError = valueText.isNotBlank() && !canSave,
                supportingText = {
                    if (valueText.isNotBlank() && !canSave) Text("Enter a positive number")
                },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                shape = RoundedCornerShape(16.dp),
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(4.dp))
            PastDateField(
                label = "Date",
                dateMillis = dateMillis,
                onSelect = { dateMillis = it },
            )
            Spacer(Modifier.height(24.dp))
            Row(Modifier.fillMaxWidth()) {
                TextButton(onClick = onDismiss) { Text("Cancel") }
                Spacer(Modifier.weight(1f))
                Button(
                    enabled = canSave,
                    onClick = {
                        // The picker only knows days, so an untouched date keeps the valuation's exact time.
                        val date =
                            if (valuation != null && dateMillis == initialDateMillis) valuation.date else pickedDateSeconds(dateMillis)
                        value?.let { onSubmit(date, it) }
                    },
                ) { Text("Save") }
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EditPropertySheet(
    name: String,
    ownershipShare: Double,
    purchaseDate: PurchaseDate,
    error: String?,
    isSaving: Boolean,
    onRetryPurchaseDate: () -> Unit,
    onSubmit: (name: String, ownershipShare: Double, purchaseDate: Long?) -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var nameText by remember { mutableStateOf(name) }
    var share by remember { mutableFloatStateOf(ownershipShare.toFloat()) }
    val loadedDateMillis = (purchaseDate as? PurchaseDate.Loaded)?.seconds?.let(::localDateUtcMillis)
    var dateMillis by remember(loadedDateMillis) { mutableStateOf(loadedDateMillis) }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(Modifier.fillMaxWidth().padding(24.dp)) {
            Text("Edit property", style = MaterialTheme.typography.headlineSmall)
            Spacer(Modifier.height(16.dp))
            OutlinedTextField(
                value = nameText,
                onValueChange = { nameText = it },
                label = { Text("Name") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
                shape = RoundedCornerShape(16.dp),
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(16.dp))
            OwnershipSection(
                ownershipShare = share,
                enabled = !isSaving,
                onChange = { share = it },
            )
            Spacer(Modifier.height(16.dp))
            val selectedDate = dateMillis
            when {
                selectedDate != null ->
                    PastDateField(
                        label = "Bought on",
                        dateMillis = selectedDate,
                        onSelect = { dateMillis = it },
                    )

                purchaseDate == PurchaseDate.Failed ->
                    PurchaseDateStatus("Couldn't load the purchase date", onRetry = onRetryPurchaseDate)

                else -> PurchaseDateStatus("Loading…", onRetry = null)
            }
            error?.let {
                Spacer(Modifier.height(8.dp))
                Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
            }
            Spacer(Modifier.height(24.dp))
            Row(Modifier.fillMaxWidth()) {
                TextButton(onClick = onDismiss) { Text("Cancel") }
                Spacer(Modifier.weight(1f))
                Button(
                    enabled = nameText.isNotBlank() && !isSaving,
                    onClick = {
                        val movedDate = selectedDate?.takeIf { it != loadedDateMillis }?.let(::pickedDateSeconds)
                        onSubmit(nameText.trim(), share.toDouble(), movedDate)
                    },
                ) { Text("Save") }
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun PurchaseDateStatus(
    message: String,
    onRetry: (() -> Unit)?,
) {
    Surface(
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier = Modifier.padding(start = 16.dp, top = 8.dp, end = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f).padding(vertical = 8.dp)) {
                Text(
                    "Bought on",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(message, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            onRetry?.let { TextButton(onClick = it) { Text("Retry") } }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LinkLoanSheet(
    candidates: LoanCandidates,
    onSelect: (AccountListItem) -> Unit,
    onRetry: () -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp)) {
            Text(
                "Link loan",
                style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.padding(start = 12.dp, bottom = 8.dp),
            )
            when (candidates) {
                LoanCandidates.Loading ->
                    Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) {
                        LoadingIndicator()
                    }

                LoanCandidates.Failed ->
                    RetryMessage("Couldn't load your loan accounts.", onRetry = onRetry, modifier = Modifier.padding(12.dp))

                is LoanCandidates.Loaded ->
                    if (candidates.accounts.isEmpty()) {
                        Text(
                            "Every mortgage and loan account is already linked. Add one under Accounts first.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(12.dp),
                        )
                    } else {
                        LazyColumn {
                            items(candidates.accounts, key = { it.id }) { account ->
                                ListItem(
                                    headlineContent = { Text(account.name) },
                                    supportingContent = { Text(accountTypeLabel(account.accountTypeId)) },
                                    colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
                                    modifier = Modifier.fillMaxWidth().clickable { onSelect(account) },
                                )
                            }
                        }
                    }
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}
