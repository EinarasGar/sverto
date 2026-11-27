package com.sverto.app.feature.portfolio

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Warning
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.PullToRefreshDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sverto.app.SvertoApp
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.core.components.AppUpdateBanner
import com.sverto.app.core.components.UpdateBannerPlacement
import com.sverto.app.core.layout.LocalShellLayout
import com.sverto.app.core.layout.ShellLayout
import com.sverto.app.core.ui.HoldingsListSkeleton
import com.sverto.app.core.ui.PortfolioChartSkeleton
import com.sverto.app.core.ui.rememberHapticPullToRefreshState
import com.sverto.app.railEndToVerticalHinge
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.map
import uniffi.sverto_core.HoldingItem
import uniffi.sverto_core.LiabilityItem

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PortfolioScreen(
    modifier: Modifier = Modifier,
    onHoldingClick: (HoldingItem) -> Unit = {},
    onLiabilityClick: (LiabilityItem) -> Unit = {},
    viewModel: HomeViewModel = viewModel(factory = SvertoViewModelFactory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val isRefreshing by viewModel.isRefreshing.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val appUpdates = remember { (context.applicationContext as SvertoApp).appUpdates }
    LaunchedEffect(appUpdates) {
        appUpdates.presentation
            .map { it.autoPromptFlexible }
            .distinctUntilChanged()
            .filter { it }
            .collect { appUpdates.autoPrompt() }
    }

    val pullToRefreshState = rememberHapticPullToRefreshState()

    PullToRefreshBox(
        isRefreshing = isRefreshing,
        onRefresh = viewModel::refresh,
        state = pullToRefreshState,
        indicator = {
            PullToRefreshDefaults.LoadingIndicator(
                state = pullToRefreshState,
                isRefreshing = isRefreshing,
                modifier = Modifier.align(Alignment.TopCenter),
            )
        },
        modifier = modifier.fillMaxSize(),
    ) {
        val summary: @Composable () -> Unit
        val holdings: @Composable () -> Unit
        when {
            state.isLoading && state.holdings.isEmpty() && state.liabilities.isEmpty() -> {
                summary = { PortfolioChartSkeleton() }
                holdings = { HoldingsListSkeleton() }
            }

            state.error != null && state.holdings.isEmpty() && state.liabilities.isEmpty() -> {
                summary = { ErrorState(message = state.error!!, onRetry = viewModel::load) }
                holdings = {}
            }

            else -> {
                summary = {
                    val portfolioData: Map<TimePeriod, List<ChartPoint>> =
                        state.chartData
                            .mapNotNull { period ->
                                val tp =
                                    TimePeriod.entries.find { it.label == period.period }
                                        ?: return@mapNotNull null
                                tp to period.points.map { ChartPoint(it.timestamp, it.value) }
                            }.toMap()

                    if (portfolioData.isNotEmpty()) {
                        PortfolioChart(portfolioData = portfolioData, currencyTicker = state.baseTicker)
                    }
                }
                holdings = {
                    HoldingsList(
                        holdings = state.holdings,
                        liabilities = state.liabilities,
                        holdingsTotal = state.holdingsTotal,
                        liabilitiesTotal = state.liabilitiesTotal,
                        baseTicker = state.baseTicker,
                        onHoldingClick = onHoldingClick,
                        onLiabilityClick = onLiabilityClick,
                    )
                }
            }
        }

        val columnScroll = rememberScrollState()
        val summaryScroll = rememberScrollState()
        val holdingsScroll = rememberScrollState()
        if (LocalShellLayout.current == ShellLayout.Expanded) {
            val summaryWidthAtHinge = railEndToVerticalHinge()?.let { it - 24.dp - 24.dp / 2 }?.takeIf { it > 0.dp }
            Row(
                modifier = Modifier.fillMaxSize().padding(horizontal = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(24.dp),
            ) {
                Column(
                    modifier =
                        Modifier
                            .then(if (summaryWidthAtHinge != null) Modifier.width(summaryWidthAtHinge) else Modifier.weight(1f))
                            .fillMaxHeight()
                            .verticalScroll(summaryScroll),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    Spacer(Modifier.height(8.dp))
                    AppUpdateBanner(UpdateBannerPlacement.HOME)
                    summary()
                }
                Column(
                    modifier = Modifier.weight(1f).fillMaxHeight().verticalScroll(holdingsScroll),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    Spacer(Modifier.height(8.dp))
                    holdings()
                    Spacer(Modifier.height(16.dp))
                }
            }
        } else {
            Column(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .padding(horizontal = 16.dp)
                        .verticalScroll(columnScroll),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Spacer(Modifier.height(8.dp))
                AppUpdateBanner(UpdateBannerPlacement.HOME)
                summary()
                holdings()
                Spacer(Modifier.height(16.dp))
            }
        }
    }
}

@Composable
private fun ErrorState(
    message: String,
    onRetry: () -> Unit,
) {
    Box(
        modifier =
            Modifier
                .fillMaxWidth()
                .height(400.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Icon(
                imageVector = Icons.Outlined.Warning,
                contentDescription = null,
                modifier = Modifier.size(64.dp),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = message,
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = "Check your connection and try again",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
            )
            Spacer(Modifier.height(8.dp))
            FilledTonalButton(onClick = onRetry) {
                Text("Retry")
            }
        }
    }
}
