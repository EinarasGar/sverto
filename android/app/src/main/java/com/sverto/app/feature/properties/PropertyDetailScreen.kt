package com.sverto.app.feature.properties

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.LinkOff
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MediumFlexibleTopAppBar
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SplitButtonDefaults
import androidx.compose.material3.SplitButtonLayout
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.PullToRefreshDefaults
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sverto.app.core.Money
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.core.ui.PortfolioChartSkeleton
import com.sverto.app.core.ui.RowDivider
import com.sverto.app.core.ui.rememberHapticPullToRefreshState
import com.sverto.app.feature.accounts.components.MetricItem
import com.sverto.app.feature.accounts.components.MetricsGrid
import com.sverto.app.feature.accounts.components.accountTypeContainerColor
import com.sverto.app.feature.accounts.components.accountTypeIcon
import com.sverto.app.feature.accounts.components.accountTypeLabel
import com.sverto.app.feature.accounts.components.accountTypeOnContainerColor
import com.sverto.app.feature.portfolio.PortfolioChart
import com.sverto.app.feature.portfolio.TimePeriod
import uniffi.sverto_core.AccountListItem
import uniffi.sverto_core.PropertyDetail
import uniffi.sverto_core.PropertyKind
import kotlin.math.roundToInt

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PropertyDetailScreen(
    propertyId: String,
    assetId: Int,
    showBack: Boolean,
    onBack: () -> Unit,
    onViewLoan: (AccountListItem) -> Unit,
    onOpenValuations: (assetId: Int, currencyAssetId: Int) -> Unit,
    modifier: Modifier = Modifier,
    viewModel: PropertyDetailViewModel = viewModel(factory = SvertoViewModelFactory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    // Reloads on return from a loan's account page, where the loan may have been renamed.
    LifecycleResumeEffect(propertyId) {
        viewModel.load(propertyId, assetId)
        onPauseOrDispose { }
    }

    val snackbarHostState = remember { SnackbarHostState() }
    val haptics = LocalHapticFeedback.current
    LaunchedEffect(Unit) {
        viewModel.saved.collect { haptics.performHapticFeedback(HapticFeedbackType.Confirm) }
    }
    LaunchedEffect(Unit) {
        viewModel.errors.collect {
            haptics.performHapticFeedback(HapticFeedbackType.Reject)
            snackbarHostState.showSnackbar(it)
        }
    }

    var showAddValuation by remember { mutableStateOf(false) }
    var showEdit by remember { mutableStateOf(false) }
    var showLinkLoan by remember { mutableStateOf(false) }
    var unlinkTarget by remember { mutableStateOf<AccountListItem?>(null) }

    val detail = state.detail
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    val pullToRefreshState = rememberHapticPullToRefreshState()

    Scaffold(
        modifier = modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        snackbarHost = { SnackbarHost(snackbarHostState) },
        topBar = {
            MediumFlexibleTopAppBar(
                title = { Text(detail?.account?.name.orEmpty()) },
                navigationIcon = {
                    if (showBack) {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                        }
                    }
                },
                actions = {
                    if (detail != null) {
                        IconButton(onClick = {
                            viewModel.startEdit()
                            showEdit = true
                        }) {
                            Icon(Icons.Outlined.Edit, contentDescription = "Edit property")
                        }
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
    ) { padding ->
        when {
            detail == null && state.isLoading ->
                Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                    LoadingIndicator()
                }

            detail == null ->
                Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                    Text(
                        "Couldn't load this property",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }

            else ->
                PullToRefreshBox(
                    isRefreshing = state.isRefreshing,
                    onRefresh = viewModel::refresh,
                    state = pullToRefreshState,
                    indicator = {
                        PullToRefreshDefaults.LoadingIndicator(
                            state = pullToRefreshState,
                            isRefreshing = state.isRefreshing,
                            modifier = Modifier.align(Alignment.TopCenter),
                        )
                    },
                    modifier = Modifier.fillMaxSize().padding(padding),
                ) {
                    Column(
                        modifier =
                            Modifier
                                .fillMaxSize()
                                .verticalScroll(rememberScrollState())
                                .padding(horizontal = 16.dp),
                        verticalArrangement = Arrangement.spacedBy(24.dp),
                    ) {
                        PropertyHeader(detail)

                        ValueSection(
                            state = state,
                            baseTicker = detail.baseTicker,
                            onRetryChart = viewModel::retryChart,
                            onAddValuation = { showAddValuation = true },
                            onOpenValuations = { onOpenValuations(detail.summary.assetId, detail.summary.currencyAssetId) },
                        )

                        PropertyFigures(detail)

                        LoansSection(
                            loans = detail.loans,
                            baseTicker = detail.baseTicker,
                            onView = onViewLoan,
                            onUnlink = { unlinkTarget = it },
                            onLink = {
                                viewModel.loadLoanCandidates()
                                showLinkLoan = true
                            },
                        )

                        Spacer(Modifier.height(8.dp))
                    }
                }
        }
    }

    if (showAddValuation) {
        ValuationSheet(
            title = "Add valuation",
            currencyTicker = state.currencyTicker,
            onSubmit = { date, value ->
                viewModel.addValuation(date, value)
                showAddValuation = false
            },
            onDismiss = { showAddValuation = false },
        )
    }
    if (showEdit && detail != null) {
        EditPropertySheet(
            name = detail.account.name,
            ownershipShare = detail.account.ownershipShare,
            purchaseDate = state.purchaseDate,
            error = state.editError,
            isSaving = state.isSavingEdit,
            onRetryPurchaseDate = viewModel::loadPurchaseDate,
            onSubmit = { name, share, purchaseDate ->
                viewModel.saveEdit(name, share, purchaseDate) {
                    haptics.performHapticFeedback(HapticFeedbackType.Confirm)
                    showEdit = false
                }
            },
            onDismiss = { showEdit = false },
        )
    }
    val loanToUnlink = unlinkTarget
    if (loanToUnlink != null && detail != null) {
        AlertDialog(
            onDismissRequest = { unlinkTarget = null },
            title = { Text("Unlink loan?") },
            text = {
                Text(
                    "Are you sure you want to unlink ${loanToUnlink.name} from ${detail.account.name}? " +
                        "The loan stays a normal account and shows in your accounts list again.",
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    viewModel.unlinkLoan(loanToUnlink.id)
                    unlinkTarget = null
                }) {
                    Text("Unlink", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { unlinkTarget = null }) { Text("Cancel") }
            },
        )
    }
    if (showLinkLoan && detail != null) {
        LinkLoanSheet(
            candidates = state.loanCandidates,
            onSelect = { loan ->
                viewModel.linkLoan(loan.id)
                showLinkLoan = false
            },
            onRetry = viewModel::loadLoanCandidates,
            onDismiss = { showLinkLoan = false },
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ValueSection(
    state: PropertyDetailUiState,
    baseTicker: String,
    onRetryChart: () -> Unit,
    onAddValuation: () -> Unit,
    onOpenValuations: () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        val trend = state.trend
        when {
            state.chartFailed -> RetryMessage("Couldn't load the value history.", onRetry = onRetryChart)

            trend != null ->
                PortfolioChart(
                    portfolioData = mapOf(TimePeriod.ALL to trend),
                    currencyTicker = baseTicker,
                    selectedPeriod = TimePeriod.ALL,
                    showPeriodSelector = false,
                    changeSuffix = " since purchase",
                    markers = state.valuations,
                )

            else -> PortfolioChartSkeleton()
        }
        SplitButtonLayout(
            leadingButton = {
                SplitButtonDefaults.LeadingButton(
                    onClick = onAddValuation,
                    colors = ButtonDefaults.filledTonalButtonColors(),
                ) {
                    Icon(
                        Icons.Filled.Add,
                        contentDescription = null,
                        modifier = Modifier.size(SplitButtonDefaults.LeadingIconSize),
                    )
                    Spacer(Modifier.width(8.dp))
                    Text("Add valuation")
                }
            },
            trailingButton = {
                SplitButtonDefaults.TrailingButton(
                    onClick = onOpenValuations,
                    colors = ButtonDefaults.filledTonalButtonColors(),
                ) {
                    Icon(
                        Icons.Outlined.History,
                        contentDescription = "Valuations",
                        modifier = Modifier.size(SplitButtonDefaults.TrailingIconSize),
                    )
                }
            },
            modifier = Modifier.align(Alignment.CenterHorizontally),
        )
    }
}

@Composable
private fun PropertyHeader(detail: PropertyDetail) {
    val typeId = detail.account.accountTypeId
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(
            modifier =
                Modifier
                    .size(40.dp)
                    .background(accountTypeContainerColor(typeId), CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                imageVector = accountTypeIcon(typeId),
                contentDescription = null,
                tint = accountTypeOnContainerColor(typeId),
                modifier = Modifier.size(22.dp),
            )
        }
        Spacer(Modifier.width(12.dp))
        Text(
            text =
                when (detail.summary.kind) {
                    PropertyKind.REAL_ESTATE -> "Real estate"
                    PropertyKind.VEHICLE -> "Vehicle"
                },
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        val share = detail.account.ownershipShare
        if (share < 1.0) {
            Spacer(Modifier.width(12.dp))
            Surface(
                shape = CircleShape,
                color = MaterialTheme.colorScheme.secondaryContainer,
            ) {
                Text(
                    text = "Joint · ${(share * 100).roundToInt()}%",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSecondaryContainer,
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                )
            }
        }
    }
}

@Composable
private fun PropertyFigures(detail: PropertyDetail) {
    val summary = detail.summary
    val ticker = detail.baseTicker

    fun amount(figure: Double?) = figure?.let { Money.format(it, ticker) } ?: "—"

    val items =
        buildList {
            add(MetricItem(label = "Value", value = amount(summary.value)))
            add(MetricItem(label = "Loan left", value = amount(summary.loanBalance)))
            add(MetricItem(label = "Equity", value = amount(summary.equity)))
            add(
                MetricItem(
                    label = "Loan-to-value",
                    value = summary.loanToValue?.let { "%.1f%%".format(it * 100) } ?: "—",
                ),
            )
            if (detail.account.ownershipShare < 1.0) {
                add(
                    MetricItem(
                        label = "Your share",
                        value = amount(summary.equityShare),
                        valueColor = MaterialTheme.colorScheme.primary,
                    ),
                )
            }
        }

    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(
            text = "Summary",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        MetricsGrid(items = items)
    }
}

@Composable
private fun LoansSection(
    loans: List<AccountListItem>,
    baseTicker: String,
    onView: (AccountListItem) -> Unit,
    onUnlink: (AccountListItem) -> Unit,
    onLink: () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(
            text = "Loans",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        Surface(
            shape = RoundedCornerShape(20.dp),
            color = MaterialTheme.colorScheme.surfaceBright,
        ) {
            Column {
                if (loans.isEmpty()) {
                    Text(
                        text = "No loans linked",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(16.dp),
                    )
                    RowDivider()
                }
                loans.forEach { loan ->
                    LoanRow(
                        loan = loan,
                        baseTicker = baseTicker,
                        onView = { onView(loan) },
                        onUnlink = { onUnlink(loan) },
                    )
                    RowDivider()
                }
                TextButton(onClick = onLink, modifier = Modifier.fillMaxWidth()) {
                    Icon(Icons.Filled.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("Link loan")
                }
            }
        }
    }
}

@Composable
private fun LoanRow(
    loan: AccountListItem,
    baseTicker: String,
    onView: () -> Unit,
    onUnlink: () -> Unit,
) {
    ListItem(
        modifier = Modifier.clickable(onClick = onView),
        leadingContent = {
            Box(
                modifier =
                    Modifier
                        .size(40.dp)
                        .background(accountTypeContainerColor(loan.accountTypeId), CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = accountTypeIcon(loan.accountTypeId),
                    contentDescription = null,
                    tint = accountTypeOnContainerColor(loan.accountTypeId),
                    modifier = Modifier.size(20.dp),
                )
            }
        },
        headlineContent = { Text(loan.name) },
        supportingContent = {
            val share = loan.ownershipShare
            val type = accountTypeLabel(loan.accountTypeId)
            Text(if (share < 1.0) "$type · ${(share * 100).roundToInt()}%" else type)
        },
        trailingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                loan.balance?.let {
                    Text(
                        text = Money.format(it, baseTicker),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                }
                IconButton(onClick = onUnlink) {
                    Icon(Icons.Outlined.LinkOff, contentDescription = "Unlink ${loan.name}")
                }
            }
        },
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceBright),
    )
}
