@file:OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MediumFlexibleTopAppBar
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.core.ui.RowDivider
import com.sverto.app.feature.imports.review.DetailLine
import com.sverto.app.feature.imports.review.ProposedRowSheet
import com.sverto.app.feature.imports.review.ProposedTransactionRow
import com.sverto.app.feature.imports.review.importDate
import com.sverto.app.feature.transactions.components.DateHeader
import kotlinx.coroutines.launch
import uniffi.sverto_core.ImportStagedRow

@Suppress("LongMethod")
@Composable
fun ImportItemsScreen(
    sessionId: String,
    initialFilter: String?,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    viewModel: ImportItemsViewModel = viewModel(factory = SvertoViewModelFactory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }
    var verbName by rememberSaveable { mutableStateOf(ItemsFilter.from(initialFilter).name) }
    var needsLook by rememberSaveable { mutableStateOf(false) }
    var accountId by rememberSaveable { mutableStateOf<String?>(null) }
    var searching by rememberSaveable { mutableStateOf(false) }
    var query by rememberSaveable { mutableStateOf("") }
    var openRowId by rememberSaveable { mutableStateOf<String?>(null) }
    val verb = ItemsFilter.valueOf(verbName)

    LaunchedEffect(sessionId) { viewModel.load(sessionId) }
    LaunchedEffect(viewModel) {
        viewModel.messages.collect { launch { snackbarHostState.showSnackbar(it) } }
    }

    val rows = state.items?.rows.orEmpty()
    val accounts = remember(rows) { accountChoices(rows) }
    val verbs = ItemsFilter.entries.filter { it == ItemsFilter.All || it == verb || rows.any { row -> row.verb == it.verb } }
    val shown =
        rows.filter { row ->
            (verb.verb == null || row.verb == verb.verb) &&
                (!needsLook || needsALook(row)) &&
                (accountId == null || row.accountId == accountId) &&
                matchesQuery(row, query)
        }
    val days = remember(shown) { groupRowsByDay(shown) }
    val listState = rememberLazyListState()
    LaunchedEffect(verbName, needsLook, accountId, query) { listState.scrollToItem(0) }
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    val barColors =
        TopAppBarDefaults.topAppBarColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainer,
            scrolledContainerColor = MaterialTheme.colorScheme.surfaceContainer,
        )

    Scaffold(
        modifier =
            modifier
                .fillMaxSize()
                .nestedScroll(scrollBehavior.nestedScrollConnection),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        snackbarHost = { SnackbarHost(snackbarHostState) },
        topBar = {
            if (searching) {
                SearchTopBar(
                    query = query,
                    onQuery = { query = it },
                    onClose = {
                        searching = false
                        query = ""
                    },
                )
            } else {
                MediumFlexibleTopAppBar(
                    title = { Text("Proposed transactions") },
                    subtitle = { Text(countOf(shown.size.toLong(), "transaction")) },
                    navigationIcon = {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                        }
                    },
                    actions = {
                        IconButton(onClick = { searching = true }) {
                            Icon(Icons.Filled.Search, contentDescription = "Search transactions")
                        }
                    },
                    colors = barColors,
                    scrollBehavior = scrollBehavior,
                )
            }
        },
    ) { innerPadding ->
        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(innerPadding),
        ) {
            ItemsFilterBar(
                needsLookCount = rows.count(::needsALook),
                needsLook = needsLook,
                onNeedsLook = { needsLook = it },
                accounts = accounts,
                accountId = accountId,
                onAccount = { accountId = it },
                verbs = verbs,
                verb = verb,
                onVerb = { verbName = it.name },
                modifier = Modifier.padding(vertical = 8.dp),
            )
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                when {
                    state.items == null && state.loadError != null -> DetailLine(state.loadError.orEmpty())
                    state.items == null -> LoadingIndicator()
                    shown.isEmpty() -> DetailLine("No transactions match.")
                    else ->
                        DayList(
                            days = days,
                            listState = listState,
                            showVerb = verb == ItemsFilter.All,
                            onRowClick = { openRowId = it.id },
                        )
                }
            }
        }
    }

    rows.firstOrNull { it.id == openRowId }?.let { row ->
        ProposedRowSheet(
            row = row,
            onVerb = { choice ->
                openRowId = null
                viewModel.setVerb(row.id, choice)
            },
            onKeep = { askKey ->
                openRowId = null
                viewModel.keep(askKey)
            },
            onDismiss = { openRowId = null },
        )
    }
}

@Composable
private fun SearchTopBar(
    query: String,
    onQuery: (String) -> Unit,
    onClose: () -> Unit,
) {
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    TopAppBar(
        title = {
            TextField(
                value = query,
                onValueChange = onQuery,
                placeholder = { Text("Search description or amount", style = MaterialTheme.typography.bodyLarge) },
                textStyle = MaterialTheme.typography.bodyLarge,
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                colors =
                    TextFieldDefaults.colors(
                        focusedContainerColor = Color.Transparent,
                        unfocusedContainerColor = Color.Transparent,
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent,
                    ),
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .focusRequester(focus),
            )
        },
        navigationIcon = {
            IconButton(onClick = onClose) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Close search")
            }
        },
        actions = {
            if (query.isNotEmpty()) {
                IconButton(onClick = { onQuery("") }) {
                    Icon(Icons.Filled.Close, contentDescription = "Clear search")
                }
            }
        },
        colors =
            TopAppBarDefaults.topAppBarColors(
                containerColor = MaterialTheme.colorScheme.surfaceContainer,
                scrolledContainerColor = MaterialTheme.colorScheme.surfaceContainer,
            ),
    )
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun DayList(
    days: List<Pair<String, List<ImportStagedRow>>>,
    listState: LazyListState,
    showVerb: Boolean,
    onRowClick: (ImportStagedRow) -> Unit,
) {
    LazyColumn(
        state = listState,
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = 32.dp),
    ) {
        days.forEach { (label, dayRows) ->
            stickyHeader(key = "header-$label") { DateHeader(label) }
            item(key = "day-$label") {
                Surface(
                    shape = RoundedCornerShape(16.dp),
                    color = MaterialTheme.colorScheme.surfaceBright,
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp),
                ) {
                    Column {
                        dayRows.forEachIndexed { index, row ->
                            if (index > 0) RowDivider()
                            ProposedTransactionRow(row = row, onClick = { onRowClick(row) }, showVerb = showVerb)
                        }
                    }
                }
            }
        }
    }
}

private fun groupRowsByDay(rows: List<ImportStagedRow>): List<Pair<String, List<ImportStagedRow>>> =
    rows
        .sortedByDescending { it.date }
        .groupBy { importDate(it.date) }
        .toList()
