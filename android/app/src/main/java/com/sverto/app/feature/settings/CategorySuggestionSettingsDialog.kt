package com.sverto.app.feature.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sverto.app.core.ai.CategorySuggester
import com.sverto.app.core.ai.CategorySuggestionConsent

@Composable
fun CategorySuggestionSettingsDialog(
    categorySuggester: CategorySuggester,
    onDismiss: () -> Unit,
) {
    val settings by categorySuggester.settings.collectAsStateWithLifecycle()
    val enabled = settings.consent == CategorySuggestionConsent.ENABLED
    val busy = settings.saving || settings.consent == CategorySuggestionConsent.LOADING
    val uriHandler = LocalUriHandler.current
    var closeAfterSaving by remember { mutableStateOf(false) }

    LaunchedEffect(settings) {
        if (closeAfterSaving && !settings.saving) {
            closeAfterSaving = false
            if (settings.errorMessage == null) onDismiss()
        }
    }

    AlertDialog(
        onDismissRequest = { if (!settings.saving) onDismiss() },
        title = { Text("Category suggestions") },
        text = {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Text(
                    text = if (enabled) "On for this device" else "Off for this device",
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    "Google ML Kit uses your transaction description and category names on this device " +
                        "to suggest a category. ML Kit does not send those inputs or suggestions to Google.",
                )
                Text(
                    "Google receives device and app information, identifiers, and ML Kit usage and performance data. " +
                        "Google uses this data to operate, maintain and improve its APIs and detect abuse. " +
                        "Enabling may download an AI model.",
                )
                Text(
                    "This is optional. You can choose categories yourself and turn suggestions off any time here. " +
                        "Turning off stops new suggestions; it does not delete data Google already received " +
                        "or stop delivery of metrics already queued.",
                )
                TextButton(onClick = { uriHandler.openUri("https://developers.google.com/ml-kit/terms") }) {
                    Text("Google ML Kit privacy")
                }
                settings.errorMessage?.let { message ->
                    Text(message, color = MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = !busy,
                onClick = {
                    closeAfterSaving = true
                    categorySuggester.setEnabled(!enabled)
                },
            ) {
                Text(
                    if (settings.saving) {
                        "Saving…"
                    } else if (enabled) {
                        "Turn off"
                    } else {
                        "Enable"
                    },
                )
            }
        },
        dismissButton = {
            TextButton(
                enabled = !busy,
                onClick = {
                    if (enabled) {
                        onDismiss()
                    } else {
                        closeAfterSaving = true
                        categorySuggester.setEnabled(false)
                    }
                },
            ) {
                Text(if (enabled) "Done" else "Keep off")
            }
        },
    )
}
