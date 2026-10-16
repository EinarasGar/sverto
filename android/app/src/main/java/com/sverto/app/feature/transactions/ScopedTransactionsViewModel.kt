package com.sverto.app.feature.transactions

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.ScopedTransactionsObserver
import uniffi.sverto_core.ScopedTransactionsState
import uniffi.sverto_core.TransactionListScope

class ScopedTransactionsViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state =
        MutableStateFlow(
            ScopedTransactionsState(
                isLoading = true,
                isLoadingMore = false,
                error = null,
                items = emptyList(),
                hasMore = false,
            ),
        )
    val state: StateFlow<ScopedTransactionsState> = _state.asStateFlow()

    private val _isRefreshing = MutableStateFlow(false)
    val isRefreshing: StateFlow<Boolean> = _isRefreshing.asStateFlow()

    private val observer =
        object : ScopedTransactionsObserver {
            override fun onScopedTransactionsChanged(state: ScopedTransactionsState) {
                val wasRefreshing = _isRefreshing.value
                _state.value = state
                if (wasRefreshing && !state.isLoading) {
                    _isRefreshing.value = false
                }
            }
        }

    private var loadedScope: TransactionListScope? = null

    init {
        store.observeScopedTransactions(observer)
    }

    fun load(scope: TransactionListScope) {
        if (scope == loadedScope) return
        loadedScope = scope
        viewModelScope.launch { store.loadScopedTransactions(scope) }
    }

    fun loadMore() {
        viewModelScope.launch { store.loadMoreScopedTransactions() }
    }

    fun refresh() {
        _isRefreshing.value = true
        viewModelScope.launch { store.refreshScopedTransactions() }
    }

    override fun onCleared() {
        store.unobserveScopedTransactions()
    }
}
