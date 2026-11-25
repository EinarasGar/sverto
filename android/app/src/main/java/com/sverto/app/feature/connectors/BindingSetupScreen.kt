package com.sverto.app.feature.connectors

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeFlexibleTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.feature.imports.review.importDate
import uniffi.sverto_core.ConnectorImportFrom
import uniffi.sverto_core.ProviderAccount
import uniffi.sverto_core.formatMoney

@Suppress("LongMethod")
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun BindingSetupScreen(
    connectionId: String,
    onBack: () -> Unit,
    onDone: () -> Unit,
    onOpenAccount: (ProviderAccount) -> Unit,
    onImported: (sessionId: String) -> Unit,
    onOpenImport: (sessionId: String) -> Unit,
    modifier: Modifier = Modifier,
    viewModel: BindingSetupViewModel = viewModel(factory = SvertoViewModelFactory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    LifecycleResumeEffect(connectionId) {
        viewModel.load(connectionId)
        onPauseOrDispose { }
    }
    LaunchedEffect(state.importSessionId) {
        state.importSessionId?.let(onImported)
    }
    var pickingFor by remember { mutableStateOf<ProviderAccount?>(null) }
    val verb = if (state.linksByImport) "Import" else "Link"

    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()

    Scaffold(
        modifier =
            modifier
                .fillMaxSize()
                .nestedScroll(scrollBehavior.nestedScrollConnection),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        topBar = {
            LargeFlexibleTopAppBar(
                title = { Text("Link accounts") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors =
                    TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.surfaceContainer,
                        scrolledContainerColor = MaterialTheme.colorScheme.surfaceContainer,
                    ),
                scrollBehavior = scrollBehavior,
            )
        },
        bottomBar = {
            HeroButton(
                text =
                    when (state.choices.size) {
                        0 -> "Done"
                        1 -> "$verb 1 account"
                        else -> "$verb ${state.choices.size} accounts"
                    },
                onClick = { if (state.choices.isEmpty()) onDone() else viewModel.linkAccounts(connectionId) },
                enabled = !state.linking,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp, vertical = 12.dp)
                        .navigationBarsPadding(),
            )
        },
    ) { innerPadding ->
        Box(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(innerPadding),
        ) {
            Column(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 16.dp),
            ) {
                Text(
                    text =
                        "Choose which Sverto account each imported account writes into. " +
                            "You can skip any and add them later.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                state.error?.let {
                    Spacer(Modifier.height(16.dp))
                    Text(
                        it,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
                Spacer(Modifier.height(24.dp))
                Text(
                    "Accounts",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                )
                Spacer(Modifier.height(8.dp))
                if (state.providerAccounts.isEmpty() && state.loading) {
                    BindingRowsSkeleton()
                } else {
                    Surface(
                        shape = RoundedCornerShape(20.dp),
                        color = MaterialTheme.colorScheme.surfaceBright,
                    ) {
                        Column {
                            state.providerAccounts.forEachIndexed { index, account ->
                                val binding = state.bindings[account.providerAccountId]
                                val pendingImport = binding?.takeIf { it.status == "pending" }
                                val choice = state.choices[account.providerAccountId]
                                Row(
                                    modifier =
                                        Modifier
                                            .fillMaxWidth()
                                            .clickable { onOpenAccount(account) }
                                            .heightIn(min = 56.dp)
                                            .padding(horizontal = 16.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                                ) {
                                    Column(modifier = Modifier.weight(1f)) {
                                        Text(
                                            humanizeAccountName(account.displayName),
                                            style = MaterialTheme.typography.bodyLarge,
                                        )
                                        val details =
                                            listOfNotNull(
                                                account.currency,
                                                account.accountNumberLast4?.let { "••$it" },
                                                account.balance?.let { formatMoney(it.amount, it.currency, false) },
                                            )
                                        if (details.isNotEmpty()) {
                                            Text(
                                                text = details.joinToString(" · "),
                                                style = MaterialTheme.typography.bodyMedium,
                                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                            )
                                        }
                                        Text(
                                            text =
                                                listOfNotNull(
                                                    when {
                                                        pendingImport != null -> "Being imported into ${pendingImport.svertoAccountName}"
                                                        binding != null -> "Linked to ${binding.svertoAccountName}"
                                                        choice != null -> "→ ${choice.name}"
                                                        else -> "Not linked"
                                                    },
                                                    historyLabel(account.history),
                                                ).joinToString(" · "),
                                            style = MaterialTheme.typography.bodyMedium,
                                            color =
                                                if (choice != null) {
                                                    MaterialTheme.colorScheme.primary
                                                } else {
                                                    MaterialTheme.colorScheme.onSurfaceVariant
                                                },
                                        )
                                    }
                                    if (pendingImport != null) {
                                        pendingImport.importSessionId?.let { sessionId ->
                                            TextButton(onClick = { onOpenImport(sessionId) }) { Text("Open the import") }
                                        }
                                    } else if (binding != null) {
                                        Icon(
                                            Icons.Outlined.CheckCircle,
                                            contentDescription = "Linked",
                                            tint = MaterialTheme.colorScheme.primary,
                                        )
                                    } else if (choice != null) {
                                        IconButton(
                                            onClick = { viewModel.clearChoice(account.providerAccountId) },
                                            enabled = !state.linking,
                                        ) {
                                            Icon(
                                                Icons.Filled.Close,
                                                contentDescription = "Don't link ${humanizeAccountName(account.displayName)}",
                                            )
                                        }
                                    } else {
                                        FilledTonalButton(
                                            onClick = { pickingFor = account },
                                            contentPadding = PaddingValues(horizontal = 16.dp),
                                            modifier = Modifier.heightIn(min = 40.dp),
                                        ) {
                                            Text("Link")
                                        }
                                    }
                                }
                                if (index < state.providerAccounts.lastIndex) HorizontalDivider()
                            }
                            if (state.providerAccounts.isEmpty()) {
                                Text(
                                    "No provider accounts available.",
                                    modifier = Modifier.padding(16.dp),
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                }
                if (state.providerAccounts.any { it.providerAccountId !in state.bindings }) {
                    Spacer(Modifier.height(24.dp))
                    ImportFromChoice(
                        importFrom = state.importFrom,
                        enabled = !state.linking,
                        onChange = viewModel::setImportFrom,
                    )
                }
                Spacer(Modifier.height(16.dp))
            }
        }
    }

    pickingFor?.let { target ->
        AccountPickerSheet(
            accounts = state.svertoAccounts,
            onSelect = { sverto ->
                viewModel.choose(target.providerAccountId, sverto)
                pickingFor = null
            },
            onDismiss = { pickingFor = null },
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ImportFromChoice(
    importFrom: ConnectorImportFrom,
    enabled: Boolean,
    onChange: (ConnectorImportFrom) -> Unit,
    modifier: Modifier = Modifier,
) {
    var pickingDate by remember { mutableStateOf(false) }
    Column(modifier = modifier) {
        Text(
            "Import transactions from",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(8.dp))
        Row(
            horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween),
            modifier = Modifier.fillMaxWidth(),
        ) {
            val options =
                listOf(
                    ConnectorImportFrom.AfterLastEntry to "My last entry",
                    ConnectorImportFrom.All to "All available",
                    null to "A date",
                )
            val colors = ToggleButtonDefaults.toggleButtonColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh)
            val haptics = LocalHapticFeedback.current
            options.forEachIndexed { index, (option, label) ->
                ToggleButton(
                    checked = if (option == null) importFrom is ConnectorImportFrom.Date else importFrom == option,
                    onCheckedChange = {
                        if (option == null) {
                            pickingDate = true
                        } else {
                            if (importFrom != option) haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
                            onChange(option)
                        }
                    },
                    enabled = enabled,
                    modifier =
                        Modifier
                            .weight(1f)
                            .semantics { role = Role.RadioButton },
                    shapes =
                        when (index) {
                            0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                            options.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                            else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                        },
                    colors = colors,
                    contentPadding = PaddingValues(horizontal = 8.dp),
                ) { Text(label, maxLines = 1) }
            }
        }
        Spacer(Modifier.height(8.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text =
                    when (importFrom) {
                        ConnectorImportFrom.AfterLastEntry ->
                            "Starts on the day of the newest transaction already in each Sverto account. " +
                                "Accounts with none get all available history."
                        ConnectorImportFrom.All -> "All the history available for each account."
                        is ConnectorImportFrom.Date -> "Starts on ${importDate(importFrom.date)}."
                    },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            if (importFrom is ConnectorImportFrom.Date) {
                TextButton(onClick = { pickingDate = true }, enabled = enabled) { Text("Change date") }
            }
        }
    }

    if (pickingDate) {
        val state =
            rememberDatePickerState(initialSelectedDateMillis = (importFrom as? ConnectorImportFrom.Date)?.date?.times(1000))
        DatePickerDialog(
            onDismissRequest = { pickingDate = false },
            confirmButton = {
                TextButton(onClick = {
                    state.selectedDateMillis?.let { onChange(ConnectorImportFrom.Date(it / 1000)) }
                    pickingDate = false
                }) { Text("OK") }
            },
            dismissButton = {
                TextButton(onClick = { pickingDate = false }) { Text("Cancel") }
            },
        ) {
            DatePicker(state = state)
        }
    }
}
