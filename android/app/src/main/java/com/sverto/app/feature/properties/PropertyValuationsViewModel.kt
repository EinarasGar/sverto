package com.sverto.app.feature.properties

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.launchCatching
import com.sverto.app.feature.portfolio.ChartPoint
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.sverto_core.ApiException
import uniffi.sverto_core.AppStore

data class PropertyValuationsUiState(
    val isLoading: Boolean = true,
    val isRefreshing: Boolean = false,
    val failed: Boolean = false,
    val currencyTicker: String = "",
    val valuations: List<ChartPoint> = emptyList(),
)

class PropertyValuationsViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state = MutableStateFlow(PropertyValuationsUiState())
    val state = _state.asStateFlow()

    private val _errors = MutableSharedFlow<String>(extraBufferCapacity = 4)
    val errors = _errors.asSharedFlow()

    private val _saved = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    val saved = _saved.asSharedFlow()

    private var assetId: Int = 0
    private var currencyAssetId: Int = 0

    fun load(
        assetId: Int,
        currencyAssetId: Int,
    ) {
        this.assetId = assetId
        this.currencyAssetId = currencyAssetId
        viewModelScope.launch(Dispatchers.IO) { loadValuations() }
    }

    fun refresh() {
        _state.update { it.copy(isRefreshing = true) }
        viewModelScope.launch(Dispatchers.IO) {
            store.refreshProperty(assetId)
            loadValuations()
            _state.update { it.copy(isRefreshing = false) }
        }
    }

    fun add(
        date: Long,
        value: Double,
    ) = launchCatching(_errors) {
        store.addPropertyValuation(assetId, currencyAssetId, date, value)
        _saved.emit(Unit)
        loadValuations()
    }

    fun edit(
        valuation: ChartPoint,
        date: Long,
        value: Double,
    ) = launchCatching(_errors) {
        store.editPropertyValuation(assetId, currencyAssetId, valuation.date, date, value)
        _saved.emit(Unit)
        loadValuations()
    }

    fun delete(valuation: ChartPoint) =
        launchCatching(_errors) {
            store.deletePropertyValuation(assetId, currencyAssetId, valuation.date)
            loadValuations()
        }

    private suspend fun loadValuations() {
        try {
            val currencyTicker =
                _state.value.currencyTicker.ifEmpty {
                    store
                        .getAllCurrencies()
                        .firstOrNull { it.id == currencyAssetId }
                        ?.ticker
                        .orEmpty()
                }
            val valuations =
                store
                    .getAssetPairRates(assetId, currencyAssetId, "all", true)
                    .map { ChartPoint(date = it.timestamp, value = it.value) }
                    .sortedByDescending { it.date }
            _state.update {
                it.copy(isLoading = false, failed = false, currencyTicker = currencyTicker, valuations = valuations)
            }
        } catch (_: ApiException) {
            _state.update { it.copy(isLoading = false, failed = true) }
        }
    }
}
