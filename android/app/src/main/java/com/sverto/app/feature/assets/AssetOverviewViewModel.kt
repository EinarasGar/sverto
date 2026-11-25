package com.sverto.app.feature.assets

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.friendlyMessage
import com.sverto.app.core.launchCatching
import com.sverto.app.feature.assets.components.HoldingSeries
import com.sverto.app.feature.portfolio.ChartPoint
import com.sverto.app.feature.portfolio.TimePeriod
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.AssetDetail
import uniffi.sverto_core.AssetOverviewObserver
import uniffi.sverto_core.AssetOverviewState
import uniffi.sverto_core.AssetPairDetail
import uniffi.sverto_core.AssetPairRef

data class AssetOverviewUiState(
    val isLoading: Boolean = true,
    val overview: AssetOverviewState? = null,
    val detail: AssetDetail? = null,
    val selectedPairId: Int? = null,
    val pairInfo: AssetPairDetail? = null,
    val selectedPeriod: TimePeriod = TimePeriod.MONTH,
    val chartByPeriod: Map<TimePeriod, List<ChartPoint>> = emptyMap(),
    val isUserAsset: Boolean = false,
    val isReloading: Boolean = false,
    val isHeld: Boolean? = null,
    val chosenSeries: HoldingSeries = HoldingSeries.VALUE,
    val valueByPeriod: Map<TimePeriod, List<ChartPoint>> = emptyMap(),
    val unitsByPeriod: Map<TimePeriod, List<ChartPoint>> = emptyMap(),
) {
    val series: HoldingSeries get() = if (isHeld == true) chosenSeries else HoldingSeries.PRICE

    val seriesData: Map<TimePeriod, List<ChartPoint>>
        get() =
            when (series) {
                HoldingSeries.PRICE -> chartByPeriod
                HoldingSeries.VALUE -> valueByPeriod
                HoldingSeries.UNITS -> unitsByPeriod
            }
}

class AssetOverviewViewModel(
    private val store: AppStore,
) : ViewModel(),
    AssetOverviewObserver {
    private val _state = MutableStateFlow(AssetOverviewUiState())
    val state = _state.asStateFlow()

    private val _errors = MutableSharedFlow<String>(extraBufferCapacity = 4)
    val errors = _errors.asSharedFlow()

    private val _saved = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    val saved = _saved.asSharedFlow()

    private var assetId: Int = 0

    init {
        store.observeAssetOverview(this)
    }

    override fun onCleared() {
        super.onCleared()
        store.unobserveAssetOverview()
    }

    override fun onAssetOverviewChanged(s: AssetOverviewState) {
        if (s.assetId != 0 && s.assetId != assetId) return
        viewModelScope.launch {
            val wasReloading = _state.value.isReloading
            // The store resets to an empty loading state on every pair switch; holding is only known once loaded.
            val settled = s.assetId == assetId && !s.isLoading && s.error == null
            _state.update {
                it.copy(
                    overview = s,
                    isLoading = false,
                    isReloading = false,
                    isHeld = if (settled) s.units != 0.0 else it.isHeld,
                )
            }
            loadSelectedSeries()
            if (s.error != null) {
                _errors.emit(s.error!!)
            } else if (wasReloading) {
                _state.value = _state.value.copy(isReloading = false)
            }
        }
    }

    fun load(
        assetId: Int,
        userAsset: Boolean,
    ) {
        val alreadyLoaded =
            assetId == this.assetId &&
                userAsset == _state.value.isUserAsset &&
                _state.value.detail != null
        if (alreadyLoaded) return
        this.assetId = assetId
        reload(userAsset)
    }

    private fun reload(userAsset: Boolean) {
        _state.value = AssetOverviewUiState(isLoading = true, isUserAsset = userAsset)
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val detail = store.getAssetDetail(assetId, userAsset)
                val baseCurrencyId = store.getDefaultAssetId()
                val defaultPair =
                    detail.basePairId
                        ?: detail.pairs.firstOrNull { it.assetId == baseCurrencyId }?.assetId
                        ?: detail.pairs.firstOrNull()?.assetId
                _state.value =
                    _state.value.copy(isLoading = false, detail = detail, selectedPairId = defaultPair)
                if (defaultPair != null) selectPair(defaultPair)
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                _state.value = _state.value.copy(isLoading = false)
                _errors.emit(e.friendlyMessage())
            }
        }
    }

    fun selectPair(referenceId: Int) {
        _state.update {
            it.copy(
                selectedPairId = referenceId,
                pairInfo = null,
                chartByPeriod = emptyMap(),
                valueByPeriod = emptyMap(),
                isReloading = true,
            )
        }
        loadPairInfo(referenceId)
        loadSelectedSeries()
        viewModelScope.launch(Dispatchers.IO) {
            store.loadAssetOverview(assetId, referenceId)
        }
    }

    fun selectPeriod(period: TimePeriod) {
        _state.update { it.copy(selectedPeriod = period) }
        loadSelectedSeries()
    }

    fun selectSeries(series: HoldingSeries) {
        _state.update { it.copy(chosenSeries = series) }
        loadSelectedSeries()
    }

    private fun loadSelectedSeries() {
        val s = _state.value
        val period = s.selectedPeriod
        if (s.seriesData.containsKey(period)) return
        when (s.series) {
            HoldingSeries.PRICE -> loadRates(period)
            HoldingSeries.VALUE -> loadHoldingValue(period)
            HoldingSeries.UNITS -> loadHoldingUnits(period)
        }
    }

    private fun loadHoldingValue(period: TimePeriod) {
        val currencyId = _state.value.selectedPairId ?: return
        launchCatching(_errors) {
            val points =
                store
                    .getHoldingValueHistory(assetId, null, currencyId, period.apiRange)
                    .map { ChartPoint(date = it.timestamp, value = it.value) }
            _state.update {
                if (it.selectedPairId != currencyId) it else it.copy(valueByPeriod = it.valueByPeriod + (period to points))
            }
        }
    }

    private fun loadHoldingUnits(period: TimePeriod) =
        launchCatching(_errors) {
            val points =
                store
                    .getHoldingUnitsHistory(assetId, null, period.apiRange)
                    .map { ChartPoint(date = it.timestamp, value = it.value) }
            _state.update { it.copy(unitsByPeriod = it.unitsByPeriod + (period to points)) }
        }

    private fun isConverted(referenceId: Int): Boolean =
        _state.value.detail
            ?.pairs
            ?.firstOrNull { it.assetId == referenceId }
            ?.converted == true

    private fun loadPairInfo(referenceId: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val info =
                    if (isConverted(referenceId)) {
                        val conv =
                            store.getAssetPairConverted(assetId, referenceId, _state.value.isUserAsset)
                        val detail = _state.value.detail
                        val pair = detail?.pairs?.firstOrNull { it.assetId == referenceId }
                        AssetPairDetail(
                            mainTicker = detail?.displaySymbol ?: "",
                            mainName = detail?.name ?: "",
                            refTicker = pair?.ticker ?: "",
                            refName = pair?.name ?: "",
                            latestRate = conv.latestRate,
                            lastUpdated = conv.lastUpdated,
                            volume = null,
                            exchange = null,
                        )
                    } else {
                        store.getAssetPair(assetId, referenceId, _state.value.isUserAsset)
                    }
                _state.value = _state.value.copy(pairInfo = info)
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                _errors.emit(e.friendlyMessage())
            }
        }
    }

    private fun loadRates(period: TimePeriod) {
        val referenceId = _state.value.selectedPairId ?: return
        val converted = isConverted(referenceId)
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val rates =
                    if (converted) {
                        store.getAssetPairConvertedRates(
                            assetId,
                            referenceId,
                            period.apiRange,
                            _state.value.isUserAsset,
                        )
                    } else {
                        store.getAssetPairRates(
                            assetId,
                            referenceId,
                            period.apiRange,
                            _state.value.isUserAsset,
                        )
                    }
                val points = rates.map { ChartPoint(date = it.timestamp, value = it.value) }
                _state.update { it.copy(chartByPeriod = it.chartByPeriod + (period to points)) }
            } catch (
                @Suppress("TooGenericExceptionCaught") e: Exception,
            ) {
                _errors.emit(e.friendlyMessage())
            }
        }
    }

    fun pairs(): List<AssetPairRef> = _state.value.detail?.pairs ?: emptyList()

    fun addRate(
        date: Long,
        rate: Double,
    ) = launchCatching(_errors) {
        val referenceId = _state.value.selectedPairId ?: return@launchCatching
        store.addUserAssetRate(assetId, referenceId, date, rate)
        _saved.emit(Unit)
        _state.value = _state.value.copy(chartByPeriod = emptyMap())
        selectPair(referenceId)
    }

    fun addPair(referenceId: Int) =
        launchCatching(_errors) {
            store.addUserAssetPair(assetId, referenceId)
            reload(_state.value.isUserAsset)
        }

    fun delete(onDone: () -> Unit) =
        launchCatching(_errors) {
            store.deleteUserAsset(assetId)
            withContext(Dispatchers.Main) { onDone() }
        }
}
