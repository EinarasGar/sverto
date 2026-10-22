package com.sverto.app.core.aifeatures

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import uniffi.sverto_core.AppStore

@Composable
fun rememberAiFeaturesGate(store: AppStore): AiFeaturesGate =
    remember(store) {
        AiFeaturesGate(
            isEnabled = store::getAiFeaturesEnabled,
            enable = { store.setAiFeaturesEnabled(true) },
        )
    }

@Composable
fun AiFeaturesDisclosure(modifier: Modifier = Modifier) {
    val uriHandler = LocalUriHandler.current
    Column(
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(AiFeaturesCopy.INTRO)
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            AiFeaturesCopy.SHARED_DATA.forEach { item ->
                Row {
                    Text("•", modifier = Modifier.width(20.dp))
                    Text(item)
                }
            }
        }
        Text(AiFeaturesCopy.FOOTER)
        TextButton(onClick = { uriHandler.openUri(AiFeaturesCopy.PRIVACY_POLICY_URL) }) {
            Text(AiFeaturesCopy.PRIVACY_POLICY)
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun AiFeaturesConsentDialog(gate: AiFeaturesGate) {
    val consent by gate.consent.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val state = consent ?: return

    AlertDialog(
        onDismissRequest = { if (!state.isEnabling) gate.notNow() },
        title = { Text(AiFeaturesCopy.POPUP_TITLE) },
        text = {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                AiFeaturesDisclosure()
                state.errorMessage?.let { message ->
                    Text(message, color = MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = !state.isEnabling,
                onClick = { scope.launch { gate.turnOn() } },
            ) {
                if (state.isEnabling) {
                    LoadingIndicator(modifier = Modifier.size(24.dp))
                } else {
                    Text(AiFeaturesCopy.ENABLE)
                }
            }
        },
        dismissButton = {
            TextButton(
                enabled = !state.isEnabling,
                onClick = gate::notNow,
            ) {
                Text(AiFeaturesCopy.POPUP_DECLINE)
            }
        },
    )
}
