package com.sverto.app.feature.portfolio

import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
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
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.PullToRefreshDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sverto.app.core.Money
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.core.ui.RowDivider
import com.sverto.app.core.ui.rememberHapticPullToRefreshState
import com.sverto.app.feature.accounts.components.accountTypeContainerColor
import com.sverto.app.feature.accounts.components.accountTypeIcon
import com.sverto.app.feature.accounts.components.accountTypeLabel
import com.sverto.app.feature.accounts.components.accountTypeOnContainerColor
import com.sverto.app.feature.assets.formatRate
import com.sverto.app.feature.transactions.TransactionAmount
import com.sverto.app.feature.transactions.TransactionGlyph
import uniffi.sverto_core.CurrencyAccount
import uniffi.sverto_core.CurrencyDetail
import uniffi.sverto_core.TransactionListItem
import uniffi.sverto_core.TransactionVisibility

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun CurrencyDetailScreen(
    assetId: Int,
    accountId: String?,
    onBack: () -> Unit,
    onOpenRate: () -> Unit,
    onAccountClick: (CurrencyAccount) -> Unit,
    onViewAllTransactions: (ticker: String, accountName: String?) -> Unit,
    onTransactionClick: (TransactionListItem) -> Unit,
    sharedTransitionScope: SharedTransitionScope,
    animatedVisibilityScope: AnimatedVisibilityScope,
    modifier: Modifier = Modifier,
    viewModel: CurrencyDetailViewModel = viewModel(factory = SvertoViewModelFactory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(assetId, accountId) { viewModel.load(assetId, accountId) }

    val snackbarHostState = remember { SnackbarHostState() }
    val haptics = LocalHapticFeedback.current
    LaunchedEffect(Unit) {
        viewModel.errors.collect {
            haptics.performHapticFeedback(HapticFeedbackType.Reject)
            snackbarHostState.showSnackbar(it)
        }
    }

    val detail = state.detail
    val accountName = if (accountId != null) detail?.accounts?.firstOrNull()?.accountName else null
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    val pullToRefreshState = rememberHapticPullToRefreshState()

    Scaffold(
        modifier = modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        snackbarHost = { SnackbarHost(snackbarHostState) },
        topBar = {
            MediumFlexibleTopAppBar(
                title = { Text(detail?.name.orEmpty()) },
                subtitle =
                    accountName?.let {
                        {
                            Text(
                                text = "in $it",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    },
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
    ) { padding ->
        when {
            detail == null && state.isLoading ->
                Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                    LoadingIndicator()
                }

            detail == null ->
                Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                    Text(
                        "Couldn't load this currency",
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
                        HeldSection(detail, onOpenRate)
                        if (accountId == null) AccountsSection(detail, onAccountClick)
                        RecentTransactionsSection(
                            transactions = detail.recentTransactions,
                            hasMore = detail.hasMoreTransactions,
                            onViewAll = { onViewAllTransactions(detail.ticker, accountName) },
                            onTransactionClick = onTransactionClick,
                            sharedTransitionScope = sharedTransitionScope,
                            animatedVisibilityScope = animatedVisibilityScope,
                        )
                        Spacer(Modifier.height(8.dp))
                    }
                }
        }
    }
}

@Composable
private fun HeldSection(
    detail: CurrencyDetail,
    onOpenRate: () -> Unit,
) {
    val chartData =
        detail.chartData
            .mapNotNull { period ->
                val timePeriod = TimePeriod.entries.find { it.label == period.period } ?: return@mapNotNull null
                timePeriod to period.points.map { ChartPoint(it.timestamp, it.value) }
            }.toMap()
    val value = detail.value?.takeUnless { detail.isBaseCurrency }
    val rate = detail.rate

    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        if (chartData.isNotEmpty()) {
            PortfolioChart(portfolioData = chartData, currencyTicker = detail.ticker)
        } else {
            Text(
                text = Money.format(detail.units, detail.ticker),
                style = MaterialTheme.typography.headlineLarge,
            )
        }

        if (value != null || rate != null) {
            Surface(
                shape = RoundedCornerShape(20.dp),
                color = MaterialTheme.colorScheme.surfaceBright,
            ) {
                Column {
                    if (value != null) {
                        ListItem(
                            headlineContent = { Text("Value in ${detail.baseTicker}") },
                            trailingContent = {
                                Text(
                                    text = Money.format(value, detail.baseTicker),
                                    style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                                    color = MaterialTheme.colorScheme.onSurface,
                                )
                            },
                            colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceBright),
                        )
                    }
                    if (value != null && rate != null) RowDivider()
                    if (rate != null) {
                        ListItem(
                            modifier = Modifier.clickable(onClick = onOpenRate),
                            headlineContent = { Text("1 ${detail.ticker} = ${formatRate(rate)} ${detail.baseTicker}") },
                            supportingContent = { Text("Exchange rate") },
                            trailingContent = {
                                Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null)
                            },
                            colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceBright),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun AccountsSection(
    detail: CurrencyDetail,
    onAccountClick: (CurrencyAccount) -> Unit,
) {
    if (detail.accounts.isEmpty()) return
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(
            text = "Accounts",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        Surface(
            shape = RoundedCornerShape(20.dp),
            color = MaterialTheme.colorScheme.surfaceBright,
        ) {
            Column {
                detail.accounts.forEachIndexed { index, account ->
                    ListItem(
                        modifier = Modifier.clickable { onAccountClick(account) },
                        leadingContent = {
                            Box(
                                modifier =
                                    Modifier
                                        .size(40.dp)
                                        .background(accountTypeContainerColor(account.accountTypeId), CircleShape),
                                contentAlignment = Alignment.Center,
                            ) {
                                Icon(
                                    imageVector = accountTypeIcon(account.accountTypeId),
                                    contentDescription = null,
                                    tint = accountTypeOnContainerColor(account.accountTypeId),
                                    modifier = Modifier.size(20.dp),
                                )
                            }
                        },
                        headlineContent = {
                            Text(account.accountName, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        },
                        supportingContent = { Text(accountTypeLabel(account.accountTypeId)) },
                        trailingContent = {
                            Text(
                                text = Money.format(account.units, detail.ticker),
                                style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                                color = MaterialTheme.colorScheme.onSurface,
                                maxLines = 1,
                            )
                        },
                        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceBright),
                    )
                    if (index < detail.accounts.lastIndex) RowDivider()
                }
            }
        }
    }
}

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
private fun RecentTransactionsSection(
    transactions: List<TransactionListItem>,
    hasMore: Boolean,
    onViewAll: () -> Unit,
    onTransactionClick: (TransactionListItem) -> Unit,
    sharedTransitionScope: SharedTransitionScope,
    animatedVisibilityScope: AnimatedVisibilityScope,
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(
            text = "Recent transactions",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        Surface(
            shape = RoundedCornerShape(20.dp),
            color = MaterialTheme.colorScheme.surfaceBright,
        ) {
            Column {
                if (transactions.isEmpty()) {
                    Text(
                        text = "No transactions",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(16.dp),
                    )
                }
                with(sharedTransitionScope) {
                    transactions.forEachIndexed { index, tx ->
                        val subtitle = tx.categoryName.ifEmpty { tx.typeLabel }
                        ListItem(
                            modifier =
                                Modifier
                                    .sharedBounds(
                                        sharedContentState = rememberSharedContentState(key = "tx_${tx.id}"),
                                        animatedVisibilityScope = animatedVisibilityScope,
                                    ).clickable { onTransactionClick(tx) }
                                    .alpha(if (tx.visibility == TransactionVisibility.GHOST) 0.55f else 1f),
                            leadingContent = {
                                TransactionGlyph(transaction = tx, modifier = Modifier.size(24.dp))
                            },
                            headlineContent = {
                                Text(
                                    text = tx.description,
                                    style = MaterialTheme.typography.bodyLarge,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            },
                            supportingContent = {
                                if (subtitle.isNotEmpty()) {
                                    Text(
                                        text = subtitle,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                }
                            },
                            trailingContent = { TransactionAmount(transaction = tx) },
                            colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceBright),
                        )
                        if (index < transactions.lastIndex) RowDivider()
                    }
                }
                if (hasMore) {
                    RowDivider()
                    TextButton(onClick = onViewAll, modifier = Modifier.fillMaxWidth()) {
                        Text("View all")
                    }
                }
            }
        }
    }
}
