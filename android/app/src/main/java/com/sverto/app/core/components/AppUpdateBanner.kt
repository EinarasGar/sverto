package com.sverto.app.core.components

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sverto.app.SvertoApp
import com.sverto.app.core.icons.LucideIcon
import com.sverto.app.core.update.InstallState
import kotlin.math.roundToInt

private const val PERCENT = 100

@Composable
fun AppUpdateBanner(
    placement: UpdateBannerPlacement,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val appUpdates = remember { (context.applicationContext as SvertoApp).appUpdates }
    val presentation by appUpdates.presentation.collectAsStateWithLifecycle()
    val visible =
        when (placement) {
            UpdateBannerPlacement.SETTINGS -> presentation.showSettingsBanner
            UpdateBannerPlacement.HOME -> presentation.showHomeBanner
        }
    AnimatedVisibility(
        visible = visible,
        exit =
            fadeOut(MaterialTheme.motionScheme.fastEffectsSpec()) +
                shrinkVertically(MaterialTheme.motionScheme.defaultSpatialSpec()),
    ) {
        UpdateBanner(
            installState = presentation.installState,
            onClick = appUpdates::onBannerClick,
            onDismiss = if (placement == UpdateBannerPlacement.HOME) appUpdates::dismissHomeBanner else null,
            modifier = modifier,
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun UpdateBanner(
    installState: InstallState,
    onClick: () -> Unit,
    onDismiss: (() -> Unit)?,
    modifier: Modifier = Modifier,
) {
    val enterSpec = MaterialTheme.motionScheme.defaultEffectsSpec<Float>()
    val exitSpec = MaterialTheme.motionScheme.fastEffectsSpec<Float>()
    Surface(
        onClick = onClick,
        enabled = installState !is InstallState.Downloading,
        shape = MaterialTheme.shapes.extraLarge,
        color = MaterialTheme.colorScheme.primaryContainer,
        contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
        modifier =
            modifier
                .fillMaxWidth()
                .animateContentSize(MaterialTheme.motionScheme.defaultSpatialSpec()),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Box(
                modifier =
                    Modifier
                        .size(44.dp)
                        .clip(MaterialShapes.Cookie9Sided.toShape())
                        .background(MaterialTheme.colorScheme.primary),
                contentAlignment = Alignment.Center,
            ) {
                LucideIcon(
                    name = if (installState == InstallState.Downloaded) "refresh-cw" else "download",
                    tint = MaterialTheme.colorScheme.onPrimary,
                    modifier = Modifier.size(22.dp),
                )
            }
            AnimatedContent(
                targetState = installState,
                contentKey = { it::class },
                transitionSpec = {
                    fadeIn(enterSpec) togetherWith fadeOut(exitSpec)
                },
                modifier = Modifier.weight(1f),
                label = "updateBannerContent",
            ) { state ->
                when (state) {
                    InstallState.Idle ->
                        BannerText(title = "Update available", supporting = "Tap to download the new version")
                    is InstallState.Downloading -> DownloadingContent(progress = state.progress)
                    InstallState.Downloaded ->
                        BannerText(title = "Update ready", supporting = "Tap to restart and finish")
                }
            }
            when {
                onDismiss != null && installState == InstallState.Idle ->
                    IconButton(
                        onClick = onDismiss,
                        modifier = Modifier.semantics { contentDescription = "Dismiss update banner" },
                    ) {
                        LucideIcon(
                            name = "x",
                            tint = MaterialTheme.colorScheme.onPrimaryContainer,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                installState !is InstallState.Downloading ->
                    LucideIcon(
                        name = "chevron-right",
                        tint = MaterialTheme.colorScheme.onPrimaryContainer,
                        modifier = Modifier.size(20.dp),
                    )
            }
        }
    }
}

@Composable
private fun BannerText(
    title: String,
    supporting: String,
) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        Text(
            text = supporting,
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun DownloadingContent(progress: Float?) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = "Downloading update",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.weight(1f),
            )
            if (progress != null) {
                Text(
                    text = "${(progress * PERCENT).roundToInt()}%",
                    style = MaterialTheme.typography.labelLarge,
                )
            }
        }
        if (progress != null) {
            LinearWavyProgressIndicator(
                progress = { progress },
                color = MaterialTheme.colorScheme.primary,
                trackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                modifier = Modifier.fillMaxWidth(),
            )
        } else {
            LinearWavyProgressIndicator(
                color = MaterialTheme.colorScheme.primary,
                trackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}
