@file:OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeFlexibleTopAppBar
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SmallExtendedFloatingActionButton
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.PullToRefreshDefaults
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.core.aifeatures.AiFeaturesGate
import com.sverto.app.core.icons.LucideIcon
import com.sverto.app.core.ui.RowDivider
import com.sverto.app.feature.connectors.EmptyState
import com.sverto.app.feature.connectors.HeroButton
import com.sverto.app.feature.connectors.relativeTime
import com.sverto.app.feature.imports.review.SectionHeader
import com.sverto.app.feature.imports.review.ToneChip
import com.sverto.app.feature.imports.review.containerColor
import com.sverto.app.feature.imports.review.contentColor
import kotlinx.coroutines.launch
import uniffi.sverto_core.ImportSessionStatus
import uniffi.sverto_core.ImportSessionSummary

@Suppress("LongMethod")
@Composable
fun ImportsScreen(
    aiFeaturesGate: AiFeaturesGate,
    onBack: () -> Unit,
    onOpenSession: (String) -> Unit,
    onSessionCreated: (String) -> Unit,
    modifier: Modifier = Modifier,
    viewModel: ImportsViewModel = viewModel(factory = SvertoViewModelFactory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }
    val listState = rememberLazyListState()
    val currentOnSessionCreated by rememberUpdatedState(onSessionCreated)
    val fabExpanded by remember { derivedStateOf { listState.firstVisibleItemIndex == 0 } }
    val startImport = { aiFeaturesGate.runWhenEnabled { viewModel.create() } }

    LifecycleResumeEffect(Unit) {
        viewModel.load()
        onPauseOrDispose { }
    }

    LaunchedEffect(viewModel) {
        viewModel.events.collect { event ->
            when (event) {
                is ImportsEvent.Created -> currentOnSessionCreated(event.sessionId)
                ImportsEvent.AiBlocked -> aiFeaturesGate.promptToEnable { viewModel.create() }
                is ImportsEvent.Message -> launch { snackbarHostState.showSnackbar(event.text) }
            }
        }
    }

    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    val pullState = rememberPullToRefreshState()
    val sessions = state.sessions
    Scaffold(
        modifier =
            modifier
                .fillMaxSize()
                .nestedScroll(scrollBehavior.nestedScrollConnection),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        snackbarHost = { SnackbarHost(snackbarHostState) },
        topBar = {
            LargeFlexibleTopAppBar(
                title = { Text("Imports") },
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
        floatingActionButton = {
            if (!sessions.isNullOrEmpty()) {
                SmallExtendedFloatingActionButton(
                    text = { Text("New import") },
                    icon = { LucideIcon(name = "plus", tint = MaterialTheme.colorScheme.onPrimaryContainer) },
                    onClick = { if (!state.creating) startImport() },
                    expanded = fabExpanded,
                    modifier = Modifier.semantics { contentDescription = "New import" },
                )
            }
        },
    ) { innerPadding ->
        PullToRefreshBox(
            isRefreshing = state.refreshing,
            onRefresh = { viewModel.load(pullToRefresh = true) },
            state = pullState,
            indicator = {
                PullToRefreshDefaults.LoadingIndicator(
                    state = pullState,
                    isRefreshing = state.refreshing,
                    modifier = Modifier.align(Alignment.TopCenter),
                )
            },
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(innerPadding),
            contentAlignment = Alignment.Center,
        ) {
            when {
                sessions == null && state.loadError != null ->
                    Column(
                        modifier = Modifier.verticalScroll(rememberScrollState()),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        EmptyState(icon = "circle-alert", title = "Couldn't load imports", body = state.loadError.orEmpty())
                        Spacer(Modifier.height(16.dp))
                        FilledTonalButton(onClick = { viewModel.load() }, shapes = ButtonDefaults.shapes()) {
                            Text("Try again")
                        }
                    }
                sessions == null -> LoadingIndicator()
                sessions.isEmpty() -> EmptyImports(creating = state.creating, onStart = startImport)
                else ->
                    LazyColumn(
                        state = listState,
                        modifier = Modifier.fillMaxSize(),
                        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 96.dp),
                    ) {
                        sessionSection("In progress", sessions.filter { it.status != ImportSessionStatus.COMPLETED }, onOpenSession)
                        sessionSection("Finished", sessions.filter { it.status == ImportSessionStatus.COMPLETED }, onOpenSession)
                    }
            }
        }
    }
}

private fun LazyListScope.sessionSection(
    title: String,
    sessions: List<ImportSessionSummary>,
    onOpenSession: (String) -> Unit,
) {
    if (sessions.isEmpty()) return
    item(key = "header-$title") { SectionHeader(title, Modifier.padding(top = 8.dp)) }
    item(key = "card-$title") {
        Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceBright) {
            Column {
                sessions.forEachIndexed { index, session ->
                    if (index > 0) RowDivider()
                    SessionRow(
                        session = session,
                        showChip = session.status != ImportSessionStatus.COMPLETED,
                        onClick = { onOpenSession(session.id) },
                    )
                }
            }
        }
    }
}

@Composable
private fun SessionRow(
    session: ImportSessionSummary,
    showChip: Boolean,
    onClick: () -> Unit,
) {
    val copy = sessionStatusCopy(session.status)
    ListItem(
        modifier = Modifier.clickable(onClick = onClick),
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceBright),
        leadingContent = {
            Box(
                modifier =
                    Modifier
                        .size(40.dp)
                        .clip(MaterialShapes.Cookie9Sided.toShape())
                        .background(copy.tone.containerColor()),
                contentAlignment = Alignment.Center,
            ) {
                LucideIcon(name = listGlyph(session.status), tint = copy.tone.contentColor(), modifier = Modifier.size(20.dp))
            }
        },
        headlineContent = { Text(session.title, maxLines = 2, overflow = TextOverflow.Ellipsis) },
        supportingContent = {
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
                itemVerticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = "Started ${relativeTime(session.createdAt)}",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                if (showChip) ToneChip(label = copy.label, tone = copy.tone)
            }
        },
    )
}

private fun listGlyph(status: ImportSessionStatus): String =
    when (status) {
        ImportSessionStatus.COLLECTING -> "file-up"
        ImportSessionStatus.PARSING, ImportSessionStatus.RECONCILING -> "hourglass"
        ImportSessionStatus.NEEDS_INPUT -> "message-circle-question-mark"
        ImportSessionStatus.PROPOSAL_READY -> "list-checks"
        ImportSessionStatus.COMPLETED -> "check"
    }

private val EMPTY_STEPS = listOf("Add statements", "Answer a few questions", "Review and apply")

@Composable
private fun EmptyImports(
    creating: Boolean,
    onStart: () -> Unit,
) {
    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp, vertical = 16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        EmptyState(
            icon = "file-up",
            title = "Import your statements",
            body = "Bring in transactions from bank and broker exports. Nothing is written to your ledger until you approve it.",
        )
        Spacer(Modifier.height(24.dp))
        Surface(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(24.dp),
            color = MaterialTheme.colorScheme.surfaceBright,
        ) {
            Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                EMPTY_STEPS.forEachIndexed { index, step ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(
                            modifier =
                                Modifier
                                    .size(32.dp)
                                    .clip(CircleShape)
                                    .background(MaterialTheme.colorScheme.secondaryContainer),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                text = "${index + 1}",
                                style = MaterialTheme.typography.labelLarge,
                                color = MaterialTheme.colorScheme.onSecondaryContainer,
                            )
                        }
                        Spacer(Modifier.size(16.dp))
                        Text(step, style = MaterialTheme.typography.bodyLarge)
                    }
                }
            }
        }
        Spacer(Modifier.height(24.dp))
        HeroButton(
            text = "Start an import",
            onClick = onStart,
            enabled = !creating,
            icon = "plus",
            modifier = Modifier.fillMaxWidth(),
        )
    }
}
