package com.sverto.app.feature.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.unit.dp
import com.sverto.app.core.ai.OnDeviceAiConsent
import com.sverto.app.core.ai.OnDeviceAiCopy
import com.sverto.app.core.ai.OnDeviceAiSettings

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun OnDeviceAiConsentDialog(
    settings: OnDeviceAiSettings,
    onTurnOn: () -> Unit,
    onNotNow: () -> Unit,
) {
    val busy = settings.saving || settings.consent == OnDeviceAiConsent.LOADING
    val uriHandler = LocalUriHandler.current

    AlertDialog(
        onDismissRequest = { if (!settings.saving) onNotNow() },
        title = { Text(OnDeviceAiCopy.TITLE) },
        text = {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Text(OnDeviceAiCopy.INPUTS)
                Text(OnDeviceAiCopy.TELEMETRY)
                Text(OnDeviceAiCopy.OPTIONAL)
                TextButton(onClick = { uriHandler.openUri(OnDeviceAiCopy.PRIVACY_URL) }) {
                    Text(OnDeviceAiCopy.PRIVACY)
                }
                settings.errorMessage?.let { message ->
                    Text(message, color = MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            TextButton(enabled = !busy, onClick = onTurnOn) {
                if (settings.saving) {
                    LoadingIndicator(modifier = Modifier.size(24.dp))
                } else {
                    Text(OnDeviceAiCopy.ENABLE)
                }
            }
        },
        dismissButton = {
            TextButton(enabled = !busy, onClick = onNotNow) {
                Text(OnDeviceAiCopy.DECLINE)
            }
        },
    )
}
