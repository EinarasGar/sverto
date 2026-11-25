package com.sverto.app.feature.portfolio

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.friendlyMessage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.sverto_core.ApiException
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.CurrencyDetail

data class CurrencyDetailUiState(
    val isLoading: Boolean = true,
    val detail: CurrencyDetail? = null,
    val isRefreshing: Boolean = false,
)

class CurrencyDetailViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state = MutableStateFlow(CurrencyDetailUiState())
    val state = _state.asStateFlow()

    private val _errors = MutableSharedFlow<String>(extraBufferCapacity = 4)
    val errors = _errors.asSharedFlow()

    private var assetId: Int = 0
    private var accountId: String? = null

    fun load(
        assetId: Int,
        accountId: String?,
    ) {
        this.assetId = assetId
        this.accountId = accountId
        viewModelScope.launch(Dispatchers.IO) { reload(refresh = false) }
    }

    fun refresh() {
        _state.update { it.copy(isRefreshing = true) }
        viewModelScope.launch(Dispatchers.IO) {
            reload(refresh = true)
            _state.update { it.copy(isRefreshing = false) }
        }
    }

    private suspend fun reload(refresh: Boolean) {
        try {
            val detail = store.getCurrencyDetail(assetId, accountId, refresh)
            _state.update { it.copy(isLoading = false, detail = detail) }
        } catch (e: ApiException) {
            _state.update { it.copy(isLoading = false) }
            _errors.emit(e.friendlyMessage())
        }
    }
}
