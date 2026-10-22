package com.sverto.app.feature.settings

import com.sverto.app.core.ai.OnDeviceAiConsent
import com.sverto.app.core.ai.OnDeviceAiSettings
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

class OnDeviceAiSettingController(
    private val setEnabled: (Boolean) -> Unit,
) {
    private val _consentVisible = MutableStateFlow(false)
    val consentVisible: StateFlow<Boolean> = _consentVisible.asStateFlow()

    fun turnOn() {
        _consentVisible.value = true
    }

    fun confirmTurnOn() = setEnabled(true)

    fun notNow() {
        _consentVisible.value = false
    }

    fun turnOff() = setEnabled(false)

    fun onSettingsChanged(settings: OnDeviceAiSettings) {
        if (settings.consent == OnDeviceAiConsent.ENABLED) _consentVisible.value = false
    }
}
