package com.sverto.app.core.ai

import android.content.Context
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import kotlinx.coroutines.flow.first
import java.io.File

class OnDeviceAiPreferences(
    context: Context,
) {
    private val dataStore =
        PreferenceDataStoreFactory.create {
            // Legacy name from when this only gated category suggestions; renaming it would reset existing users' choice.
            File(context.noBackupFilesDir, "category_suggestions.preferences_pb")
        }
    private val enabledKey = booleanPreferencesKey("enabled")

    suspend fun isEnabled(): Boolean = dataStore.data.first()[enabledKey] ?: false

    suspend fun setEnabled(enabled: Boolean) {
        dataStore.edit { it[enabledKey] = enabled }
    }
}
