package com.sverto.app.core.update

data class UpdatePresentation(
    val showBadge: Boolean = false,
    val showSettingsBanner: Boolean = false,
    val showHomeBanner: Boolean = false,
    val installState: InstallState = InstallState.Idle,
    val autoPromptFlexible: Boolean = false,
    val blocking: Boolean = false,
)
