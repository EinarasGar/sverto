package com.sverto.app.core.aifeatures

import com.sverto.app.core.friendlyMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.sverto_core.ApiException

class AiFeaturesGate(
    private val isEnabled: () -> Boolean,
    private val enable: suspend () -> Unit,
) {
    private val _consent = MutableStateFlow<AiFeaturesConsentState?>(null)
    val consent: StateFlow<AiFeaturesConsentState?> = _consent.asStateFlow()

    private var pendingAction: (() -> Unit)? = null

    fun runWhenEnabled(action: () -> Unit) {
        if (isEnabled()) action() else promptToEnable(action)
    }

    fun promptToEnable(action: () -> Unit) {
        pendingAction = action
        _consent.value = AiFeaturesConsentState()
    }

    suspend fun turnOn() {
        val action = pendingAction ?: return
        _consent.value = AiFeaturesConsentState(isEnabling = true)
        try {
            enable()
        } catch (e: ApiException) {
            _consent.value = AiFeaturesConsentState(errorMessage = e.friendlyMessage())
            return
        }
        dismiss()
        action()
    }

    fun notNow() = dismiss()

    private fun dismiss() {
        pendingAction = null
        _consent.value = null
    }
}
