@file:Suppress("MatchingDeclarationName")

package com.sverto.app.feature.imports.review

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.sverto.app.core.icons.LucideIcon

data class StatusHeaderState(
    val label: String,
    val summary: String,
    val tone: ImportTone,
    val icon: String,
    val working: Boolean,
    val error: String? = null,
)

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ImportStatusHeader(
    state: StatusHeaderState,
    modifier: Modifier = Modifier,
    retryLabel: String? = null,
    onRetry: () -> Unit = {},
) {
    val effects = MaterialTheme.motionScheme.defaultEffectsSpec<Color>()
    val fadeInSpec = MaterialTheme.motionScheme.defaultEffectsSpec<Float>()
    val fadeOutSpec = MaterialTheme.motionScheme.fastEffectsSpec<Float>()
    val container by animateColorAsState(state.tone.containerColor(), effects, label = "headerContainer")
    val content by animateColorAsState(state.tone.contentColor(), effects, label = "headerContent")
    Column(
        modifier =
            modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp, vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier =
                Modifier
                    .size(72.dp)
                    .clip(MaterialShapes.Cookie9Sided.toShape())
                    .background(container),
            contentAlignment = Alignment.Center,
        ) {
            AnimatedContent(
                targetState = state.working to state.icon,
                transitionSpec = {
                    (fadeIn(fadeInSpec) + scaleIn(fadeInSpec, initialScale = 0.8f)).togetherWith(fadeOut(fadeOutSpec))
                },
                label = "headerGlyph",
            ) { (working, icon) ->
                if (working) {
                    LoadingIndicator(
                        color = content,
                        modifier =
                            Modifier
                                .size(48.dp)
                                .semantics { contentDescription = state.label },
                    )
                } else {
                    LucideIcon(name = icon, tint = content, modifier = Modifier.size(32.dp))
                }
            }
        }
        Spacer(Modifier.height(16.dp))
        Text(
            text = state.label,
            style = MaterialTheme.typography.headlineSmall,
            textAlign = TextAlign.Center,
        )
        Spacer(Modifier.height(6.dp))
        Text(
            text = state.summary,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        state.error?.let { error ->
            Spacer(Modifier.height(8.dp))
            Text(
                text = error,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.error,
                textAlign = TextAlign.Center,
            )
        }
        if (retryLabel != null) {
            Spacer(Modifier.height(16.dp))
            FilledTonalButton(onClick = onRetry, shapes = ButtonDefaults.shapes()) {
                LucideIcon(
                    name = "rotate-cw",
                    tint = MaterialTheme.colorScheme.onSecondaryContainer,
                    modifier = Modifier.size(ButtonDefaults.IconSize),
                )
                Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                Text(retryLabel)
            }
        }
    }
}
