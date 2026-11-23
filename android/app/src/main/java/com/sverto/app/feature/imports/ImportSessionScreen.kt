package com.sverto.app.feature.imports

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MediumFlexibleTopAppBar
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.core.aifeatures.AiFeaturesGate
import com.sverto.app.feature.assets.components.AssetPickerSheet
import com.sverto.app.feature.connectors.EmptyState
import com.sverto.app.feature.imports.review.ApplyBar
import com.sverto.app.feature.imports.review.ApplyBarMode
import com.sverto.app.feature.imports.review.ApplyConfirmDialog
import com.sverto.app.feature.imports.review.BalanceGapsSheet
import com.sverto.app.feature.imports.review.ProposedRowSheet
import com.sverto.app.feature.transactions.create.AccountPickerSheet
import kotlinx.coroutines.launch
import uniffi.sverto_core.ImportAsk
import uniffi.sverto_core.ImportAssetCandidate
import uniffi.sverto_core.ImportItemVerb
import uniffi.sverto_core.ImportSessionStatus
import uniffi.sverto_core.ImportSourceKind

@Suppress("LongMethod", "CyclomaticComplexMethod")
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ImportSessionScreen(
    sessionId: String,
    aiFeaturesGate: AiFeaturesGate,
    onBack: () -> Unit,
    onOpenItems: (filter: String) -> Unit,
    onCreateAccount: () -> Unit,
    onViewTransactions: () -> Unit,
    modifier: Modifier = Modifier,
    pickFilesOnOpen: Boolean = false,
    viewModel: ImportSessionViewModel = viewModel(factory = SvertoViewModelFactory),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val resolver = LocalContext.current.contentResolver
    val listState = rememberLazyListState()
    val currentOnBack by rememberUpdatedState(onBack)
    var menuOpen by remember { mutableStateOf(false) }
    var confirmDelete by remember { mutableStateOf(false) }
    var confirmApply by remember { mutableStateOf(false) }
    var pickingAccountFor by remember { mutableStateOf<String?>(null) }
    var searchingAsset by remember { mutableStateOf<ImportAsk.AssetMapping?>(null) }
    var openRowId by rememberSaveable { mutableStateOf<String?>(null) }
    var gapsKey by rememberSaveable { mutableStateOf<String?>(null) }
    var expanded by rememberSaveable { mutableStateOf(setOf<String>()) }
    var pickerShown by rememberSaveable { mutableStateOf(false) }

    LifecycleResumeEffect(sessionId) {
        viewModel.resume(sessionId)
        onPauseOrDispose { viewModel.pause() }
    }

    LaunchedEffect(viewModel) {
        viewModel.events.collect { event ->
            when (event) {
                is ImportSessionEvent.Message -> launch { snackbarHostState.showSnackbar(event.text) }
                is ImportSessionEvent.AiBlocked -> aiFeaturesGate.promptToEnable(event.retry)
                ImportSessionEvent.Deleted -> currentOnBack()
            }
        }
    }

    val filePicker =
        rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
            if (uris.isEmpty()) return@rememberLauncherForActivityResult
            scope.launch {
                val results = uris.map { readPickedFile(resolver, it) }
                results.forEach { result ->
                    when (result) {
                        is PickResult.TooLarge -> launch { snackbarHostState.showSnackbar("“${result.name}” is over 20 MB") }
                        is PickResult.Unreadable -> launch { snackbarHostState.showSnackbar("Couldn't open “${result.name}”") }
                        is PickResult.Picked -> Unit
                    }
                }
                viewModel.upload(results.filterIsInstance<PickResult.Picked>().map { it.file })
            }
        }

    LaunchedEffect(pickFilesOnOpen) {
        if (pickFilesOnOpen && !pickerShown) {
            pickerShown = true
            filePicker.launch(arrayOf("*/*"))
        }
    }

    val detail = state.detail
    val items = state.items?.takeIf { it.ready }
    val blocks =
        remember(state) {
            detail?.let { sessionBlocks(it, state.items, state.seenDuplicates, state.applied) }.orEmpty()
        }
    val barState =
        detail?.takeIf { it.summary.status != ImportSessionStatus.COMPLETED }?.let { session ->
            items?.takeIf { it.fingerprint != null }?.let { applyBarState(session, it) }
        }
    val navigate: (String) -> Unit = { target ->
        if (target.startsWith(ROW_TARGET)) {
            openRowId = target.removePrefix(ROW_TARGET)
        } else {
            val index = blocks.indexOfFirst { it.key == target }
            if (index >= 0) scope.launch { listState.animateScrollToItem(index) }
        }
    }

    val actions =
        SessionActions(
            onRetryCategorizing = viewModel::retryClassification,
            onChooseAccount = viewModel::chooseAccount,
            onPickAccount = { pickingAccountFor = it },
            onCreateAccount = { raw -> viewModel.createAccount(raw, onCreateAccount) },
            onSameAccount = viewModel::setSameAccount,
            onAnswerAccounts = viewModel::answerAccounts,
            onChooseAsset = viewModel::chooseAsset,
            onSearchAsset = { searchingAsset = it },
            onAnswerAsset = viewModel::answerAsset,
            onEnterBalance = viewModel::enterBalance,
            onSetVerb = viewModel::setVerb,
            onChooseDuplicate = viewModel::chooseDuplicate,
            onConfirmDuplicate = viewModel::confirmDuplicate,
            onReviewGaps = { gapsKey = it },
            onRowClick = { openRowId = it.id },
            onNavigate = navigate,
            onViewTransactions = onViewTransactions,
            onAddFiles = { aiFeaturesGate.runWhenEnabled { filePicker.launch(arrayOf("*/*")) } },
            onRetryFile = { id -> aiFeaturesGate.runWhenEnabled { viewModel.retryFile(id) } },
            onRemoveFile = viewModel::removeFile,
            onDateOrder = { id, order -> aiFeaturesGate.runWhenEnabled { viewModel.setDateOrder(id, order) } },
            onRetryUpload = viewModel::retryUpload,
            onDismissUpload = viewModel::dismissUpload,
            onOpenAllRows = { onOpenItems("all") },
        )

    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    Scaffold(
        modifier =
            modifier
                .fillMaxSize()
                .nestedScroll(scrollBehavior.nestedScrollConnection),
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        snackbarHost = { SnackbarHost(snackbarHostState) },
        topBar = {
            MediumFlexibleTopAppBar(
                title = { Text(detail?.summary?.title ?: "Import", maxLines = 1, overflow = TextOverflow.Ellipsis) },
                subtitle = detail?.let { { Text(sessionSubtitle(it)) } },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    if (detail != null) {
                        Box {
                            IconButton(onClick = { menuOpen = true }) {
                                Icon(Icons.Filled.MoreVert, contentDescription = "More options")
                            }
                            DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                                DropdownMenuItem(
                                    text = { Text("Delete import", color = MaterialTheme.colorScheme.error) },
                                    onClick = {
                                        menuOpen = false
                                        confirmDelete = true
                                    },
                                )
                            }
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
        bottomBar = {
            barState?.let { bar ->
                ApplyBar(
                    mode =
                        when (bar) {
                            is ApplyBarState.Apply -> ApplyBarMode.Apply
                            is ApplyBarState.Navigate -> ApplyBarMode.Navigate
                            is ApplyBarState.Disabled -> ApplyBarMode.Disabled
                        },
                    label =
                        when (bar) {
                            is ApplyBarState.Apply -> bar.label
                            is ApplyBarState.Navigate -> bar.label
                            is ApplyBarState.Disabled -> "Apply"
                        },
                    supporting =
                        when (bar) {
                            is ApplyBarState.Apply -> bar.summary
                            is ApplyBarState.Navigate -> null
                            is ApplyBarState.Disabled -> bar.text
                        },
                    applying = state.applying,
                    busy = state.working,
                    onClick = {
                        when (bar) {
                            is ApplyBarState.Apply -> if (bar.confirm == null) viewModel.apply() else confirmApply = true
                            is ApplyBarState.Navigate -> navigate(bar.target)
                            is ApplyBarState.Disabled -> Unit
                        }
                    },
                )
            }
        },
    ) { innerPadding ->
        Box(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(innerPadding),
            contentAlignment = Alignment.Center,
        ) {
            when {
                detail != null ->
                    SessionContent(
                        blocks = blocks,
                        state = state,
                        actions = actions,
                        listState = listState,
                        expanded = expanded,
                        onToggle = { key -> expanded = if (key in expanded) expanded - key else expanded + key },
                    )
                state.loadError != null ->
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        EmptyState(
                            icon = "circle-alert",
                            title = "Couldn't load this import",
                            body = state.loadError.orEmpty(),
                            modifier = Modifier.padding(horizontal = 32.dp),
                        )
                        Spacer(Modifier.height(16.dp))
                        FilledTonalButton(
                            onClick = { viewModel.resume(sessionId) },
                            shapes = ButtonDefaults.shapes(),
                        ) { Text("Try again") }
                    }
                else -> LoadingIndicator()
            }
        }
    }

    pickingAccountFor?.let { raw ->
        AccountPickerSheet(
            accounts = state.accounts.orEmpty(),
            selectedAccountId = state.accountChoices[raw],
            onSelect = { account ->
                viewModel.chooseAccount(raw, account.id)
                pickingAccountFor = null
            },
            onDismiss = { pickingAccountFor = null },
        )
    }

    searchingAsset?.let { ask ->
        AssetPickerSheet(
            title = "Which asset is “${ask.raw}”?",
            onSelect = { asset ->
                viewModel.chooseAsset(ask.askKey, ImportAssetCandidate(asset.id, asset.ticker, asset.name))
                searchingAsset = null
            },
            onDismiss = { searchingAsset = null },
        )
    }

    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text("Delete this import?") },
            text = {
                val sources = detail?.sources.orEmpty()
                val bankAccounts = sources.any { it.kind is ImportSourceKind.BankAccount }
                val bankOnly = bankAccounts && sources.all { it.kind is ImportSourceKind.BankAccount }
                val linksPending = bankAccounts && detail?.summary?.status != ImportSessionStatus.COMPLETED
                Text(
                    listOfNotNull(
                        if (bankOnly) "The import is deleted." else "The import and its uploaded files are deleted.",
                        "The bank account links it set up are removed too.".takeIf { linksPending },
                        "Nothing in your ledger changes, including transactions this import already applied.",
                    ).joinToString(" "),
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmDelete = false
                    viewModel.delete()
                }) { Text("Delete import", color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text("Keep import") } },
        )
    }

    val confirm = (barState as? ApplyBarState.Apply)?.confirm
    if (confirmApply && confirm != null) {
        ApplyConfirmDialog(
            summary = confirm.summary,
            warnings = confirm.warnings,
            risky = confirm.risky,
            onConfirm = {
                confirmApply = false
                viewModel.apply()
            },
            onDismiss = { confirmApply = false },
        )
    }

    items?.rows?.firstOrNull { it.id == openRowId }?.let { row ->
        ProposedRowSheet(
            row = row,
            onVerb = { verb ->
                openRowId = null
                viewModel.setVerb(listOf(row.id), verb)
            },
            onKeep = { askKey ->
                openRowId = null
                viewModel.answerUnlisted(askKey, false)
            },
            onDismiss = { openRowId = null },
        )
    }

    (blocks.firstOrNull { it.key == gapsKey } as? SessionBlock.BalanceCheck)?.let { block ->
        BalanceGapsSheet(
            check = block.check,
            adjustments = block.adjustments,
            enabled = !state.working,
            onSetAdjustments = { ids, add -> viewModel.setVerb(ids, if (add) ImportItemVerb.ADD else ImportItemVerb.AUTO) },
            onAnswerUnlisted = viewModel::answerUnlisted,
            onDismiss = { gapsKey = null },
        )
    }
}
