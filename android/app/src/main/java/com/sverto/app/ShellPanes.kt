package com.sverto.app

import androidx.activity.ComponentActivity
import androidx.activity.compose.LocalActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ScaffoldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.adaptive.allVerticalHingeBounds
import androidx.compose.material3.adaptive.currentWindowAdaptiveInfoV2
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.viewmodel.compose.LocalViewModelStoreOwner
import androidx.navigation.NavBackStackEntry
import androidx.window.core.layout.WindowSizeClass
import com.sverto.app.core.layout.NarrowPaneWidth
import com.sverto.app.core.layout.StandardPaneWidth
import com.sverto.app.core.navigation.TopLevelRoute

private val focusRoutePrefixes =
    listOf(
        "addAccount",
        "editAccount/",
        "addProperty",
        "createTransaction",
        "editTransaction",
        "groupAddTransaction/",
        "groupEditTransaction/",
        "connectTrueLayer",
        "connectTrading212",
        "connectEnableBanking",
        "bindingSetup/",
        "importSession/",
        "importItems/",
    )

internal fun isFocusRoute(route: String?): Boolean = route != null && focusRoutePrefixes.any { route.startsWith(it) }

internal fun paneMaxWidth(route: String?): Dp =
    when {
        route == TopLevelRoute.Portfolio.route -> Dp.Unspecified
        isFocusRoute(route) || route == "settings" || route == TopLevelRoute.AiChat.route -> NarrowPaneWidth
        else -> StandardPaneWidth
    }

@Composable
internal fun railEndToVerticalHinge(): Dp? {
    val hinge = currentWindowAdaptiveInfoV2().windowPosture.allVerticalHingeBounds.firstOrNull() ?: return null
    val direction = LocalLayoutDirection.current
    val hingeFromStart = if (direction == LayoutDirection.Ltr) hinge.left else LocalWindowInfo.current.containerSize.width - hinge.right
    val railEnd = RailWidth + railInsets().asPaddingValues().calculateStartPadding(direction)
    return with(LocalDensity.current) { hingeFromStart.toDp() } - railEnd
}

@Composable
internal fun listPaneWidth(): Dp {
    val large =
        currentWindowAdaptiveInfoV2().windowSizeClass.isWidthAtLeastBreakpoint(WindowSizeClass.WIDTH_DP_LARGE_LOWER_BOUND)
    val atHinge = railEndToVerticalHinge()
    return when {
        atHinge != null && atHinge >= 280.dp -> atHinge
        large -> 412.dp
        else -> 360.dp
    }
}

@Composable
internal fun ListPane(
    rootEntry: NavBackStackEntry,
    width: Dp,
    topBar: @Composable () -> Unit,
    content: @Composable (PaddingValues) -> Unit,
) {
    val activity = LocalActivity.current as ComponentActivity
    CompositionLocalProvider(
        LocalViewModelStoreOwner provides rootEntry,
        LocalLifecycleOwner provides activity,
    ) {
        Scaffold(
            modifier =
                Modifier
                    .width(width)
                    .fillMaxHeight()
                    .consumeWindowInsets(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal)),
            containerColor = MaterialTheme.colorScheme.surfaceContainer,
            topBar = topBar,
            contentWindowInsets = ScaffoldDefaults.contentWindowInsets.only(WindowInsetsSides.Vertical),
        ) { padding -> content(padding) }
    }
}

@Composable
internal fun PanePlaceholder(text: String) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(
            text = text,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
