package com.sverto.app

import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.sverto.app.core.SvertoViewModelFactory
import com.sverto.app.core.navigation.TopLevelRoute
import com.sverto.app.feature.accounts.AccountsScreen
import com.sverto.app.feature.aichat.AiChatViewModel
import com.sverto.app.feature.aichat.ConversationList
import com.sverto.app.feature.transactions.TransactionsScreen
import com.sverto.app.feature.transactions.TransactionsViewModel
import com.sverto.app.feature.transactions.held.HeldTransactionsViewModel
import com.sverto.app.feature.transactions.quickupload.QuickUploadUiItem
import uniffi.sverto_core.TransactionListItem

@OptIn(ExperimentalSharedTransitionApi::class)
@Suppress("ViewModelForwarding", "MutableStateParam")
@Composable
internal fun TransactionsTabRoot(
    viewModel: TransactionsViewModel,
    listState: LazyListState,
    quickUploadItems: List<QuickUploadUiItem>,
    onQuickUpload: () -> Unit,
    onQuickUploadRetry: (String) -> Unit,
    onQuickUploadDismiss: (String) -> Unit,
    onRefreshQuickUploads: () -> Unit,
    pendingGroupSelection: MutableState<List<String>>,
    navigate: (String) -> Unit,
    onTransactionClick: (TransactionListItem) -> Unit,
    openTransactionId: String?,
    sharedTransitionScope: SharedTransitionScope?,
    animatedVisibilityScope: AnimatedVisibilityScope?,
    modifier: Modifier = Modifier,
    heldTransactionsViewModel: HeldTransactionsViewModel = viewModel(factory = SvertoViewModelFactory),
) {
    TransactionsScreen(
        modifier = modifier,
        onTransactionClick = onTransactionClick,
        openTransactionId = openTransactionId,
        onCreateTransaction = { typeKey -> navigate("createTransaction/$typeKey") },
        onCreateGroup = { navigate("createTransactionGroup") },
        onGroupSelected = { ids ->
            pendingGroupSelection.value = ids
            navigate("createTransactionGroup?fromSelection=true")
        },
        onQuickUpload = onQuickUpload,
        onImportStatements = { navigate("imports") },
        quickUploadItems = quickUploadItems,
        onQuickUploadItemClick = { item ->
            if (item.proposalType == "transaction_group") {
                navigate("createTransactionGroup?quickUploadId=${item.id}")
            } else {
                navigate("createTransaction/regular_transaction?quickUploadId=${item.id}")
            }
        },
        onQuickUploadRetry = onQuickUploadRetry,
        onQuickUploadDismiss = onQuickUploadDismiss,
        onRefreshQuickUploads = onRefreshQuickUploads,
        sharedTransitionScope = sharedTransitionScope,
        animatedVisibilityScope = animatedVisibilityScope,
        listState = listState,
        viewModel = viewModel,
        heldTransactionsViewModel = heldTransactionsViewModel,
    )
}

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
internal fun AccountsTabRoot(
    scrollState: ScrollState,
    navigate: (String) -> Unit,
    openId: String?,
    sharedTransitionScope: SharedTransitionScope?,
    animatedVisibilityScope: AnimatedVisibilityScope?,
    modifier: Modifier = Modifier,
) {
    AccountsScreen(
        onAccountClick = { account ->
            val property = account.property
            if (property != null) {
                navigate("property/${property.propertyId}/${property.assetId}")
            } else {
                navigate("accountDetail/${account.id}/${account.name}/${account.accountTypeId}")
            }
        },
        onAddAccount = { navigate("addAccount") },
        onAddProperty = { navigate("addProperty") },
        openId = openId,
        sharedTransitionScope = sharedTransitionScope,
        animatedVisibilityScope = animatedVisibilityScope,
        scrollState = scrollState,
        modifier = modifier,
    )
}

@Suppress("ViewModelForwarding", "MutableStateParam")
@Composable
internal fun ListPaneContent(
    tab: TopLevelRoute,
    rightPaneEntryId: String?,
    openId: String?,
    transactionsViewModel: TransactionsViewModel?,
    transactionsListState: LazyListState,
    accountsScrollState: ScrollState,
    quickUploadItems: List<QuickUploadUiItem>,
    onQuickUpload: () -> Unit,
    onQuickUploadRetry: (String) -> Unit,
    onQuickUploadDismiss: (String) -> Unit,
    onRefreshQuickUploads: () -> Unit,
    pendingGroupSelection: MutableState<List<String>>,
    aiChatViewModel: AiChatViewModel?,
    replace: (String) -> Unit,
    onTransactionClick: (TransactionListItem) -> Unit,
    onConversationOpened: () -> Unit,
    modifier: Modifier = Modifier,
) {
    when (tab) {
        TopLevelRoute.Transactions ->
            if (transactionsViewModel != null) {
                // The list stays resumed while the right pane changes, so reload what the phone
                // reloads when the list comes back on top.
                val held: HeldTransactionsViewModel = viewModel(factory = SvertoViewModelFactory)
                LaunchedEffect(rightPaneEntryId) { held.load() }
                TransactionsTabRoot(
                    viewModel = transactionsViewModel,
                    listState = transactionsListState,
                    quickUploadItems = quickUploadItems,
                    onQuickUpload = onQuickUpload,
                    onQuickUploadRetry = onQuickUploadRetry,
                    onQuickUploadDismiss = onQuickUploadDismiss,
                    onRefreshQuickUploads = onRefreshQuickUploads,
                    pendingGroupSelection = pendingGroupSelection,
                    navigate = replace,
                    onTransactionClick = onTransactionClick,
                    openTransactionId = openId,
                    sharedTransitionScope = null,
                    animatedVisibilityScope = null,
                    modifier = modifier,
                    heldTransactionsViewModel = held,
                )
            }

        TopLevelRoute.Accounts ->
            AccountsTabRoot(
                scrollState = accountsScrollState,
                navigate = replace,
                openId = openId,
                sharedTransitionScope = null,
                animatedVisibilityScope = null,
                modifier = modifier,
            )

        TopLevelRoute.AiChat ->
            if (aiChatViewModel != null) {
                val state by aiChatViewModel.uiState.collectAsStateWithLifecycle()
                ConversationList(
                    conversations = state.conversations,
                    activeConversationId = state.activeConversationId,
                    activeContainerColor = MaterialTheme.colorScheme.secondaryContainer,
                    onSelect = { id ->
                        aiChatViewModel.selectConversation(id)
                        onConversationOpened()
                    },
                    onCreate = {
                        aiChatViewModel.startNewConversation()
                        onConversationOpened()
                    },
                    onDelete = aiChatViewModel::deleteConversation,
                    modifier = modifier,
                )
            }

        TopLevelRoute.Portfolio -> Unit
    }
}
