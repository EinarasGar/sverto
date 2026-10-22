package com.sverto.app.feature.settings

data class AiFeaturesSettingState(
    val enabled: Boolean,
    val saving: Boolean = false,
    val errorMessage: String? = null,
)
