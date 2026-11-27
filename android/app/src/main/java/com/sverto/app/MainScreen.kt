package com.sverto.app

import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContentScope
import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsTopHeight
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.Scaffold
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.rememberSearchBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.IntOffset
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NamedNavArgument
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavDeepLink
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavGraphBuilder
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import androidx.navigation.navDeepLink
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.core.aifeatures.AiFeaturesConsentDialog
import com.sverto.app.core.aifeatures.AiFeaturesGate
import com.sverto.app.core.aifeatures.rememberAiFeaturesGate
import com.sverto.app.core.components.AttachmentSheet
import com.sverto.app.core.components.createCameraImageUri
import com.sverto.app.core.layout.CappedPane
import com.sverto.app.core.layout.LocalShellLayout
import com.sverto.app.core.layout.ShellLayout
import com.sverto.app.core.navigation.TopLevelRoute
import com.sverto.app.core.ui.OfflineBanner
import com.sverto.app.feature.accounts.AccountDetailScreen
import com.sverto.app.feature.accounts.AddAccountScreen
import com.sverto.app.feature.accounts.AssetDetailScreen
import com.sverto.app.feature.aichat.AiChatScreen
import com.sverto.app.feature.aichat.AiChatViewModel
import com.sverto.app.feature.aichat.ConversationDrawer
import com.sverto.app.feature.assets.AssetOverviewScreen
import com.sverto.app.feature.assets.AssetOverviewViewModel
import com.sverto.app.feature.assets.CustomAssetsScreen
import com.sverto.app.feature.categories.CustomCategoriesScreen
import com.sverto.app.feature.connectors.BindingSetupScreen
import com.sverto.app.feature.connectors.BindingSetupViewModel
import com.sverto.app.feature.connectors.ConnectEnableBankingScreen
import com.sverto.app.feature.connectors.ConnectTrading212Screen
import com.sverto.app.feature.connectors.ConnectTrueLayerScreen
import com.sverto.app.feature.connectors.ConnectionDetailScreen
import com.sverto.app.feature.connectors.ConnectorDetailScreen
import com.sverto.app.feature.connectors.ConnectorsScreen
import com.sverto.app.feature.connectors.ProviderAccountDetailScreen
import com.sverto.app.feature.imports.ImportItemsScreen
import com.sverto.app.feature.imports.ImportSessionScreen
import com.sverto.app.feature.imports.ImportsScreen
import com.sverto.app.feature.portfolio.CurrencyDetailScreen
import com.sverto.app.feature.portfolio.PortfolioScreen
import com.sverto.app.feature.portfolio.isCurrencyAssetType
import com.sverto.app.feature.properties.AddPropertyScreen
import com.sverto.app.feature.properties.PropertyDetailScreen
import com.sverto.app.feature.properties.PropertyValuationsScreen
import com.sverto.app.feature.server.AppSessionViewModel
import com.sverto.app.feature.settings.SettingsScreen
import com.sverto.app.feature.transactions.ScopedTransactionsScreen
import com.sverto.app.feature.transactions.TransactionDetailScreen
import com.sverto.app.feature.transactions.TransactionsViewModel
import com.sverto.app.feature.transactions.create.CreateTransactionScreen
import com.sverto.app.feature.transactions.create.CreateTransactionViewModel
import com.sverto.app.feature.transactions.create.GroupEditDisplayData
import com.sverto.app.feature.transactions.create.apiTypeToConfigKey
import com.sverto.app.feature.transactions.group.CreateTransactionGroupScreen
import com.sverto.app.feature.transactions.group.CreateTransactionGroupViewModel
import com.sverto.app.feature.transactions.group.GroupTransactionItem
import com.sverto.app.feature.transactions.quickupload.PreparedQuickUpload
import com.sverto.app.feature.transactions.quickupload.QuickUploadUiItem
import com.sverto.app.feature.transactions.quickupload.QuickUploadViewModel
import com.sverto.app.feature.transactions.quickupload.prepareQuickUpload
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.sverto_core.ConnectionStatus
import uniffi.sverto_core.TransactionListItem
import uniffi.sverto_core.TransactionListScope
import com.sverto.app.feature.assets.AssetDetailScreen as MarketAssetDetailScreen

private const val TRANSACTION_DETAIL_ROUTE = "transactionDetail/{txId}"
private const val EDIT_TRANSACTION_ROUTE = "editTransaction/{typeKey}/{txId}"
private const val EDIT_GROUP_ROUTE = "editTransactionGroup/{groupId}"
private const val GROUP_ADD_TXN_ROUTE = "groupAddTransaction/{typeKey}"
private const val GROUP_EDIT_TXN_ROUTE = "groupEditTransaction/{typeKey}/{index}"
private const val ACCOUNT_DETAIL_ROUTE = "accountDetail/{accountId}/{accountName}/{accountTypeId}"
private const val ASSET_DETAIL_ROUTE = "assetDetail/{accountId}/{assetId}?accountName={accountName}"
private const val ACCOUNT_TRANSACTIONS_ROUTE = "accountTransactions/{accountId}"
private const val ASSET_TRANSACTIONS_ROUTE =
    "assetTransactions/{assetId}?accountId={accountId}&accountName={accountName}&title={title}"
private const val ADD_ACCOUNT_ROUTE = "addAccount"
private const val EDIT_ACCOUNT_ROUTE = "editAccount/{accountId}"
private const val ADD_PROPERTY_ROUTE = "addProperty"
private const val PROPERTY_DETAIL_ROUTE = "property/{propertyId}/{assetId}"
private const val CURRENCY_DETAIL_ROUTE = "currency/{assetId}?accountId={accountId}"
private const val CURRENCY_TRANSACTIONS_ROUTE = "currencyTransactions/{assetId}?title={title}"
private const val PROPERTY_VALUATIONS_ROUTE = "propertyValuations/{assetId}/{currencyAssetId}"
private const val MARKET_ASSET_DETAIL_ROUTE = "asset/{assetId}?userAsset={userAsset}"
private const val ASSET_OVERVIEW_ROUTE = "assetOverview/{assetId}?userAsset={userAsset}"
private const val CUSTOM_ASSETS_ROUTE = "customAssets"
private const val CONNECTORS_ROUTE = "connectors"
private const val CONNECTOR_DETAIL_ROUTE = "connectorDetail/{providerKind}"
private const val CONNECTION_DETAIL_ROUTE = "connectionDetail/{connectionId}"
private const val CONNECT_TRUELAYER_ROUTE = "connectTrueLayer?code={code}&state={state}&error={error}"
private const val CONNECT_TRADING212_ROUTE = "connectTrading212"
private const val CONNECT_ENABLEBANKING_ROUTE =
    "connectEnableBanking?code={code}&state={state}&error={error}"
private const val BINDING_SETUP_ROUTE = "bindingSetup/{connectionId}"
private const val IMPORTS_ROUTE = "imports"
private const val IMPORT_SESSION_ROUTE = "importSession/{sessionId}?pickFiles={pickFiles}"
private const val IMPORT_ITEMS_ROUTE = "importItems/{sessionId}?filter={filter}"
private const val PROVIDER_ACCOUNT_DETAIL_ROUTE =
    "providerAccountDetail/{connectionId}/{providerAccountId}?name={name}&currency={currency}"

private data class TransactionDetailState(
    val transaction: TransactionListItem,
    val isInGroup: Boolean,
)

@Suppress("LongMethod", "CyclomaticComplexMethod", "ModifierMissing", "ViewModelForwarding")
@OptIn(
    ExperimentalMaterial3Api::class,
    ExperimentalMaterial3ExpressiveApi::class,
    ExperimentalSharedTransitionApi::class,
)
@Composable
fun MainScreen(
    sharedFileUris: List<Uri> = emptyList(),
    onSharedFilesHandled: () -> Unit = {},
    sessionViewModel: AppSessionViewModel? = null,
) {
    val navController = rememberNavController()
    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentRoute = navBackStackEntry?.destination?.route
    val isTopLevel = TopLevelRoute.entries.any { it.route == currentRoute }
    val isAiChatTab = currentRoute == TopLevelRoute.AiChat.route
    val drawerState = rememberDrawerState(initialValue = DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val activity = context as ComponentActivity
    val shellLayout = LocalShellLayout.current
    val currentTab = remember(navBackStackEntry) { navController.currentTab() }
    val listPaneTab =
        currentTab.takeIf {
            shellLayout == ShellLayout.Expanded && it != TopLevelRoute.Portfolio && !isFocusRoute(currentRoute)
        }
    val backStack by navController.currentBackStack.collectAsStateWithLifecycle()
    val listPaneOpenId =
        listPaneTab?.let { tab ->
            val root = backStack.indexOfLast { it.destination.route == tab.route }
            val entry = if (root >= 0) backStack.getOrNull(root + 1) else null
            val args = entry?.arguments
            when (entry?.destination?.route) {
                TRANSACTION_DETAIL_ROUTE -> args?.getString("txId")
                ACCOUNT_DETAIL_ROUTE -> args?.getString("accountId")
                PROPERTY_DETAIL_ROUTE -> args?.getString("propertyId")
                else -> null
            }
        }
    val modules = mainStartupModules(currentRoute, listPaneTab?.route, sharedFileUris.isNotEmpty())
    val transactionsViewModel =
        if (modules.transactions) {
            viewModel<TransactionsViewModel>(viewModelStoreOwner = activity, factory = SvertoViewModelFactory)
        } else {
            null
        }
    val quickUploadViewModel =
        if (modules.quickUploads) {
            viewModel<QuickUploadViewModel>(viewModelStoreOwner = activity, factory = SvertoViewModelFactory)
        } else {
            null
        }
    val aiChatViewModel =
        if (modules.aiChat) {
            viewModel<AiChatViewModel>(viewModelStoreOwner = activity, factory = SvertoViewModelFactory)
        } else {
            null
        }

    // Hoisted above the app bar's AnimatedVisibility so opening a transaction from search
    // doesn't tear down the query and results when the top bar leaves composition.
    val transactionSearchBarState = rememberSearchBarState()
    val transactionSearchTextState = rememberTextFieldState()
    val transactionSearchListState = rememberLazyListState()
    val transactionsListState = rememberLazyListState()
    val accountsScrollState = rememberScrollState()

    val transactionCache = remember { mutableMapOf<String, TransactionDetailState>() }
    val pendingGroupTransaction = remember { mutableStateOf<GroupTransactionItem?>(null) }
    val editingGroupIndex = remember { mutableStateOf<Int?>(null) }
    val pendingGroupSelection = remember { mutableStateOf<List<String>>(emptyList()) }

    val quickUploadItems =
        quickUploadViewModel
            ?.items
            ?.collectAsStateWithLifecycle()
            ?.value
            .orEmpty()
    val selectedTransactionIds =
        transactionsViewModel
            ?.selectedIds
            ?.collectAsStateWithLifecycle()
            ?.value
            .orEmpty()
    val transactionSelectionActive =
        currentRoute == TopLevelRoute.Transactions.route && selectedTransactionIds.isNotEmpty()

    val appStore = remember { (context.applicationContext as SvertoApp).appStore }
    val aiFeaturesGate = rememberAiFeaturesGate(appStore)
    val quickUploadsHeldByAiFeatures =
        quickUploadViewModel
            ?.heldByAiFeatures
            ?.collectAsStateWithLifecycle()
            ?.value == true
    LaunchedEffect(quickUploadsHeldByAiFeatures) {
        if (quickUploadsHeldByAiFeatures) {
            quickUploadViewModel.clearHeldByAiFeatures()
            aiFeaturesGate.promptToEnable { quickUploadViewModel.refresh() }
        }
    }
    val appUpdates = remember { (context.applicationContext as SvertoApp).appUpdates }
    val updatePresentation = appUpdates.presentation.collectAsStateWithLifecycle()
    val showUpdateBadge by remember { derivedStateOf { updatePresentation.value.showBadge } }
    val aiStreaming =
        aiChatViewModel?.let { vm ->
            remember(vm) { vm.uiState.map { it.isStreaming }.distinctUntilChanged() }
                .collectAsStateWithLifecycle(initialValue = false)
                .value
        } == true
    val restartSafe = isTopLevel && !aiStreaming
    LaunchedEffect(restartSafe) { appUpdates.setRestartSafe(restartSafe) }

    DisposableEffect(navController) {
        val listener =
            androidx.core.util.Consumer<Intent> { intent ->
                navController.handleDeepLink(intent)
            }
        activity.addOnNewIntentListener(listener)
        onDispose { activity.removeOnNewIntentListener(listener) }
    }

    suspend fun prepareAndQueue(uri: Uri) {
        when (val prepared = prepareQuickUpload(context.contentResolver, uri)) {
            is PreparedQuickUpload.Ready ->
                quickUploadViewModel?.queueUpload(prepared.data, prepared.thumbnail, prepared.mimeType)
            is PreparedQuickUpload.Rejected -> Toast.makeText(context, prepared.message, Toast.LENGTH_SHORT).show()
        }
    }

    val quickUploadPhotosLauncher =
        rememberLauncherForActivityResult(
            contract = ActivityResultContracts.PickMultipleVisualMedia(),
        ) { uris ->
            scope.launch { uris.forEach { prepareAndQueue(it) } }
        }
    val quickUploadFilesLauncher =
        rememberLauncherForActivityResult(
            contract = ActivityResultContracts.OpenMultipleDocuments(),
        ) { uris ->
            scope.launch { uris.forEach { prepareAndQueue(it) } }
        }
    var quickUploadCameraUri by rememberSaveable { mutableStateOf<Uri?>(null) }
    val quickUploadCameraLauncher =
        rememberLauncherForActivityResult(
            contract = ActivityResultContracts.TakePicture(),
        ) { saved ->
            val uri = quickUploadCameraUri ?: return@rememberLauncherForActivityResult
            quickUploadCameraUri = null
            scope.launch {
                if (saved) prepareAndQueue(uri)
                withContext(Dispatchers.IO) { context.contentResolver.delete(uri, null, null) }
            }
        }
    var showQuickUploadSheet by remember { mutableStateOf(false) }
    if (showQuickUploadSheet) {
        AttachmentSheet(
            onDismiss = { showQuickUploadSheet = false },
            onPhotos = {
                showQuickUploadSheet = false
                quickUploadPhotosLauncher.launch(
                    PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly),
                )
            },
            onCamera = {
                showQuickUploadSheet = false
                val uri = createCameraImageUri(context)
                quickUploadCameraUri = uri
                quickUploadCameraLauncher.launch(uri)
            },
            onFiles = {
                showQuickUploadSheet = false
                quickUploadFilesLauncher.launch(arrayOf("image/*", "application/pdf", "text/*"))
            },
        )
    }

    LaunchedEffect(sharedFileUris) {
        if (sharedFileUris.isNotEmpty()) {
            val uris = sharedFileUris
            onSharedFilesHandled()
            navController.navigate(TopLevelRoute.Transactions.route) {
                popUpTo(navController.graph.findStartDestination().id) { saveState = true }
                launchSingleTop = true
                restoreState = true
            }
            aiFeaturesGate.runWhenEnabled {
                scope.launch { uris.forEach { prepareAndQueue(it) } }
            }
        }
    }

    fun navigateToDetail(
        transaction: TransactionListItem,
        isInGroup: Boolean = false,
    ) {
        transactionCache[transaction.id] =
            TransactionDetailState(
                transaction = transaction,
                isInGroup = isInGroup,
            )
        navController.navigate("transactionDetail/${transaction.id}")
    }

    fun selectTab(tab: TopLevelRoute) {
        if (tab == currentTab) {
            navController.popBackStack(tab.route, inclusive = false)
        } else {
            navController.navigate(tab.route) {
                popUpTo(navController.graph.findStartDestination().id) { saveState = true }
                launchSingleTop = true
                restoreState = true
            }
        }
    }

    fun replaceDetail(route: String) {
        navController.navigate(route) { popUpTo(currentTab.route) }
    }

    fun openFromListPane(transaction: TransactionListItem) {
        transactionCache[transaction.id] = TransactionDetailState(transaction = transaction, isInGroup = false)
        replaceDetail("transactionDetail/${transaction.id}")
    }

    val onQuickUpload: () -> Unit = { aiFeaturesGate.runWhenEnabled { showQuickUploadSheet = true } }
    val onQuickUploadRetry: (String) -> Unit = { id -> aiFeaturesGate.runWhenEnabled { quickUploadViewModel?.retry(id) } }
    val onQuickUploadDismiss: (String) -> Unit = { id -> quickUploadViewModel?.dismiss(id) }
    val onRefreshQuickUploads: () -> Unit = { quickUploadViewModel?.refresh() }

    LaunchedEffect(listPaneTab) {
        if (listPaneTab == TopLevelRoute.AiChat) drawerState.snapTo(DrawerValue.Closed)
    }

    val connectionStatus = remember { mutableStateOf(ConnectionStatus.ONLINE) }

    DisposableEffect(Unit) {
        val observer =
            object : uniffi.sverto_core.ConnectionObserver {
                override fun onConnectionStatusChanged(status: ConnectionStatus) {
                    connectionStatus.value = status
                }
            }
        appStore.observeConnection(observer)
        onDispose {
            appStore.unobserveConnection()
        }
    }

    val bannerAbovePanes = shellLayout != ShellLayout.Compact && connectionStatus.value != ConnectionStatus.ONLINE
    val bannerTopInsets = WindowInsets.safeDrawing.only(WindowInsetsSides.Top)

    SharedTransitionLayout {
        val sharedScope = this

        ModalNavigationDrawer(
            drawerState = drawerState,
            // Only interactive on the AI Chat tab: enables scrim tap-to-close and edge-swipe
            // there, while keeping the drawer inert (and unreachable) on the other tabs.
            gesturesEnabled = isAiChatTab && listPaneTab == null,
            drawerContent = {
                // Always compose the drawer so its width stays constant (304.dp). If the
                // content collapsed to zero width on non-chat tabs, ModalNavigationDrawer
                // would recompute its anchors when the chat content appears and spring open.
                // The drawer is only reachable from the AI Chat tab (see the hamburger below).
                val chatViewModel = aiChatViewModel
                if (chatViewModel == null) {
                    ConversationDrawer(emptyList(), null, {}, {}, {})
                } else {
                    val aiChatState by chatViewModel.uiState.collectAsStateWithLifecycle()
                    ConversationDrawer(
                        conversations = aiChatState.conversations,
                        activeConversationId = aiChatState.activeConversationId,
                        onSelect = { id ->
                            chatViewModel.selectConversation(id)
                            scope.launch { drawerState.close() }
                        },
                        onCreate = {
                            chatViewModel.startNewConversation()
                            scope.launch { drawerState.close() }
                        },
                        onDelete = chatViewModel::deleteConversation,
                    )
                }
            },
        ) {
            Column(Modifier.fillMaxSize()) {
                if (shellLayout != ShellLayout.Compact) {
                    if (bannerAbovePanes) {
                        Spacer(
                            Modifier
                                .fillMaxWidth()
                                .background(MaterialTheme.colorScheme.surfaceContainer)
                                .windowInsetsTopHeight(bannerTopInsets),
                        )
                    }
                    OfflineBanner(status = connectionStatus.value)
                }
                Row(
                    Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .then(if (bannerAbovePanes) Modifier.consumeWindowInsets(bannerTopInsets) else Modifier),
                ) {
                    if (shellLayout != ShellLayout.Compact) {
                        ShellRail(selected = currentTab, onSelect = ::selectTab)
                    }
                    if (listPaneTab != null) {
                        ListPane(
                            rootEntry = remember(navBackStackEntry) { navController.getBackStackEntry(listPaneTab.route) },
                            width = listPaneWidth(),
                            topBar = {
                                ShellTopBar(
                                    route = listPaneTab.route,
                                    showMenu = false,
                                    onMenu = {},
                                    onAssetClick = {},
                                    transactionSearchBarState = transactionSearchBarState,
                                    transactionSearchTextState = transactionSearchTextState,
                                    transactionSearchListState = transactionSearchListState,
                                    onTransactionClick = ::openFromListPane,
                                    onProfile = { replaceDetail("settings") },
                                    showUpdateBadge = showUpdateBadge,
                                )
                            },
                        ) { padding ->
                            ListPaneContent(
                                tab = listPaneTab,
                                rightPaneEntryId = navBackStackEntry?.id,
                                openId = listPaneOpenId,
                                transactionsViewModel = transactionsViewModel,
                                transactionsListState = transactionsListState,
                                accountsScrollState = accountsScrollState,
                                quickUploadItems = quickUploadItems,
                                onQuickUpload = onQuickUpload,
                                onQuickUploadRetry = onQuickUploadRetry,
                                onQuickUploadDismiss = onQuickUploadDismiss,
                                onRefreshQuickUploads = onRefreshQuickUploads,
                                pendingGroupSelection = pendingGroupSelection,
                                aiChatViewModel = aiChatViewModel,
                                replace = ::replaceDetail,
                                onTransactionClick = ::openFromListPane,
                                onConversationOpened = {
                                    navController.popBackStack(TopLevelRoute.AiChat.route, inclusive = false)
                                },
                                modifier =
                                    Modifier
                                        .fillMaxSize()
                                        .padding(padding)
                                        .consumeWindowInsets(padding),
                            )
                        }
                    }
                    Scaffold(
                        modifier =
                            Modifier
                                .weight(1f)
                                .fillMaxHeight()
                                .then(
                                    if (shellLayout == ShellLayout.Compact) {
                                        Modifier
                                    } else {
                                        Modifier.consumeWindowInsets(railInsets().only(WindowInsetsSides.Start))
                                    },
                                ),
                        containerColor = MaterialTheme.colorScheme.surfaceContainer,
                        topBar = {
                            Column {
                                AnimatedVisibility(
                                    visible = isTopLevel && listPaneTab == null,
                                    enter = slideInVertically(initialOffsetY = { -it }) + fadeIn(),
                                    exit = slideOutVertically(targetOffsetY = { -it }) + fadeOut(),
                                ) {
                                    ShellTopBar(
                                        route = currentRoute,
                                        showMenu = isAiChatTab,
                                        onMenu = { scope.launch { drawerState.open() } },
                                        onAssetClick = { id -> navController.navigate("assetOverview/$id?userAsset=false") },
                                        transactionSearchBarState = transactionSearchBarState,
                                        transactionSearchTextState = transactionSearchTextState,
                                        transactionSearchListState = transactionSearchListState,
                                        onTransactionClick = { transaction -> navigateToDetail(transaction, false) },
                                        onProfile = { navController.navigate("settings") },
                                        showUpdateBadge = showUpdateBadge,
                                    )
                                }
                                if (shellLayout == ShellLayout.Compact) {
                                    OfflineBanner(status = connectionStatus.value)
                                }
                            }
                        },
                        bottomBar = {
                            AnimatedVisibility(
                                visible = isTopLevel && !transactionSelectionActive && shellLayout == ShellLayout.Compact,
                                enter = slideInVertically(initialOffsetY = { it }) + fadeIn(),
                                exit = slideOutVertically(targetOffsetY = { it }) + fadeOut(),
                            ) {
                                ShellBottomBar(
                                    currentRoute = currentRoute,
                                    onSelect = { route ->
                                        navController.navigate(route.route) {
                                            popUpTo(navController.graph.findStartDestination().id) {
                                                saveState = true
                                            }
                                            launchSingleTop = true
                                            restoreState = true
                                        }
                                    },
                                )
                            }
                        },
                    ) { innerPadding ->
                        CappedPane(
                            maxWidth = paneMaxWidth(currentRoute),
                            backdrop = MaterialTheme.colorScheme.surfaceContainer,
                        ) {
                            MainNavGraph(
                                navController = navController,
                                innerPadding = innerPadding,
                                sharedScope = sharedScope,
                                listPaneTab = listPaneTab,
                                transactionsViewModel = transactionsViewModel,
                                transactionsListState = transactionsListState,
                                accountsScrollState = accountsScrollState,
                                quickUploadItems = quickUploadItems,
                                onQuickUpload = onQuickUpload,
                                onQuickUploadRetry = onQuickUploadRetry,
                                onQuickUploadDismiss = onQuickUploadDismiss,
                                onRefreshQuickUploads = onRefreshQuickUploads,
                                onProposalCompleted = { quickUploadViewModel?.onProposalCompleted(it) },
                                transactionCache = transactionCache,
                                pendingGroupTransaction = pendingGroupTransaction,
                                editingGroupIndex = editingGroupIndex,
                                pendingGroupSelection = pendingGroupSelection,
                                onNavigateToDetail = ::navigateToDetail,
                                aiChatViewModel = aiChatViewModel,
                                aiFeaturesGate = aiFeaturesGate,
                                onEdit = { id -> navController.navigate("editAccount/$id") },
                                sessionViewModel = sessionViewModel,
                            )
                        }
                    }
                }
            }
        }
    } // ModalNavigationDrawer

    AiFeaturesConsentDialog(gate = aiFeaturesGate)
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class, ExperimentalSharedTransitionApi::class)
@Composable
@Suppress(
    "LongMethod",
    "CyclomaticComplexMethod",
    "MutableParams",
    "MutableStateParam",
    "ViewModelForwarding",
    "ParameterNaming",
)
private fun MainNavGraph(
    navController: NavHostController,
    innerPadding: PaddingValues,
    sharedScope: SharedTransitionScope,
    listPaneTab: TopLevelRoute?,
    transactionsViewModel: TransactionsViewModel?,
    transactionsListState: LazyListState,
    accountsScrollState: ScrollState,
    quickUploadItems: List<QuickUploadUiItem>,
    onQuickUpload: () -> Unit,
    onQuickUploadRetry: (String) -> Unit,
    onQuickUploadDismiss: (String) -> Unit,
    onRefreshQuickUploads: () -> Unit,
    onProposalCompleted: (String) -> Unit,
    transactionCache: MutableMap<String, TransactionDetailState>,
    pendingGroupTransaction: MutableState<GroupTransactionItem?>,
    editingGroupIndex: MutableState<Int?>,
    pendingGroupSelection: MutableState<List<String>>,
    onNavigateToDetail: (TransactionListItem, Boolean) -> Unit,
    aiChatViewModel: AiChatViewModel?,
    aiFeaturesGate: AiFeaturesGate,
    onEdit: (String) -> Unit,
    sessionViewModel: AppSessionViewModel?,
) {
    val fadeSpec = MaterialTheme.motionScheme.defaultEffectsSpec<Float>()
    val slideSpec = MaterialTheme.motionScheme.defaultSpatialSpec<IntOffset>()
    NavHost(
        navController = navController,
        startDestination = TopLevelRoute.Portfolio.route,
        enterTransition = { fadeIn(animationSpec = fadeSpec) },
        exitTransition = { fadeOut(animationSpec = fadeSpec) },
        modifier = Modifier.fillMaxSize(),
    ) {
        composable(TopLevelRoute.Portfolio.route) {
            PortfolioScreen(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .padding(innerPadding)
                        .consumeWindowInsets(innerPadding),
                onHoldingClick = { holding ->
                    val property = holding.property
                    when {
                        property != null -> navController.navigate("property/${property.propertyId}/${holding.assetId}")
                        isCurrencyAssetType(holding.assetTypeId) -> navController.navigate("currency/${holding.assetId}")
                        else -> navController.navigate("assetOverview/${holding.assetId}?userAsset=false")
                    }
                },
                onLiabilityClick = { liability ->
                    navController.navigate(
                        "accountDetail/${liability.accountId}/${Uri.encode(liability.accountName)}/${liability.accountTypeId}",
                    )
                },
            )
        }
        composable(TopLevelRoute.Transactions.route) {
            val viewModel = transactionsViewModel ?: return@composable
            if (listPaneTab == TopLevelRoute.Transactions) {
                PanePlaceholder("Select a transaction")
            } else {
                TransactionsTabRoot(
                    viewModel = viewModel,
                    listState = transactionsListState,
                    quickUploadItems = quickUploadItems,
                    onQuickUpload = onQuickUpload,
                    onQuickUploadRetry = onQuickUploadRetry,
                    onQuickUploadDismiss = onQuickUploadDismiss,
                    onRefreshQuickUploads = onRefreshQuickUploads,
                    pendingGroupSelection = pendingGroupSelection,
                    navigate = { route -> navController.navigate(route) },
                    onTransactionClick = { onNavigateToDetail(it, false) },
                    openTransactionId = null,
                    sharedTransitionScope = sharedScope,
                    animatedVisibilityScope = this@composable,
                    modifier =
                        Modifier
                            .fillMaxSize()
                            .padding(innerPadding)
                            .consumeWindowInsets(innerPadding),
                )
            }
        }
        composable(TopLevelRoute.Accounts.route) {
            if (listPaneTab == TopLevelRoute.Accounts) {
                PanePlaceholder("Select an account")
            } else {
                AccountsTabRoot(
                    scrollState = accountsScrollState,
                    navigate = { route -> navController.navigate(route) },
                    openId = null,
                    sharedTransitionScope = sharedScope,
                    animatedVisibilityScope = this@composable,
                    modifier =
                        Modifier
                            .fillMaxSize()
                            .padding(innerPadding)
                            .consumeWindowInsets(innerPadding),
                )
            }
        }
        composable(TopLevelRoute.AiChat.route) {
            val viewModel = aiChatViewModel ?: return@composable
            AiChatScreen(
                viewModel = viewModel,
                aiFeaturesGate = aiFeaturesGate,
                modifier =
                    Modifier
                        .fillMaxSize()
                        .padding(innerPadding)
                        .consumeWindowInsets(innerPadding),
            )
        }
        composable(ADD_ACCOUNT_ROUTE) {
            AddAccountScreen(
                onBack = { navController.popBackStack() },
                onSuccess = { navController.popBackStack() },
            )
        }
        composable(
            route = EDIT_ACCOUNT_ROUTE,
            arguments = listOf(navArgument("accountId") { type = NavType.StringType }),
        ) { backStackEntry ->
            val accountId =
                backStackEntry.arguments?.getString("accountId") ?: return@composable
            AddAccountScreen(
                onBack = { navController.popBackStack() },
                onSuccess = { navController.popBackStack() },
                accountId = accountId,
            )
        }
        composable(ADD_PROPERTY_ROUTE) {
            AddPropertyScreen(
                onBack = { navController.popBackStack() },
                onSuccess = { navController.popBackStack() },
            )
        }
        composable(
            route = PROPERTY_DETAIL_ROUTE,
            arguments =
                listOf(
                    navArgument("propertyId") { type = NavType.StringType },
                    navArgument("assetId") { type = NavType.IntType },
                ),
        ) { backStackEntry ->
            val propertyId = backStackEntry.arguments?.getString("propertyId") ?: return@composable
            val assetId = backStackEntry.arguments?.getInt("assetId") ?: return@composable
            PropertyDetailScreen(
                propertyId = propertyId,
                assetId = assetId,
                showBack = showsBack(navController, backStackEntry, listPaneTab),
                onBack = { navController.popBackStack() },
                onViewLoan = { loan ->
                    navController.navigate(
                        "accountDetail/${loan.id}/${Uri.encode(loan.name)}/${loan.accountTypeId}",
                    )
                },
                onOpenValuations = { valuedAssetId, currencyAssetId ->
                    navController.navigate("propertyValuations/$valuedAssetId/$currencyAssetId")
                },
                modifier = Modifier.fillMaxSize(),
            )
        }
        composable(
            route = CURRENCY_DETAIL_ROUTE,
            arguments =
                listOf(
                    navArgument("assetId") { type = NavType.IntType },
                    navArgument("accountId") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                ),
        ) { backStackEntry ->
            val assetId = backStackEntry.arguments?.getInt("assetId") ?: return@composable
            val accountId = backStackEntry.arguments?.getString("accountId")
            CurrencyDetailScreen(
                assetId = assetId,
                accountId = accountId,
                onBack = { navController.popBackStack() },
                onOpenRate = { navController.navigate("assetOverview/$assetId?userAsset=false") },
                onAccountClick = { account ->
                    navController.navigate(
                        "accountDetail/${account.accountId}/${Uri.encode(account.accountName)}/${account.accountTypeId}",
                    )
                },
                onViewAllTransactions = { ticker, accountName ->
                    if (accountId == null) {
                        navController.navigate("currencyTransactions/$assetId?title=${Uri.encode(ticker)}")
                    } else {
                        navController.navigate(assetTransactionsRoute(assetId, ticker, accountId, accountName))
                    }
                },
                onTransactionClick = { tx -> onNavigateToDetail(tx, false) },
                sharedTransitionScope = sharedScope,
                animatedVisibilityScope = this@composable,
                modifier = Modifier.fillMaxSize(),
            )
        }
        composable(
            route = CURRENCY_TRANSACTIONS_ROUTE,
            arguments =
                listOf(
                    navArgument("assetId") { type = NavType.IntType },
                    navArgument("title") {
                        type = NavType.StringType
                        defaultValue = "Transactions"
                    },
                ),
        ) { backStackEntry ->
            val assetId = backStackEntry.arguments?.getInt("assetId") ?: return@composable
            val title = backStackEntry.arguments?.getString("title") ?: "Transactions"
            ScopedTransactionsScreen(
                scope = TransactionListScope.Currency(assetId),
                title = title,
                onBack = { navController.popBackStack() },
                onTransactionClick = { tx -> onNavigateToDetail(tx, false) },
                sharedTransitionScope = sharedScope,
                animatedVisibilityScope = this@composable,
                modifier = Modifier.fillMaxSize(),
            )
        }
        composable(
            route = PROPERTY_VALUATIONS_ROUTE,
            arguments =
                listOf(
                    navArgument("assetId") { type = NavType.IntType },
                    navArgument("currencyAssetId") { type = NavType.IntType },
                ),
        ) { backStackEntry ->
            val assetId = backStackEntry.arguments?.getInt("assetId") ?: return@composable
            val currencyAssetId = backStackEntry.arguments?.getInt("currencyAssetId") ?: return@composable
            PropertyValuationsScreen(
                assetId = assetId,
                currencyAssetId = currencyAssetId,
                onBack = { navController.popBackStack() },
                modifier = Modifier.fillMaxSize(),
            )
        }
        composable(
            route = TRANSACTION_DETAIL_ROUTE,
            arguments = listOf(navArgument("txId") { type = NavType.StringType }),
        ) { backStackEntry ->
            val txId = backStackEntry.arguments?.getString("txId")
            val detailState = txId?.let { transactionCache[it] }
            if (detailState != null) {
                TransactionDetailScreen(
                    transaction = detailState.transaction,
                    isInGroup = detailState.isInGroup,
                    showBack = showsBack(navController, backStackEntry, listPaneTab),
                    onBack = { navController.popBackStack() },
                    onEdit = {
                        if (detailState.transaction.isGroup) {
                            navController.navigate(
                                "editTransactionGroup/${detailState.transaction.id}",
                            )
                        } else {
                            val configKey =
                                apiTypeToConfigKey(detailState.transaction.transactionType)
                            navController.navigate(
                                "editTransaction/$configKey/${detailState.transaction.id}",
                            )
                        }
                    },
                    onDelete = {
                        val deletedId = detailState.transaction.id
                        if (detailState.transaction.isGroup) {
                            transactionsViewModel?.deleteTransactionGroup(deletedId) {
                                transactionCache.remove(deletedId)
                            }
                        } else {
                            transactionsViewModel?.deleteTransaction(deletedId) {
                                transactionCache.remove(deletedId)
                            }
                        }
                        navController.popBackStack()
                    },
                    onMarkReviewed = {
                        transactionsViewModel?.markReviewed(detailState.transaction.id)
                    },
                    onChildClick = { child -> onNavigateToDetail(child, true) },
                    sharedTransitionScope = sharedScope,
                    animatedVisibilityScope = this@composable,
                )
            }
        }
        composable(
            route = "createTransaction/{typeKey}?quickUploadId={quickUploadId}",
            arguments =
                listOf(
                    navArgument("typeKey") { type = NavType.StringType },
                    navArgument("quickUploadId") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                ),
        ) { backStackEntry ->
            val typeKey = backStackEntry.arguments?.getString("typeKey") ?: return@composable
            val quickUploadId = backStackEntry.arguments?.getString("quickUploadId")
            CreateTransactionScreen(
                typeKey = typeKey,
                quickUploadId = quickUploadId,
                onDiscard = { navController.popBackStack() },
                aiFeaturesGate = aiFeaturesGate,
                onSuccess = { _ ->
                    transactionsViewModel?.refresh()
                    if (quickUploadId != null) {
                        onProposalCompleted(quickUploadId)
                    }
                    navController.popBackStack()
                },
                onCorrectionTypeChanged = { change ->
                    navController.popBackStack()
                    val route =
                        if (change.newProposalType == "transaction_group") {
                            "createTransactionGroup?quickUploadId=${change.quickUploadId}"
                        } else {
                            "createTransaction/${change.newProposalType}?quickUploadId=${change.quickUploadId}"
                        }
                    navController.navigate(route)
                },
            )
        }
        composable(
            route = EDIT_TRANSACTION_ROUTE,
            arguments =
                listOf(
                    navArgument("typeKey") { type = NavType.StringType },
                    navArgument("txId") { type = NavType.StringType },
                ),
        ) { backStackEntry ->
            val typeKey = backStackEntry.arguments?.getString("typeKey") ?: return@composable
            val txId = backStackEntry.arguments?.getString("txId") ?: return@composable
            CreateTransactionScreen(
                typeKey = typeKey,
                editTransactionId = txId,
                onDiscard = { navController.popBackStack() },
                aiFeaturesGate = aiFeaturesGate,
                onSuccess = { updatedTransaction ->
                    updatedTransaction?.let { updated ->
                        val existingState = transactionCache[txId]
                        transactionCache[txId] =
                            TransactionDetailState(
                                transaction = updated,
                                isInGroup = existingState?.isInGroup ?: false,
                            )
                    }
                    transactionsViewModel?.refresh()
                    navController.popBackStack()
                },
            )
        }
        composable(
            route = "createTransactionGroup?quickUploadId={quickUploadId}&fromSelection={fromSelection}",
            arguments =
                listOf(
                    navArgument("quickUploadId") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                    navArgument("fromSelection") {
                        type = NavType.BoolType
                        defaultValue = false
                    },
                ),
        ) { backStackEntry ->
            val quickUploadId = backStackEntry.arguments?.getString("quickUploadId")
            val fromSelection = backStackEntry.arguments?.getBoolean("fromSelection") ?: false
            val pending = pendingGroupTransaction.value
            val vm: CreateTransactionGroupViewModel = viewModel(factory = SvertoViewModelFactory)

            LaunchedEffect(quickUploadId) {
                if (quickUploadId != null) {
                    vm.initFromProposal(quickUploadId)
                }
            }

            LaunchedEffect(fromSelection) {
                if (fromSelection) {
                    val ids = pendingGroupSelection.value
                    if (ids.isNotEmpty()) {
                        vm.initFromSelection(ids)
                        pendingGroupSelection.value = emptyList()
                    }
                }
            }

            LaunchedEffect(pending) {
                if (pending != null) {
                    val idx = editingGroupIndex.value
                    if (idx != null) {
                        vm.updateTransaction(idx, pending)
                        editingGroupIndex.value = null
                    } else {
                        vm.addTransaction(pending)
                    }
                    pendingGroupTransaction.value = null
                }
            }

            CreateTransactionGroupScreen(
                quickUploadId = quickUploadId,
                fromSelection = fromSelection,
                onDiscard = { navController.popBackStack() },
                aiFeaturesGate = aiFeaturesGate,
                onSuccess = {
                    transactionsViewModel?.clearSelection()
                    transactionsViewModel?.refresh()
                    if (quickUploadId != null) {
                        onProposalCompleted(quickUploadId)
                    }
                    navController.popBackStack()
                },
                onAddTransaction = { typeKey ->
                    navController.navigate("groupAddTransaction/$typeKey")
                },
                onEditTransaction = { index, typeKey ->
                    editingGroupIndex.value = index
                    navController.navigate("groupEditTransaction/$typeKey/$index")
                },
                onCorrectionTypeChanged = { change ->
                    navController.popBackStack()
                    val route =
                        if (change.newProposalType == "transaction_group") {
                            "createTransactionGroup?quickUploadId=${change.quickUploadId}"
                        } else {
                            "createTransaction/regular_transaction?quickUploadId=${change.quickUploadId}"
                        }
                    navController.navigate(route)
                },
                viewModel = vm,
            )
        }
        composable(
            route = EDIT_GROUP_ROUTE,
            arguments = listOf(navArgument("groupId") { type = NavType.StringType }),
        ) { backStackEntry ->
            val groupId = backStackEntry.arguments?.getString("groupId") ?: return@composable
            val group = transactionCache[groupId]?.transaction
            val pending = pendingGroupTransaction.value
            val vm: CreateTransactionGroupViewModel = viewModel(factory = SvertoViewModelFactory)

            LaunchedEffect(pending) {
                if (pending != null) {
                    val idx = editingGroupIndex.value
                    if (idx != null) {
                        vm.updateTransaction(idx, pending)
                        editingGroupIndex.value = null
                    } else {
                        vm.addTransaction(pending)
                    }
                    pendingGroupTransaction.value = null
                }
            }

            CreateTransactionGroupScreen(
                editGroupId = groupId,
                editGroup = group,
                onDiscard = { navController.popBackStack() },
                aiFeaturesGate = aiFeaturesGate,
                onSuccess = {
                    transactionCache.remove(groupId)
                    transactionsViewModel?.refresh()
                    navController.popBackStack()
                },
                onAddTransaction = { typeKey ->
                    navController.navigate("groupAddTransaction/$typeKey")
                },
                onEditTransaction = { index, typeKey ->
                    editingGroupIndex.value = index
                    navController.navigate("groupEditTransaction/$typeKey/$index")
                },
                viewModel = vm,
            )
        }
        composable(
            route = GROUP_ADD_TXN_ROUTE,
            arguments = listOf(navArgument("typeKey") { type = NavType.StringType }),
        ) { backStackEntry ->
            val typeKey = backStackEntry.arguments?.getString("typeKey") ?: return@composable
            CreateTransactionScreen(
                typeKey = typeKey,
                isGroupMode = true,
                onDiscard = { navController.popBackStack() },
                aiFeaturesGate = aiFeaturesGate,
                onSuccess = { navController.popBackStack() },
                onGroupTransactionReady = { item ->
                    pendingGroupTransaction.value = item
                },
            )
        }
        composable(
            route = GROUP_EDIT_TXN_ROUTE,
            arguments =
                listOf(
                    navArgument("typeKey") { type = NavType.StringType },
                    navArgument("index") { type = NavType.IntType },
                ),
        ) { backStackEntry ->
            val typeKey = backStackEntry.arguments?.getString("typeKey") ?: return@composable
            val index = backStackEntry.arguments?.getInt("index") ?: return@composable

            val parentEntry = remember(backStackEntry) { navController.previousBackStackEntry }
            val groupVm: CreateTransactionGroupViewModel? =
                parentEntry?.let {
                    viewModel(it, factory = SvertoViewModelFactory)
                }
            val item =
                remember {
                    groupVm
                        ?.formState
                        ?.value
                        ?.transactions
                        ?.getOrNull(index)
                }

            if (item != null) {
                val vm: CreateTransactionViewModel = viewModel(factory = SvertoViewModelFactory)
                vm.initForGroupEdit(
                    item.input,
                    GroupEditDisplayData(
                        categoryName = item.categoryName,
                        primaryAccountName = item.accountName,
                        primaryAssetDisplay = item.assetDisplay,
                    ),
                )

                CreateTransactionScreen(
                    typeKey = typeKey,
                    isGroupMode = true,
                    editTransactionId = item.input.transactionId,
                    onDiscard = { navController.popBackStack() },
                    aiFeaturesGate = aiFeaturesGate,
                    onSuccess = { navController.popBackStack() },
                    viewModel = vm,
                    onGroupTransactionReady = { pendingGroupTransaction.value = it },
                )
            } else {
                LaunchedEffect(Unit) { navController.popBackStack() }
            }
        }
        composable(
            route = ACCOUNT_DETAIL_ROUTE,
            arguments =
                listOf(
                    navArgument("accountId") { type = NavType.StringType },
                    navArgument("accountName") { type = NavType.StringType },
                    navArgument("accountTypeId") { type = NavType.IntType },
                ),
        ) { backStackEntry ->
            val accountId = backStackEntry.arguments?.getString("accountId") ?: return@composable
            val accountName = backStackEntry.arguments?.getString("accountName") ?: return@composable
            val accountTypeId = backStackEntry.arguments?.getInt("accountTypeId") ?: return@composable
            AccountDetailScreen(
                accountId = accountId,
                accountName = accountName,
                accountTypeId = accountTypeId,
                showBack = showsBack(navController, backStackEntry, listPaneTab),
                onBack = { navController.popBackStack() },
                onHoldingClick = { _, assetId ->
                    navController.navigate("assetDetail/$accountId/$assetId?accountName=${Uri.encode(accountName)}")
                },
                onBalanceClick = { assetId ->
                    navController.navigate("currency/$assetId?accountId=${Uri.encode(accountId)}")
                },
                onViewAllTransactions = {
                    navController.navigate("accountTransactions/$accountId")
                },
                onTransactionClick = { tx -> onNavigateToDetail(tx, false) },
                onEdit = onEdit,
                sharedTransitionScope = sharedScope,
                animatedVisibilityScope = this@composable,
                modifier = Modifier.fillMaxSize(),
            )
        }
        composable(
            route = ASSET_DETAIL_ROUTE,
            arguments =
                listOf(
                    navArgument("accountId") { type = NavType.StringType },
                    navArgument("assetId") { type = NavType.IntType },
                    navArgument("accountName") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                ),
        ) { backStackEntry ->
            val accountId = backStackEntry.arguments?.getString("accountId") ?: return@composable
            val assetId = backStackEntry.arguments?.getInt("assetId") ?: return@composable
            val accountName = backStackEntry.arguments?.getString("accountName")
            AssetDetailScreen(
                accountId = accountId,
                assetId = assetId,
                onBack = { navController.popBackStack() },
                onViewTransactions = { title ->
                    navController.navigate(assetTransactionsRoute(assetId, title, accountId, accountName))
                },
                modifier = Modifier.fillMaxSize(),
            )
        }
        composable(
            route = ACCOUNT_TRANSACTIONS_ROUTE,
            arguments = listOf(navArgument("accountId") { type = NavType.StringType }),
        ) { backStackEntry ->
            val accountId = backStackEntry.arguments?.getString("accountId") ?: return@composable
            ScopedTransactionsScreen(
                scope = TransactionListScope.Account(accountId),
                title = "Transactions",
                onBack = { navController.popBackStack() },
                onTransactionClick = { tx -> onNavigateToDetail(tx, false) },
                sharedTransitionScope = sharedScope,
                animatedVisibilityScope = this@composable,
                modifier = Modifier.fillMaxSize(),
            )
        }
        composable(
            route = ASSET_TRANSACTIONS_ROUTE,
            arguments =
                listOf(
                    navArgument("assetId") { type = NavType.IntType },
                    navArgument("accountId") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                    navArgument("accountName") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                    navArgument("title") {
                        type = NavType.StringType
                        defaultValue = "Transactions"
                    },
                ),
        ) { backStackEntry ->
            val assetId = backStackEntry.arguments?.getInt("assetId") ?: return@composable
            val accountId = backStackEntry.arguments?.getString("accountId")
            val accountName = backStackEntry.arguments?.getString("accountName")
            val title = backStackEntry.arguments?.getString("title") ?: "Transactions"
            ScopedTransactionsScreen(
                scope = TransactionListScope.Asset(assetId, accountId),
                title = title,
                subtitle = accountName,
                onBack = { navController.popBackStack() },
                onTransactionClick = { tx -> onNavigateToDetail(tx, false) },
                sharedTransitionScope = sharedScope,
                animatedVisibilityScope = this@composable,
                modifier = Modifier.fillMaxSize(),
            )
        }
        composable(
            route = "settings",
            enterTransition = {
                slideIntoContainer(
                    AnimatedContentTransitionScope.SlideDirection.Start,
                    animationSpec = slideSpec,
                )
            },
            exitTransition = {
                slideOutOfContainer(
                    AnimatedContentTransitionScope.SlideDirection.Start,
                    animationSpec = slideSpec,
                )
            },
            popEnterTransition = {
                slideIntoContainer(
                    AnimatedContentTransitionScope.SlideDirection.End,
                    animationSpec = slideSpec,
                )
            },
            popExitTransition = {
                slideOutOfContainer(
                    AnimatedContentTransitionScope.SlideDirection.End,
                    animationSpec = slideSpec,
                )
            },
        ) {
            SettingsScreen(
                onBack = { navController.popBackStack() },
                onCustomCategories = { navController.navigate("customCategories") },
                onCustomAssets = { navController.navigate("customAssets") },
                onConnectors = { navController.navigate(CONNECTORS_ROUTE) },
                onImports = { navController.navigate(IMPORTS_ROUTE) },
                aiFeaturesGate = aiFeaturesGate,
                sessionViewModel = sessionViewModel,
            )
        }
        composable(
            route = "customCategories",
            enterTransition = {
                slideIntoContainer(
                    AnimatedContentTransitionScope.SlideDirection.Start,
                    animationSpec = slideSpec,
                )
            },
            exitTransition = {
                slideOutOfContainer(
                    AnimatedContentTransitionScope.SlideDirection.Start,
                    animationSpec = slideSpec,
                )
            },
            popEnterTransition = {
                slideIntoContainer(
                    AnimatedContentTransitionScope.SlideDirection.End,
                    animationSpec = slideSpec,
                )
            },
            popExitTransition = {
                slideOutOfContainer(
                    AnimatedContentTransitionScope.SlideDirection.End,
                    animationSpec = slideSpec,
                )
            },
        ) {
            CustomCategoriesScreen(onBack = { navController.popBackStack() })
        }
        slideComposable(IMPORTS_ROUTE, slideSpec) {
            ImportsScreen(
                aiFeaturesGate = aiFeaturesGate,
                onBack = { navController.popBackStack() },
                onOpenSession = { id -> navController.navigate("importSession/$id") },
                onSessionCreated = { id -> navController.navigate("importSession/$id?pickFiles=true") },
            )
        }
        slideComposable(
            IMPORT_SESSION_ROUTE,
            slideSpec,
            arguments =
                listOf(
                    navArgument("sessionId") { type = NavType.StringType },
                    navArgument("pickFiles") {
                        type = NavType.BoolType
                        defaultValue = false
                    },
                ),
        ) { entry ->
            val sessionId = entry.arguments?.getString("sessionId") ?: return@slideComposable
            ImportSessionScreen(
                sessionId = sessionId,
                aiFeaturesGate = aiFeaturesGate,
                onBack = { navController.popBackStack() },
                onOpenItems = { filter -> navController.navigate("importItems/$sessionId?filter=$filter") },
                onCreateAccount = { navController.navigate(ADD_ACCOUNT_ROUTE) },
                onViewTransactions = {
                    if (!navController.popBackStack(TopLevelRoute.Transactions.route, inclusive = false)) {
                        navController.navigate(TopLevelRoute.Transactions.route) {
                            popUpTo(navController.graph.findStartDestination().id)
                            launchSingleTop = true
                        }
                    }
                },
                pickFilesOnOpen = entry.arguments?.getBoolean("pickFiles") == true,
            )
        }
        slideComposable(
            IMPORT_ITEMS_ROUTE,
            slideSpec,
            arguments =
                listOf(
                    navArgument("sessionId") { type = NavType.StringType },
                    navArgument("filter") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                ),
        ) { entry ->
            val sessionId = entry.arguments?.getString("sessionId") ?: return@slideComposable
            ImportItemsScreen(
                sessionId = sessionId,
                initialFilter = entry.arguments?.getString("filter"),
                onBack = { navController.popBackStack() },
            )
        }
        slideComposable(CONNECTORS_ROUTE, slideSpec) {
            ConnectorsScreen(
                onBack = { navController.popBackStack() },
                onConnectorClick = { kind -> navController.navigate("connectorDetail/$kind") },
            )
        }
        slideComposable(
            CONNECTOR_DETAIL_ROUTE,
            slideSpec,
            arguments = listOf(navArgument("providerKind") { type = NavType.StringType }),
        ) { entry ->
            val kind = entry.arguments?.getString("providerKind") ?: return@slideComposable
            ConnectorDetailScreen(
                providerKind = kind,
                onBack = { navController.popBackStack() },
                onConnectionClick = { id -> navController.navigate("connectionDetail/$id") },
                onConnect = {
                    when (kind) {
                        "truelayer" -> navController.navigate("connectTrueLayer")
                        "enablebanking" -> navController.navigate("connectEnableBanking")
                        else -> navController.navigate("connectTrading212")
                    }
                },
            )
        }
        slideComposable(
            CONNECTION_DETAIL_ROUTE,
            slideSpec,
            arguments = listOf(navArgument("connectionId") { type = NavType.StringType }),
        ) { entry ->
            val connectionId = entry.arguments?.getString("connectionId") ?: return@slideComposable
            ConnectionDetailScreen(
                connectionId = connectionId,
                onBack = { navController.popBackStack() },
                onAddBinding = { navController.navigate("bindingSetup/$connectionId") },
                onRevoked = { navController.popBackStack(CONNECTORS_ROUTE, inclusive = false) },
                onOpenAccount = { providerAccountId, displayName ->
                    navController.navigate(
                        "providerAccountDetail/$connectionId/" +
                            Uri.encode(providerAccountId) +
                            "?name=${Uri.encode(displayName)}",
                    )
                },
                onOpenImport = { sessionId -> navController.navigate("importSession/$sessionId") },
            )
        }
        slideComposable(
            BINDING_SETUP_ROUTE,
            slideSpec,
            arguments = listOf(navArgument("connectionId") { type = NavType.StringType }),
        ) { entry ->
            val connectionId = entry.arguments?.getString("connectionId") ?: return@slideComposable
            BindingSetupScreen(
                connectionId = connectionId,
                onBack = { navController.popBackStack() },
                onDone = { navController.popBackStack() },
                onOpenAccount = { account ->
                    navController.navigate(
                        "providerAccountDetail/$connectionId/" +
                            Uri.encode(account.providerAccountId) +
                            "?name=${Uri.encode(account.displayName)}" +
                            "&currency=${Uri.encode(account.currency ?: "")}",
                    )
                },
                onImported = { sessionId ->
                    navController.navigate("importSession/$sessionId") {
                        popUpTo(BINDING_SETUP_ROUTE) { inclusive = true }
                    }
                },
                onOpenImport = { sessionId -> navController.navigate("importSession/$sessionId") },
            )
        }
        slideComposable(
            PROVIDER_ACCOUNT_DETAIL_ROUTE,
            slideSpec,
            arguments =
                listOf(
                    navArgument("connectionId") { type = NavType.StringType },
                    navArgument("providerAccountId") { type = NavType.StringType },
                    navArgument("name") {
                        type = NavType.StringType
                        defaultValue = ""
                    },
                    navArgument("currency") {
                        type = NavType.StringType
                        defaultValue = ""
                    },
                ),
        ) { entry ->
            val connectionId = entry.arguments?.getString("connectionId") ?: return@slideComposable
            val providerAccountId =
                entry.arguments?.getString("providerAccountId") ?: return@slideComposable
            val linkSetup: BindingSetupViewModel? =
                remember(entry) {
                    navController.previousBackStackEntry?.takeIf { it.destination.route == BINDING_SETUP_ROUTE }
                }?.let { viewModel(it, factory = SvertoViewModelFactory) }
            ProviderAccountDetailScreen(
                connectionId = connectionId,
                providerAccountId = providerAccountId,
                displayName =
                    entry.arguments
                        ?.getString("name")
                        .orEmpty()
                        .ifEmpty { providerAccountId },
                currency = entry.arguments?.getString("currency")?.takeIf { it.isNotEmpty() },
                onBack = { navController.popBackStack() },
                onChoose =
                    linkSetup?.let { setup ->
                        { account ->
                            setup.choose(providerAccountId, account)
                            navController.popBackStack()
                        }
                    },
            )
        }
        slideComposable(
            CONNECT_TRUELAYER_ROUTE,
            slideSpec,
            arguments =
                listOf(
                    navArgument("code") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                    navArgument("state") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                    navArgument("error") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                ),
            deepLinks =
                listOf(
                    navDeepLink {
                        uriPattern = "sverto://connectors/truelayer?code={code}&state={state}&error={error}"
                    },
                ),
        ) { entry ->
            ConnectTrueLayerScreen(
                oauthState = entry.arguments?.getString("state"),
                oauthCode = entry.arguments?.getString("code"),
                oauthError = entry.arguments?.getString("error"),
                onBack = { navController.popBackStack() },
                onCompleted = { connectionId ->
                    navController.navigate("connectionDetail/$connectionId") {
                        popUpTo(CONNECT_TRUELAYER_ROUTE) { inclusive = true }
                    }
                    navController.navigate("bindingSetup/$connectionId")
                },
            )
        }
        slideComposable(CONNECT_TRADING212_ROUTE, slideSpec) {
            ConnectTrading212Screen(
                onBack = { navController.popBackStack() },
                onCreated = { id ->
                    navController.navigate("connectionDetail/$id") {
                        popUpTo(CONNECT_TRADING212_ROUTE) { inclusive = true }
                    }
                    navController.navigate("bindingSetup/$id")
                },
            )
        }
        slideComposable(
            CONNECT_ENABLEBANKING_ROUTE,
            slideSpec,
            arguments =
                listOf(
                    navArgument("code") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                    navArgument("state") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                    navArgument("error") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                ),
            deepLinks =
                listOf(
                    navDeepLink {
                        uriPattern = "sverto://connectors/enablebanking?code={code}&state={state}&error={error}"
                    },
                ),
        ) { entry ->
            ConnectEnableBankingScreen(
                oauthState = entry.arguments?.getString("state"),
                oauthCode = entry.arguments?.getString("code"),
                oauthError = entry.arguments?.getString("error"),
                onBack = { navController.popBackStack() },
                onCompleted = { connectionId ->
                    navController.navigate("connectionDetail/$connectionId") {
                        popUpTo(CONNECT_ENABLEBANKING_ROUTE) { inclusive = true }
                    }
                    navController.navigate("bindingSetup/$connectionId")
                },
            )
        }
        composable(
            route = MARKET_ASSET_DETAIL_ROUTE,
            arguments =
                listOf(
                    navArgument("assetId") { type = NavType.IntType },
                    navArgument("userAsset") {
                        type = NavType.BoolType
                        defaultValue = false
                    },
                ),
        ) { backStackEntry ->
            val assetId = backStackEntry.arguments?.getInt("assetId") ?: return@composable
            val userAsset = backStackEntry.arguments?.getBoolean("userAsset") ?: false
            MarketAssetDetailScreen(
                assetId = assetId,
                userAsset = userAsset,
                onBack = { navController.popBackStack() },
            )
        }
        composable(CUSTOM_ASSETS_ROUTE) {
            CustomAssetsScreen(
                onBack = { navController.popBackStack() },
                onAssetClick = { assetId -> navController.navigate("assetOverview/$assetId?userAsset=true") },
            )
        }
        composable(
            route = ASSET_OVERVIEW_ROUTE,
            arguments =
                listOf(
                    navArgument("assetId") { type = NavType.IntType },
                    navArgument("userAsset") {
                        type = NavType.BoolType
                        defaultValue = false
                    },
                ),
        ) { backStackEntry ->
            val assetId = backStackEntry.arguments?.getInt("assetId") ?: return@composable
            val userAsset = backStackEntry.arguments?.getBoolean("userAsset") ?: false

            @Suppress("ViewModelForwarding")
            val vm: AssetOverviewViewModel = viewModel(factory = SvertoViewModelFactory)
            AssetOverviewScreen(
                assetId = assetId,
                userAsset = userAsset,
                onBack = { navController.popBackStack() },
                onViewTransactions = { title ->
                    navController.navigate(assetTransactionsRoute(assetId, title))
                },
                viewModel = vm,
            )
        }
    }
}

// An entry's predecessor never changes while it is on the back stack. Reading it once keeps it right
// while predictive back composes this entry underneath the one being popped.
@Composable
private fun showsBack(
    navController: NavHostController,
    entry: NavBackStackEntry,
    listPaneTab: TopLevelRoute?,
): Boolean {
    val parentRoute =
        remember(entry) {
            val stack = navController.currentBackStack.value
            stack.getOrNull(stack.indexOf(entry) - 1)?.destination?.route
        }
    return listPaneTab == null || parentRoute != listPaneTab.route
}

private fun assetTransactionsRoute(
    assetId: Int,
    title: String,
    accountId: String? = null,
    accountName: String? = null,
): String {
    val accountParam = accountId?.let { "&accountId=${Uri.encode(it)}" } ?: ""
    val accountNameParam = accountName?.let { "&accountName=${Uri.encode(it)}" } ?: ""
    return "assetTransactions/$assetId?title=${Uri.encode(title)}$accountParam$accountNameParam"
}

private fun NavGraphBuilder.slideComposable(
    route: String,
    slideSpec: FiniteAnimationSpec<IntOffset>,
    arguments: List<NamedNavArgument> = emptyList(),
    deepLinks: List<NavDeepLink> = emptyList(),
    content: @Composable AnimatedContentScope.(NavBackStackEntry) -> Unit,
) {
    composable(
        route = route,
        arguments = arguments,
        deepLinks = deepLinks,
        enterTransition = {
            slideIntoContainer(AnimatedContentTransitionScope.SlideDirection.Start, slideSpec)
        },
        exitTransition = {
            slideOutOfContainer(AnimatedContentTransitionScope.SlideDirection.Start, slideSpec)
        },
        popEnterTransition = {
            slideIntoContainer(AnimatedContentTransitionScope.SlideDirection.End, slideSpec)
        },
        popExitTransition = {
            slideOutOfContainer(AnimatedContentTransitionScope.SlideDirection.End, slideSpec)
        },
        content = content,
    )
}
