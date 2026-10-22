package com.sverto.app.feature.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sverto.app.core.ai.CategorySuggester
import com.sverto.app.core.ai.OnDeviceAiConsent
import com.sverto.app.core.ai.OnDeviceAiCopy
import com.sverto.app.core.aifeatures.AiFeaturesCopy
import com.sverto.app.core.aifeatures.AiFeaturesGate
import kotlinx.coroutines.launch
import uniffi.sverto_core.AppStore

@Composable
fun AiSettingsSection(
    gate: AiFeaturesGate,
    store: AppStore,
    categorySuggester: CategorySuggester,
    modifier: Modifier = Modifier,
    usage: @Composable () -> Unit,
) {
    val aiFeatures =
        remember(gate, store) {
            AiFeaturesSettingController(
                gate = gate,
                isEnabled = store::getAiFeaturesEnabled,
                disable = { store.setAiFeaturesEnabled(false) },
            )
        }
    val aiFeaturesState by aiFeatures.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner, aiFeatures) {
        val observer =
            LifecycleEventObserver { _, event ->
                if (event == Lifecycle.Event.ON_RESUME) aiFeatures.refresh()
            }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    Column(modifier = modifier) {
        HorizontalDivider(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            color = MaterialTheme.colorScheme.outlineVariant,
        )
        SettingSwitchRow(
            icon = "bot-message-square",
            title = AiFeaturesCopy.TITLE,
            description = AiFeaturesCopy.SETTING_DESCRIPTION,
            checked = aiFeaturesState.enabled,
            enabled = !aiFeaturesState.saving,
            errorMessage = aiFeaturesState.errorMessage,
            onCheckedChange = { checked ->
                if (checked) aiFeatures.turnOn() else scope.launch { aiFeatures.turnOff() }
            },
        )
        OnDeviceAiSettingRow(categorySuggester = categorySuggester)
        if (aiFeaturesState.enabled) usage()
    }
}

@Composable
private fun OnDeviceAiSettingRow(categorySuggester: CategorySuggester) {
    val settings by categorySuggester.settings.collectAsStateWithLifecycle()
    val controller = remember(categorySuggester) { OnDeviceAiSettingController(categorySuggester::setEnabled) }
    val consentVisible by controller.consentVisible.collectAsStateWithLifecycle()

    LaunchedEffect(controller) {
        categorySuggester.settings.collect(controller::onSettingsChanged)
    }

    SettingSwitchRow(
        icon = "smartphone",
        title = OnDeviceAiCopy.TITLE,
        description = OnDeviceAiCopy.SETTING_DESCRIPTION,
        checked = settings.consent == OnDeviceAiConsent.ENABLED,
        enabled = !settings.saving && settings.consent != OnDeviceAiConsent.LOADING,
        errorMessage = settings.errorMessage,
        onCheckedChange = { checked -> if (checked) controller.turnOn() else controller.turnOff() },
    )

    if (consentVisible) {
        OnDeviceAiConsentDialog(
            settings = settings,
            onTurnOn = controller::confirmTurnOn,
            onNotNow = controller::notNow,
        )
    }
}
