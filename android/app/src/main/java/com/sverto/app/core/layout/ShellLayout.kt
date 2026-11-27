package com.sverto.app.core.layout

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.adaptive.currentWindowAdaptiveInfoV2
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.window.core.layout.WindowSizeClass

enum class ShellLayout {
    Compact,
    Medium,
    Expanded,
}

@Suppress("CompositionLocalAllowlist")
val LocalShellLayout = staticCompositionLocalOf { ShellLayout.Compact }

val NarrowPaneWidth = 640.dp
val StandardPaneWidth = 840.dp
val SearchPillMaxWidth = 560.dp

@Composable
fun currentShellLayout(): ShellLayout {
    val sizeClass = currentWindowAdaptiveInfoV2().windowSizeClass
    return when {
        !sizeClass.isWidthAtLeastBreakpoint(WindowSizeClass.WIDTH_DP_MEDIUM_LOWER_BOUND) ||
            !sizeClass.isHeightAtLeastBreakpoint(WindowSizeClass.HEIGHT_DP_MEDIUM_LOWER_BOUND) -> ShellLayout.Compact
        sizeClass.isWidthAtLeastBreakpoint(WindowSizeClass.WIDTH_DP_EXPANDED_LOWER_BOUND) -> ShellLayout.Expanded
        else -> ShellLayout.Medium
    }
}

@Composable
fun chartHeight(): Dp = if (LocalShellLayout.current == ShellLayout.Compact) 180.dp else 240.dp

@Composable
fun CappedPane(
    maxWidth: Dp,
    backdrop: Color,
    modifier: Modifier = Modifier,
    content: @Composable BoxScope.() -> Unit,
) {
    val compact = LocalShellLayout.current == ShellLayout.Compact
    Box(
        modifier =
            modifier.fillMaxSize().then(
                if (compact) {
                    Modifier
                } else {
                    Modifier
                        .background(backdrop)
                        .windowInsetsPadding(WindowInsets.displayCutout.only(WindowInsetsSides.Horizontal))
                },
            ),
        contentAlignment = Alignment.TopCenter,
    ) {
        Box(
            modifier = Modifier.then(if (compact) Modifier else Modifier.widthIn(max = maxWidth)).fillMaxSize(),
            content = content,
        )
    }
}

@Composable
fun NarrowPane(content: @Composable BoxScope.() -> Unit) {
    CappedPane(maxWidth = NarrowPaneWidth, backdrop = MaterialTheme.colorScheme.background, content = content)
}
