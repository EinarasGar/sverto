package com.sverto.app.core.ai

enum class OnDeviceAiConsent { LOADING, DISABLED, ENABLED }

data class OnDeviceAiSettings(
    val consent: OnDeviceAiConsent = OnDeviceAiConsent.LOADING,
    val saving: Boolean = false,
    val errorMessage: String? = null,
)
