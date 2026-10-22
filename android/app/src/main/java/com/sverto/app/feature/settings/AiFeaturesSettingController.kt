package com.sverto.app.feature.settings

import com.sverto.app.core.aifeatures.AiFeaturesGate
import com.sverto.app.core.friendlyMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.sverto_core.ApiException

class AiFeaturesSettingController(
    private val gate: AiFeaturesGate,
    private val isEnabled: () -> Boolean,
    private val disable: suspend () -> Unit,
) {
    private val _state = MutableStateFlow(AiFeaturesSettingState(enabled = isEnabled()))
    val state: StateFlow<AiFeaturesSettingState> = _state.asStateFlow()

    fun refresh() {
        if (!_state.value.saving) _state.value = _state.value.copy(enabled = isEnabled())
    }

    fun turnOn() {
        _state.value = _state.value.copy(errorMessage = null)
        gate.promptToEnable(::refresh)
    }

    suspend fun turnOff() {
        _state.value = AiFeaturesSettingState(enabled = false, saving = true)
        try {
            disable()
        } catch (e: ApiException) {
            _state.value = AiFeaturesSettingState(enabled = isEnabled(), errorMessage = e.friendlyMessage())
            return
        }
        _state.value = AiFeaturesSettingState(enabled = isEnabled())
    }
}
