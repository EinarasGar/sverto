package com.sverto.app.core.update

sealed interface InstallState {
    data object Idle : InstallState

    data class Downloading(
        val progress: Float?,
    ) : InstallState

    data object Downloaded : InstallState
}
