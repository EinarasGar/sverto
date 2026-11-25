package com.sverto.app.feature.properties

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material3.AlertDialog
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
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sverto.app.core.Money
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.core.ui.RowDivider
import com.sverto.app.core.ui.rememberHapticPullToRefreshState
import com.sverto.app.feature.portfolio.ChartPoint
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

private val valuationDateFormatter = DateTimeFormatter.ofPattern("MMM d, yyyy", Locale.US)

private fun valuationDate(valuation: ChartPoint): String =
    Instant
        .ofEpochSecond(valuation.date)
        .atZone(ZoneId.systemDefault())
        .format(valuationDateFormatter)

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PropertyValuationsScreen(
    assetId: Int,
    currencyAssetId: Int,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    viewModel: PropertyValuationsViewModel = viewModel(factory = SvertoViewModelFactory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(assetId, currencyAssetId) { viewModel.load(assetId, currencyAssetId) }

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

    var showAdd by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<ChartPoint?>(null) }
    var deleting by remember { mutableStateOf<ChartPoint?>(null) }

    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    val pullToRefreshState = rememberHapticPullToRefreshState()

    Scaffold(
        modifier = modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        snackbarHost = { SnackbarHost(snackbarHostState) },
        topBar = {
            MediumFlexibleTopAppBar(
                title = { Text("Valuations") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    IconButton(onClick = { showAdd = true }) {
                        Icon(Icons.Filled.Add, contentDescription = "Add valuation")
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
            LazyColumn(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
                when {
                    state.isLoading ->
                        item {
                            Box(Modifier.fillMaxWidth().padding(48.dp), contentAlignment = Alignment.Center) {
                                LoadingIndicator()
                            }
                        }

                    state.failed && state.valuations.isEmpty() ->
                        item { RetryMessage("Couldn't load the valuations.", onRetry = viewModel::refresh) }

                    state.valuations.isEmpty() ->
                        item {
                            Text(
                                text = "No valuations yet",
                                style = MaterialTheme.typography.bodyLarge,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.fillMaxWidth().padding(vertical = 48.dp),
                            )
                        }

                    else ->
                        item {
                            Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceBright) {
                                Column {
                                    state.valuations.forEachIndexed { index, valuation ->
                                        ValuationRow(
                                            valuation = valuation,
                                            currencyTicker = state.currencyTicker,
                                            onEdit = { editing = valuation },
                                            onDelete = { deleting = valuation },
                                        )
                                        if (index < state.valuations.lastIndex) RowDivider()
                                    }
                                }
                            }
                        }
                }
            }
        }
    }

    if (showAdd) {
        ValuationSheet(
            title = "Add valuation",
            currencyTicker = state.currencyTicker,
            onSubmit = { date, value ->
                viewModel.add(date, value)
                showAdd = false
            },
            onDismiss = { showAdd = false },
        )
    }
    editing?.let { valuation ->
        ValuationSheet(
            title = "Edit valuation",
            currencyTicker = state.currencyTicker,
            valuation = valuation,
            onSubmit = { date, value ->
                viewModel.edit(valuation, date, value)
                editing = null
            },
            onDismiss = { editing = null },
        )
    }
    deleting?.let { valuation ->
        DeleteValuationDialog(
            valuation = valuation,
            currencyTicker = state.currencyTicker,
            isLast = state.valuations.size == 1,
            onConfirm = {
                viewModel.delete(valuation)
                deleting = null
            },
            onDismiss = { deleting = null },
        )
    }
}

@Composable
private fun ValuationRow(
    valuation: ChartPoint,
    currencyTicker: String,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
) {
    val date = valuationDate(valuation)
    ListItem(
        modifier = Modifier.clickable(onClick = onEdit),
        headlineContent = {
            Text(
                text = Money.format(valuation.value, currencyTicker),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
            )
        },
        supportingContent = { Text(date) },
        trailingContent = {
            IconButton(onClick = onDelete) {
                Icon(Icons.Outlined.Delete, contentDescription = "Delete valuation from $date")
            }
        },
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceBright),
    )
}

@Composable
private fun DeleteValuationDialog(
    valuation: ChartPoint,
    currencyTicker: String,
    isLast: Boolean,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    val removes = "This removes the ${Money.format(valuation.value, currencyTicker)} valuation from ${valuationDate(valuation)}."
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Delete valuation?") },
        text = {
            Text(if (isLast) "$removes The property will have no value until you add a new one." else removes)
        },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text("Delete", color = MaterialTheme.colorScheme.error)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}
