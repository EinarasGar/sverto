package com.sverto.app.feature.accounts

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.launchCatching
import com.sverto.app.feature.assets.components.HoldingSeries
import com.sverto.app.feature.portfolio.ChartPoint
import com.sverto.app.feature.portfolio.TimePeriod
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.AssetDetailObserver
import uniffi.sverto_core.AssetDetailState

data class HoldingChartUiState(
    val isHeld: Boolean = false,
    val chosenSeries: HoldingSeries = HoldingSeries.VALUE,
    val period: TimePeriod = TimePeriod.MONTH,
    val showBase: Boolean = false,
    val valueByPeriod: Map<TimePeriod, List<ChartPoint>> = emptyMap(),
    val unitsByPeriod: Map<TimePeriod, List<ChartPoint>> = emptyMap(),
) {
    val series: HoldingSeries get() = if (isHeld) chosenSeries else HoldingSeries.PRICE
}

class AssetDetailViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state =
        MutableStateFlow(
            AssetDetailState(
                isLoading = true,
                error = null,
                assetId = 0,
                ticker = "",
                name = "",
                units = 0.0,
                value = 0.0,
                costBasis = 0.0,
                unrealizedGains = 0.0,
                totalFees = 0.0,
                currentPrice = 0.0,
                chartData = emptyList(),
                lots = emptyList(),
                baseTicker = "",
                priceTicker = "",
                priceCurrencyId = 0,
                supportsBaseConversion = false,
                baseChartData = emptyList(),
            ),
        )
    val state: StateFlow<AssetDetailState> = _state.asStateFlow()

    private val _chart = MutableStateFlow(HoldingChartUiState())
    val chart: StateFlow<HoldingChartUiState> = _chart.asStateFlow()

    private val _errors = MutableSharedFlow<String>(extraBufferCapacity = 4)
    val errors = _errors.asSharedFlow()

    private val _isRefreshing = MutableStateFlow(false)
    val isRefreshing: StateFlow<Boolean> = _isRefreshing.asStateFlow()

    private var accountId = ""
    private var assetId = 0

    private val observer =
        object : AssetDetailObserver {
            override fun onAssetDetailChanged(state: AssetDetailState) {
                val wasRefreshing = _isRefreshing.value
                _state.value = state
                if (wasRefreshing && !state.isLoading) {
                    _isRefreshing.value = false
                }
                if (!state.isLoading && state.error == null && state.assetId == assetId) {
                    _chart.update { it.copy(isHeld = state.units != 0.0) }
                    loadSelectedSeries()
                }
            }
        }

    init {
        store.observeAssetDetail(observer)
    }

    fun load(
        accountId: String,
        assetId: Int,
    ) {
        if (accountId != this.accountId || assetId != this.assetId) _chart.value = HoldingChartUiState()
        this.accountId = accountId
        this.assetId = assetId
        viewModelScope.launch { store.loadAssetDetail(accountId, assetId) }
    }

    fun refresh() {
        _isRefreshing.value = true
        viewModelScope.launch { store.refreshAssetDetail() }
    }

    fun selectSeries(series: HoldingSeries) {
        _chart.update { it.copy(chosenSeries = series) }
        loadSelectedSeries()
    }

    fun selectPeriod(period: TimePeriod) {
        _chart.update { it.copy(period = period) }
        loadSelectedSeries()
    }

    fun selectBaseCurrency(showBase: Boolean) {
        _chart.update { it.copy(showBase = showBase, valueByPeriod = emptyMap()) }
        if (showBase && _state.value.baseChartData.isEmpty()) {
            viewModelScope.launch { store.loadAssetDetailBaseChart() }
        }
        loadSelectedSeries()
    }

    private fun loadSelectedSeries() {
        val chart = _chart.value
        val period = chart.period
        when (chart.series) {
            HoldingSeries.PRICE -> Unit
            HoldingSeries.VALUE -> if (period !in chart.valueByPeriod) loadHoldingValue(period, chart.showBase)
            HoldingSeries.UNITS -> if (period !in chart.unitsByPeriod) loadHoldingUnits(period)
        }
    }

    private fun loadHoldingValue(
        period: TimePeriod,
        showBase: Boolean,
    ) = launchCatching(_errors) {
        val currencyId = if (showBase) store.getDefaultAssetId() else _state.value.priceCurrencyId
        val points =
            store
                .getHoldingValueHistory(assetId, accountId, currencyId ?: return@launchCatching, period.apiRange)
                .map { ChartPoint(date = it.timestamp, value = it.value) }
        _chart.update {
            if (it.showBase != showBase) it else it.copy(valueByPeriod = it.valueByPeriod + (period to points))
        }
    }

    private fun loadHoldingUnits(period: TimePeriod) =
        launchCatching(_errors) {
            val points =
                store
                    .getHoldingUnitsHistory(assetId, accountId, period.apiRange)
                    .map { ChartPoint(date = it.timestamp, value = it.value) }
            _chart.update { it.copy(unitsByPeriod = it.unitsByPeriod + (period to points)) }
        }

    override fun onCleared() {
        store.unobserveAssetDetail()
    }
}
