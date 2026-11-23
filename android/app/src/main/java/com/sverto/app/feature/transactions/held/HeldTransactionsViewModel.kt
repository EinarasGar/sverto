package com.sverto.app.feature.transactions.held

import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.friendlyMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.sverto_core.ApiException
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.HeldTransaction
import uniffi.sverto_core.HeldTransactionDecision

private const val TAG = "HeldTransactionsVM"

data class HeldTransactionsUiState(
    val items: List<HeldTransaction> = emptyList(),
    val decidingId: String? = null,
    val error: HeldDecisionError? = null,
)

data class HeldDecisionError(
    val heldTransactionId: String,
    val message: String,
)

class HeldTransactionsViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state = MutableStateFlow(HeldTransactionsUiState())
    val state: StateFlow<HeldTransactionsUiState> = _state.asStateFlow()

    fun load() {
        viewModelScope.launch {
            try {
                val items = store.listHeldTransactions()
                _state.update { it.copy(items = items) }
            } catch (e: ApiException) {
                Log.w(TAG, "Loading held transactions failed", e)
            }
        }
    }

    fun decide(
        heldTransactionId: String,
        decision: HeldTransactionDecision,
    ) {
        if (_state.value.decidingId != null) return
        _state.update { it.copy(decidingId = heldTransactionId, error = null) }
        viewModelScope.launch {
            try {
                store.decideHeldTransaction(heldTransactionId, decision)
                _state.update { state -> state.copy(items = state.items.filterNot { it.id == heldTransactionId }) }
                store.refreshTransactions()
            } catch (e: ApiException) {
                _state.update { it.copy(error = HeldDecisionError(heldTransactionId, e.friendlyMessage())) }
            } finally {
                _state.update { it.copy(decidingId = null) }
            }
            load()
        }
    }
}
