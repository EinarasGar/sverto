package com.sverto.app.core.ui

import androidx.compose.material3.pulltorefresh.PullToRefreshState
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import kotlinx.coroutines.flow.filter

@Composable
fun rememberHapticPullToRefreshState(): PullToRefreshState {
    val state = rememberPullToRefreshState()
    val haptics = LocalHapticFeedback.current
    LaunchedEffect(state, haptics) {
        // The indicator settles at exactly 1 while refreshing; only a finger pulls it past, so a refresh
        // started in code stays silent.
        snapshotFlow { state.distanceFraction > 1f && !state.isAnimating }
            .filter { it }
            .collect { haptics.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate) }
    }
    return state
}
