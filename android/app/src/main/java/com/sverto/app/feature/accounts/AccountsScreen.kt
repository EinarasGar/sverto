package com.sverto.app.feature.accounts

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.foundation.background
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.outlined.AccountBalance
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Warning
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FloatingActionButtonMenu
import androidx.compose.material3.FloatingActionButtonMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.ToggleFloatingActionButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.PullToRefreshDefaults
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.core.ui.shimmerBrush
import com.sverto.app.feature.accounts.components.AccountCard
import com.sverto.app.feature.accounts.components.accountTypeDisplayOrder
import com.sverto.app.feature.accounts.components.accountTypeLabel
import uniffi.sverto_core.AccountListItem
import uniffi.sverto_core.AccountsState

@Suppress("MultipleEmitters")
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class, ExperimentalSharedTransitionApi::class)
@Composable
fun AccountsScreen(
    onAccountClick: (AccountListItem) -> Unit,
    onAddAccount: () -> Unit,
    onAddProperty: () -> Unit,
    sharedTransitionScope: SharedTransitionScope,
    animatedVisibilityScope: AnimatedVisibilityScope,
    modifier: Modifier = Modifier,
    viewModel: AccountsViewModel = viewModel(factory = SvertoViewModelFactory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val isRefreshing by viewModel.isRefreshing.collectAsStateWithLifecycle()
    var addMenuExpanded by rememberSaveable { mutableStateOf(false) }
    BackHandler(addMenuExpanded) { addMenuExpanded = false }

    val pullToRefreshState = rememberPullToRefreshState()

    Scaffold(
        modifier = modifier.fillMaxSize(),
        // Transparent so the host (MainScreen) surfaceContainer background shows through and the
        // surfaceBright account cards keep their contrast.
        containerColor = Color.Transparent,
        floatingActionButton = {
            AddMenu(
                expanded = addMenuExpanded,
                onToggle = { addMenuExpanded = !addMenuExpanded },
                onAddAccount = onAddAccount,
                onAddProperty = onAddProperty,
            )
        },
    ) { padding ->
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
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(padding),
        ) {
            Column(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .padding(horizontal = 16.dp)
                        .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Spacer(Modifier.height(8.dp))

                when {
                    state.isLoading && state.accounts.isEmpty() -> {
                        AccountsScreenSkeleton()
                    }

                    state.error != null && state.accounts.isEmpty() -> {
                        ErrorState(message = state.error!!, onRetry = viewModel::load)
                    }

                    else -> {
                        AccountsContent(
                            state = state,
                            onAccountClick = onAccountClick,
                            sharedTransitionScope = sharedTransitionScope,
                            animatedVisibilityScope = animatedVisibilityScope,
                        )
                    }
                }

                // Bottom spacer so the FAB doesn't overlap the last account card.
                Spacer(Modifier.height(88.dp))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun AddMenu(
    expanded: Boolean,
    onToggle: () -> Unit,
    onAddAccount: () -> Unit,
    onAddProperty: () -> Unit,
) {
    FloatingActionButtonMenu(
        expanded = expanded,
        button = {
            ToggleFloatingActionButton(
                checked = expanded,
                onCheckedChange = { onToggle() },
            ) {
                val imageVector by remember {
                    derivedStateOf { if (checkedProgress > 0.5f) Icons.Filled.Close else Icons.Filled.Add }
                }
                Icon(
                    painter = rememberVectorPainter(imageVector),
                    contentDescription = if (expanded) "Close menu" else "Add",
                )
            }
        },
    ) {
        FloatingActionButtonMenuItem(
            onClick = {
                onToggle()
                onAddProperty()
            },
            icon = { Icon(Icons.Outlined.Home, contentDescription = null) },
            text = { Text("Add property") },
        )
        FloatingActionButtonMenuItem(
            onClick = {
                onToggle()
                onAddAccount()
            },
            icon = { Icon(Icons.Outlined.AccountBalance, contentDescription = null) },
            text = { Text("Add account") },
        )
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

@Suppress("MultipleEmitters")
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
private fun AccountsContent(
    state: AccountsState,
    onAccountClick: (AccountListItem) -> Unit,
    sharedTransitionScope: SharedTransitionScope,
    animatedVisibilityScope: AnimatedVisibilityScope,
) {
    val sections =
        remember(state.accounts) {
            val grouped =
                state.accounts
                    .filter { it.linkedPropertyId == null }
                    .groupBy { it.accountTypeId }
            val orderedTypeIds =
                accountTypeDisplayOrder.filter(grouped::containsKey) +
                    grouped.keys.filterNot(accountTypeDisplayOrder::contains)
            orderedTypeIds.map { typeId -> typeId to grouped.getValue(typeId) }
        }

    sections.forEach { (typeId, accounts) ->
        AccountTypeSection(
            label = accountTypeLabel(typeId),
            accounts = accounts,
            baseTicker = state.baseTicker,
            onAccountClick = onAccountClick,
            sharedTransitionScope = sharedTransitionScope,
            animatedVisibilityScope = animatedVisibilityScope,
        )
    }
}

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
private fun AccountTypeSection(
    label: String,
    accounts: List<AccountListItem>,
    baseTicker: String,
    onAccountClick: (AccountListItem) -> Unit,
    sharedTransitionScope: SharedTransitionScope,
    animatedVisibilityScope: AnimatedVisibilityScope,
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(
            text = label,
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 4.dp),
        )
        accounts.forEach { account ->
            AccountCard(
                account = account,
                baseTicker = baseTicker,
                onClick = { onAccountClick(account) },
                sharedTransitionScope = sharedTransitionScope,
                animatedVisibilityScope = animatedVisibilityScope,
            )
        }
    }
}

@Composable
private fun AccountsScreenSkeleton() {
    val brush = shimmerBrush()

    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        repeat(5) {
            Box(
                Modifier
                    .fillMaxWidth()
                    .height(80.dp)
                    .background(brush, RoundedCornerShape(28.dp)),
            )
        }
    }
}
