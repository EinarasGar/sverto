package com.sverto.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material3.CenterAlignedTopAppBar
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SearchBarState
import androidx.compose.material3.ShortNavigationBar
import androidx.compose.material3.ShortNavigationBarItem
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.WideNavigationRail
import androidx.compose.material3.WideNavigationRailDefaults
import androidx.compose.material3.WideNavigationRailItem
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.navigation.NavController
import com.sverto.app.core.components.ProfileAvatarButton
import com.sverto.app.core.navigation.TopLevelRoute
import com.sverto.app.feature.assets.AssetSearchAppBar
import com.sverto.app.feature.transactions.TransactionSearchAppBar
import uniffi.sverto_core.TransactionListItem

internal val RailWidth = 96.dp

@Composable
internal fun railInsets(): WindowInsets =
    WindowInsets.systemBars
        .union(WindowInsets.displayCutout)
        .only(WindowInsetsSides.Vertical + WindowInsetsSides.Start)

// Every tab switch pops to the start destination, so at most one tab root other than Portfolio is
// ever on the back stack, and it is the one being shown.
internal fun NavController.currentTab(): TopLevelRoute =
    TopLevelRoute.entries.lastOrNull { tab ->
        tab != TopLevelRoute.Portfolio && runCatching { getBackStackEntry(tab.route) }.isSuccess
    } ?: TopLevelRoute.Portfolio

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun ShellRail(
    selected: TopLevelRoute,
    onSelect: (TopLevelRoute) -> Unit,
) {
    WideNavigationRail(
        colors = WideNavigationRailDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        windowInsets = railInsets(),
        arrangement = Arrangement.Center,
    ) {
        TopLevelRoute.entries.forEach { route ->
            WideNavigationRailItem(
                selected = route == selected,
                onClick = { onSelect(route) },
                icon = {
                    Icon(
                        imageVector = if (route == selected) route.selectedIcon else route.unselectedIcon,
                        contentDescription = null,
                    )
                },
                label = { Text(route.label) },
                railExpanded = false,
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun ShellTopBar(
    route: String?,
    showMenu: Boolean,
    onMenu: () -> Unit,
    onAssetClick: (Int) -> Unit,
    transactionSearchBarState: SearchBarState,
    transactionSearchTextState: TextFieldState,
    transactionSearchListState: LazyListState,
    onTransactionClick: (TransactionListItem) -> Unit,
    onProfile: () -> Unit,
    showUpdateBadge: Boolean,
) {
    CenterAlignedTopAppBar(
        navigationIcon = {
            if (showMenu) {
                IconButton(onClick = onMenu) {
                    Icon(Icons.Default.Menu, contentDescription = "Menu")
                }
            } else if (route == TopLevelRoute.Portfolio.route ||
                route == TopLevelRoute.Transactions.route
            ) {
                // Decorative logo sized like an IconButton so the
                // app-bar layout matches the other tabs, without a
                // ripple-emitting dead button.
                Box(
                    modifier = Modifier.size(48.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        painter = painterResource(id = R.drawable.ic_sverto_logo),
                        contentDescription = "Sverto",
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.height(24.dp),
                    )
                }
            }
        },
        title = {
            when (route) {
                TopLevelRoute.Portfolio.route ->
                    AssetSearchAppBar(onAssetClick = onAssetClick)

                TopLevelRoute.Transactions.route ->
                    TransactionSearchAppBar(
                        searchBarState = transactionSearchBarState,
                        textFieldState = transactionSearchTextState,
                        listState = transactionSearchListState,
                        onTransactionClick = onTransactionClick,
                    )

                else ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(
                            painter = painterResource(id = R.drawable.ic_sverto_logo),
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.height(24.dp),
                        )
                        Spacer(Modifier.width(8.dp))
                        Text("Sverto")
                    }
            }
        },
        actions = {
            ProfileAvatarButton(
                onClick = onProfile,
                showUpdateBadge = showUpdateBadge,
            )
        },
        colors =
            TopAppBarDefaults.topAppBarColors(
                containerColor = MaterialTheme.colorScheme.surfaceContainer,
            ),
    )
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun ShellBottomBar(
    currentRoute: String?,
    onSelect: (TopLevelRoute) -> Unit,
) {
    ShortNavigationBar {
        TopLevelRoute.entries.forEach { route ->
            val selected = currentRoute == route.route
            ShortNavigationBarItem(
                selected = selected,
                onClick = { onSelect(route) },
                icon = {
                    Icon(
                        imageVector =
                            if (selected) {
                                route.selectedIcon
                            } else {
                                route.unselectedIcon
                            },
                        contentDescription = route.label,
                    )
                },
                label = { Text(route.label) },
            )
        }
    }
}
