@file:Suppress("MatchingDeclarationName")
@file:OptIn(ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports.review

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.sverto.app.core.icons.LucideIcon
import com.sverto.app.feature.connectors.HeroButton

enum class ApplyBarMode { Apply, Navigate, Disabled }

@Composable
fun ApplyBar(
    mode: ApplyBarMode,
    label: String,
    supporting: String?,
    applying: Boolean,
    busy: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
        color = MaterialTheme.colorScheme.surfaceBright,
        shadowElevation = 3.dp,
    ) {
        Column(
            modifier =
                Modifier
                    .navigationBarsPadding()
                    .padding(start = 24.dp, end = 24.dp, top = 16.dp, bottom = 12.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            supporting?.let {
                Text(
                    text = it,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
            }
            val buttonModifier = Modifier.fillMaxWidth()
            when (mode) {
                ApplyBarMode.Navigate ->
                    FilledTonalButton(
                        onClick = onClick,
                        enabled = !busy,
                        shapes = ButtonDefaults.shapes(),
                        contentPadding = ButtonDefaults.MediumContentPadding,
                        modifier = buttonModifier.heightIn(min = ButtonDefaults.MediumContainerHeight),
                    ) {
                        Text(label, style = MaterialTheme.typography.titleMedium)
                        Spacer(Modifier.width(8.dp))
                        LucideIcon(name = "arrow-down", tint = LocalContentColor.current, modifier = Modifier.size(20.dp))
                    }
                ApplyBarMode.Apply ->
                    HeroButton(
                        text = if (applying) "Applying…" else label,
                        onClick = onClick,
                        enabled = !busy,
                        icon = "check",
                        modifier = buttonModifier,
                    )
                ApplyBarMode.Disabled ->
                    HeroButton(
                        text = label,
                        onClick = onClick,
                        enabled = false,
                        icon = "check",
                        modifier = buttonModifier,
                    )
            }
        }
    }
}

@Composable
fun ApplyConfirmDialog(
    summary: String,
    warnings: List<String>,
    risky: Boolean,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        modifier = modifier,
        title = { Text("Apply this import?") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(summary)
                warnings.forEach { Text(it, color = MaterialTheme.colorScheme.error) }
            }
        },
        confirmButton = {
            if (risky) {
                Button(onClick = onConfirm, shapes = ButtonDefaults.shapes()) { Text("Apply") }
            } else {
                TextButton(onClick = onConfirm, shapes = ButtonDefaults.shapes()) { Text("Apply") }
            }
        },
        dismissButton = { TextButton(onClick = onDismiss, shapes = ButtonDefaults.shapes()) { Text("Keep reviewing") } },
    )
}
