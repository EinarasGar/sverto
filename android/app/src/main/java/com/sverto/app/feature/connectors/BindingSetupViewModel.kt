package com.sverto.app.feature.connectors

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.friendlyMessage
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.sverto_core.AccountListItem
import uniffi.sverto_core.ApiException
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.ConnectorBinding
import uniffi.sverto_core.ConnectorImportAccount
import uniffi.sverto_core.ConnectorImportFrom
import uniffi.sverto_core.ProviderAccount
import uniffi.sverto_core.ProviderAccountHistory

data class BindingSetupUiState(
    val providerAccounts: List<ProviderAccount> = emptyList(),
    val bindings: Map<String, ConnectorBinding> = emptyMap(),
    val svertoAccounts: List<AccountListItem> = emptyList(),
    val loading: Boolean = true,
    val error: String? = null,
    val linksByImport: Boolean = false,
    val choices: Map<String, AccountListItem> = emptyMap(),
    val importFrom: ConnectorImportFrom = ConnectorImportFrom.AfterLastEntry,
    val linking: Boolean = false,
    val importSessionId: String? = null,
)

class BindingSetupViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state = MutableStateFlow(BindingSetupUiState())
    val state: StateFlow<BindingSetupUiState> = _state.asStateFlow()

    private var loadJob: Job? = null

    fun load(connectionId: String) {
        loadJob?.cancel()
        loadJob =
            viewModelScope.launch(Dispatchers.IO) {
                _state.value = _state.value.copy(loading = true, error = null)
                do {
                    try {
                        val provider = store.listConnectorProviderAccounts(connectionId)
                        val bindings =
                            store
                                .listConnectorBindings()
                                .filter { it.connectionId == connectionId }
                                .associateBy { it.providerAccountId }
                        val accounts = store.listSvertoAccounts()
                        val linksByImport =
                            store.listConnectorConnections().firstOrNull { it.id == connectionId }?.linksByImport == true
                        _state.update {
                            it.copy(
                                providerAccounts = provider,
                                bindings = bindings,
                                svertoAccounts = accounts,
                                loading = false,
                                linksByImport = linksByImport,
                                choices = it.choices.filterKeys { id -> id !in bindings },
                            )
                        }
                    } catch (e: CancellationException) {
                        throw e
                    } catch (
                        @Suppress("TooGenericExceptionCaught") e: Exception,
                    ) {
                        _state.value = _state.value.copy(loading = false, error = e.message ?: "Failed to load accounts")
                        return@launch
                    }
                    val fetching = _state.value.providerAccounts.any { it.history == ProviderAccountHistory.FETCHING }
                    if (fetching) delay(HISTORY_POLL_MS)
                } while (fetching)
            }
    }

    fun choose(
        providerAccountId: String,
        account: AccountListItem,
    ) = _state.update { it.copy(choices = it.choices + (providerAccountId to account)) }

    fun clearChoice(providerAccountId: String) = _state.update { it.copy(choices = it.choices - providerAccountId) }

    fun setImportFrom(importFrom: ConnectorImportFrom) = _state.update { it.copy(importFrom = importFrom) }

    fun linkAccounts(connectionId: String) {
        val current = _state.value
        if (current.linking) return
        _state.update { it.copy(linking = true, error = null) }
        viewModelScope.launch(Dispatchers.IO) {
            try {
                if (current.linksByImport) {
                    val sessionId =
                        store.createConnectorImport(
                            connectionId,
                            current.choices.map { (providerAccountId, account) -> ConnectorImportAccount(providerAccountId, account.id) },
                            current.importFrom,
                        )
                    _state.update { it.copy(importSessionId = sessionId) }
                } else {
                    current.choices.forEach { (providerAccountId, account) ->
                        store.createConnectorBinding(connectionId, account.id, providerAccountId, current.importFrom)
                        _state.update { it.copy(choices = it.choices - providerAccountId) }
                    }
                    load(connectionId)
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: ApiException) {
                _state.update { it.copy(error = e.friendlyMessage()) }
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                _state.update { it.copy(error = e.message ?: "Failed to link accounts") }
            } finally {
                _state.update { it.copy(linking = false) }
            }
        }
    }
}

const val HISTORY_POLL_MS = 5_000L
