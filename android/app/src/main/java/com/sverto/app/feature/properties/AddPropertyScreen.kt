package com.sverto.app.feature.properties

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.outlined.DirectionsCar
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MediumFlexibleTopAppBar
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.feature.accounts.FieldSection
import com.sverto.app.feature.accounts.OwnershipSection
import com.sverto.app.feature.accounts.SaveBar
import com.sverto.app.feature.accounts.SubmitState
import com.sverto.app.feature.assets.components.CurrencyPickerSheet
import com.sverto.app.feature.assets.components.LabeledPickerField
import uniffi.sverto_core.PropertyKind

@Suppress("LongMethod")
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun AddPropertyScreen(
    onBack: () -> Unit,
    onSuccess: () -> Unit,
    modifier: Modifier = Modifier,
    viewModel: AddPropertyViewModel = viewModel(factory = SvertoViewModelFactory),
) {
    val kind by viewModel.kind.collectAsStateWithLifecycle()
    val name by viewModel.name.collectAsStateWithLifecycle()
    val currencyAssetId by viewModel.currencyAssetId.collectAsStateWithLifecycle()
    val ticker by viewModel.currencyTicker.collectAsStateWithLifecycle()
    val valueText by viewModel.value.collectAsStateWithLifecycle()
    val ownershipShare by viewModel.ownershipShare.collectAsStateWithLifecycle()
    val purchaseDateMillis by viewModel.purchaseDateMillis.collectAsStateWithLifecycle()
    val boughtBeforeToday by viewModel.boughtBeforeToday.collectAsStateWithLifecycle()
    val purchasePrice by viewModel.purchasePrice.collectAsStateWithLifecycle()
    val loanAccountIds by viewModel.loanAccountIds.collectAsStateWithLifecycle()
    val loanCandidates by viewModel.loanCandidates.collectAsStateWithLifecycle()
    val submitState by viewModel.submitState.collectAsStateWithLifecycle()

    val snackbarHostState = remember { SnackbarHostState() }
    LaunchedEffect(submitState) {
        val error = submitState as? SubmitState.Error ?: return@LaunchedEffect
        snackbarHostState.showSnackbar(error.message)
        viewModel.clearError()
    }

    var showCurrencyPicker by remember { mutableStateOf(false) }
    val isLoading = submitState is SubmitState.Loading

    val price = purchasePrice.trim().toDoubleOrNull()
    val priceError = purchasePrice.isNotBlank() && (price == null || price <= 0.0)
    val value = valueText.trim().toDoubleOrNull()
    val valueError = valueText.isNotBlank() && (value == null || value <= 0.0)
    val isValid =
        name.isNotBlank() &&
            currencyAssetId != null &&
            price != null &&
            !priceError &&
            (!boughtBeforeToday || (value != null && !valueError))

    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    val motionScheme = MaterialTheme.motionScheme

    Scaffold(
        topBar = {
            MediumFlexibleTopAppBar(
                title = { Text("Add property") },
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
        snackbarHost = { SnackbarHost(snackbarHostState) },
        bottomBar = {
            SaveBar(
                label = "Save property",
                isLoading = isLoading,
                enabled = isValid && !isLoading,
                onSave = { viewModel.save(onSuccess) },
            )
        },
        modifier = modifier.fillMaxSize().nestedScroll(scrollBehavior.nestedScrollConnection),
    ) { innerPadding ->
        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(innerPadding)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(28.dp),
        ) {
            Spacer(Modifier.height(8.dp))

            KindSelector(
                kind = kind,
                enabled = !isLoading,
                onSelect = { viewModel.kind.value = it },
            )

            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { viewModel.name.value = it },
                    label = { Text("Name") },
                    singleLine = true,
                    shape = RoundedCornerShape(16.dp),
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words),
                    enabled = !isLoading,
                    modifier = Modifier.fillMaxWidth(),
                )
                LabeledPickerField(
                    label = "Currency",
                    value = ticker.ifEmpty { "Choose a currency" },
                    onClick = { if (!isLoading) showCurrencyPicker = true },
                )
                PastDateField(
                    label = "Bought on",
                    dateMillis = purchaseDateMillis,
                    onSelect = { viewModel.purchaseDateMillis.value = it },
                )
                AmountField(
                    value = purchasePrice,
                    onValueChange = { viewModel.purchasePrice.value = it },
                    label = "Purchase price",
                    ticker = ticker,
                    isError = priceError,
                    enabled = !isLoading,
                )
                AnimatedVisibility(
                    visible = boughtBeforeToday,
                    enter = fadeIn(motionScheme.defaultEffectsSpec()) + expandVertically(motionScheme.defaultSpatialSpec()),
                    exit = fadeOut(motionScheme.fastEffectsSpec()) + shrinkVertically(motionScheme.fastSpatialSpec()),
                ) {
                    AmountField(
                        value = valueText,
                        onValueChange = { viewModel.value.value = it },
                        label = "Value today",
                        ticker = ticker,
                        isError = valueError,
                        enabled = !isLoading,
                    )
                }
            }

            OwnershipSection(
                ownershipShare = ownershipShare,
                enabled = !isLoading,
                onChange = { viewModel.ownershipShare.value = it },
            )

            FieldSection(title = "Loans (optional)") {
                LoanChips(
                    candidates = loanCandidates,
                    selectedIds = loanAccountIds,
                    enabled = !isLoading,
                    onToggle = viewModel::toggleLoan,
                    onRetry = viewModel::loadLoanCandidates,
                )
            }

            Spacer(Modifier.height(8.dp))
        }
    }

    if (showCurrencyPicker) {
        CurrencyPickerSheet(
            title = "Currency",
            selectedId = currencyAssetId,
            onSelect = {
                viewModel.selectCurrency(it)
                showCurrencyPicker = false
            },
            onDismiss = { showCurrencyPicker = false },
        )
    }
}

@Composable
private fun AmountField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    ticker: String,
    isError: Boolean,
    enabled: Boolean,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        suffix = { Text(ticker) },
        singleLine = true,
        isError = isError,
        supportingText = if (isError) ({ Text("Enter a positive number") }) else null,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
        shape = RoundedCornerShape(16.dp),
        enabled = enabled,
        modifier = Modifier.fillMaxWidth(),
    )
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun KindSelector(
    kind: PropertyKind,
    enabled: Boolean,
    onSelect: (PropertyKind) -> Unit,
) {
    FieldSection(title = "Kind") {
        Row(
            horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween),
            modifier = Modifier.fillMaxWidth(),
        ) {
            val colors =
                ToggleButtonDefaults.toggleButtonColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                )
            listOf(
                Triple(PropertyKind.REAL_ESTATE, "Real estate", Icons.Outlined.Home),
                Triple(PropertyKind.VEHICLE, "Vehicle", Icons.Outlined.DirectionsCar),
            ).forEachIndexed { index, (option, label, icon) ->
                ToggleButton(
                    checked = kind == option,
                    onCheckedChange = { onSelect(option) },
                    enabled = enabled,
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
                ) {
                    Icon(icon, contentDescription = null, modifier = Modifier.size(ToggleButtonDefaults.IconSize))
                    Spacer(Modifier.width(ToggleButtonDefaults.IconSpacing))
                    Text(label, maxLines = 1)
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun LoanChips(
    candidates: LoanCandidates,
    selectedIds: Set<String>,
    enabled: Boolean,
    onToggle: (String) -> Unit,
    onRetry: () -> Unit,
) {
    val accounts =
        when (candidates) {
            LoanCandidates.Loading -> {
                LoadingIndicator()
                return
            }

            LoanCandidates.Failed -> {
                RetryMessage("Couldn't load your loan accounts.", onRetry = onRetry)
                return
            }

            is LoanCandidates.Loaded -> candidates.accounts
        }
    if (accounts.isEmpty()) {
        Text(
            text = "No unlinked mortgage or loan accounts.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        return
    }
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        accounts.forEach { account ->
            val selected = account.id in selectedIds
            FilterChip(
                selected = selected,
                onClick = { onToggle(account.id) },
                label = { Text(account.name) },
                enabled = enabled,
                leadingIcon =
                    if (selected) {
                        { Icon(Icons.Filled.Check, contentDescription = null, modifier = Modifier.size(FilterChipDefaults.IconSize)) }
                    } else {
                        null
                    },
            )
        }
    }
}
