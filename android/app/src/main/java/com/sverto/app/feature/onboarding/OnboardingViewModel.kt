package com.sverto.app.feature.onboarding

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sverto.app.core.friendlyMessage
import com.sverto.app.feature.assets.components.orderCurrencies
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.sverto_core.AppStore
import uniffi.sverto_core.AssetItem

data class OnboardingUiState(
    val steps: List<OnboardingStepId> = emptyList(),
    val query: String = "",
    val currencyResults: List<AssetItem> = emptyList(),
    val selectedCurrency: AssetItem? = null,
    val isLoadingCurrencies: Boolean = false,
    val isSaving: Boolean = false,
    val finished: Boolean = false,
    val error: String? = null,
)

class OnboardingViewModel(
    private val store: AppStore,
) : ViewModel() {
    private val _state =
        MutableStateFlow(
            OnboardingUiState(steps = stepsForVersion(store.getOnboardingVersion())),
        )

    val state: StateFlow<OnboardingUiState> = _state.asStateFlow()

    private val _aiFeaturesSaved = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    val aiFeaturesSaved: SharedFlow<Unit> = _aiFeaturesSaved.asSharedFlow()

    private var allCurrencies: List<AssetItem> = emptyList()

    init {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoadingCurrencies = true)
            allCurrencies =
                orderCurrencies(runCatching { store.getAllCurrencies() }.getOrDefault(emptyList()))
            val storedDefaultId = store.getDefaultAssetId()
            _state.value =
                _state.value.copy(
                    currencyResults = filtered(_state.value.query),
                    selectedCurrency =
                        _state.value.selectedCurrency
                            ?: storedDefaultId?.let { id -> allCurrencies.firstOrNull { it.id == id } },
                    isLoadingCurrencies = false,
                )
        }
    }

    fun onQueryChange(query: String) {
        _state.value = _state.value.copy(query = query, currencyResults = filtered(query))
    }

    private fun filtered(query: String): List<AssetItem> {
        if (query.isBlank()) return allCurrencies
        return allCurrencies.filter {
            it.ticker.contains(query, ignoreCase = true) || it.name.contains(query, ignoreCase = true)
        }
    }

    fun selectCurrency(item: AssetItem) {
        _state.value = _state.value.copy(selectedCurrency = item)
    }

    fun chooseAiFeatures(enabled: Boolean) {
        viewModelScope.launch {
            _state.value = _state.value.copy(isSaving = true, error = null)
            val result = runCatching { store.setAiFeaturesEnabled(enabled) }
            _state.value =
                result.fold(
                    onSuccess = { _state.value.copy(isSaving = false) },
                    onFailure = { _state.value.copy(isSaving = false, error = it.friendlyMessage()) },
                )
            if (result.isSuccess) _aiFeaturesSaved.emit(Unit)
        }
    }

    fun finish() {
        viewModelScope.launch {
            _state.value = _state.value.copy(isSaving = true, error = null)
            val result =
                runCatching {
                    if (OnboardingStepId.BASE_CURRENCY in _state.value.steps) {
                        _state.value.selectedCurrency?.let { store.updateBaseAsset(it.id, it.ticker) }
                    }
                    store.setOnboardingVersion(CURRENT_ONBOARDING_VERSION)
                }
            _state.value =
                result.fold(
                    onSuccess = { _state.value.copy(isSaving = false, finished = true) },
                    onFailure = { _state.value.copy(isSaving = false, error = it.message) },
                )
        }
    }
}
