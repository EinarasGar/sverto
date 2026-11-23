package com.sverto.app.feature.connectors

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.sverto_core.AccountListItem
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.ProviderAccountHistory
import uniffi.sverto_core.ProviderAccountTransaction

data class ProviderAccountDetailUiState(
    val transactions: List<ProviderAccountTransaction> = emptyList(),
    val svertoAccounts: List<AccountListItem> = emptyList(),
    val linkedTo: String? = null,
    val history: ProviderAccountHistory? = null,
    val loading: Boolean = true,
    val error: String? = null,
)

class ProviderAccountDetailViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state = MutableStateFlow(ProviderAccountDetailUiState())
    val state: StateFlow<ProviderAccountDetailUiState> = _state.asStateFlow()

    private var loadJob: Job? = null

    fun load(
        connectionId: String,
        providerAccountId: String,
    ) {
        loadJob?.cancel()
        loadJob =
            viewModelScope.launch(Dispatchers.IO) {
                _state.value = _state.value.copy(loading = true, error = null)
                do {
                    try {
                        val transactions =
                            store.listConnectorProviderAccountTransactions(connectionId, providerAccountId)
                        val history =
                            store
                                .listConnectorProviderAccounts(connectionId)
                                .firstOrNull { it.providerAccountId == providerAccountId }
                                ?.history
                        val linked =
                            store
                                .listConnectorBindings()
                                .firstOrNull {
                                    it.connectionId == connectionId && it.providerAccountId == providerAccountId
                                }?.svertoAccountName
                        val accounts = store.listSvertoAccounts()
                        _state.value =
                            _state.value.copy(
                                transactions = transactions,
                                svertoAccounts = accounts,
                                linkedTo = linked,
                                history = history,
                                loading = false,
                            )
                    } catch (e: CancellationException) {
                        throw e
                    } catch (
                        @Suppress("TooGenericExceptionCaught") e: Exception,
                    ) {
                        _state.value =
                            _state.value.copy(loading = false, error = e.message ?: "Failed to load transactions")
                        return@launch
                    }
                    val fetching = _state.value.history == ProviderAccountHistory.FETCHING
                    if (fetching) delay(HISTORY_POLL_MS)
                } while (fetching)
            }
    }
}
