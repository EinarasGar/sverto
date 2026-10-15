package com.sverto.app.core.update

import android.content.Context
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.longPreferencesKey
import kotlinx.coroutines.flow.first
import java.io.File
import java.io.IOException
import java.time.Instant

class UpdatePreferences(
    context: Context,
) {
    private val dataStore =
        PreferenceDataStoreFactory.create(
            corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
        ) {
            File(context.noBackupFilesDir, "app_updates.preferences_pb")
        }
    private val autoPromptedVersionCodeKey = intPreferencesKey("auto_prompted_version_code")
    private val dismissedVersionCodeKey = intPreferencesKey("dismissed_version_code")
    private val dismissedAtKey = longPreferencesKey("dismissed_at_epoch_millis")

    suspend fun autoPromptedVersionCode(): Int? =
        runCatching { dataStore.data.first()[autoPromptedVersionCodeKey] }
            .onFailure { if (it !is IOException) throw it }
            .getOrNull()

    suspend fun setAutoPromptedVersionCode(versionCode: Int) {
        runCatching { dataStore.edit { it[autoPromptedVersionCodeKey] = versionCode } }
            .onFailure { if (it !is IOException) throw it }
    }

    suspend fun bannerDismissal(): BannerDismissal? =
        runCatching {
            val preferences = dataStore.data.first()
            val versionCode = preferences[dismissedVersionCodeKey]
            val dismissedAt = preferences[dismissedAtKey]
            if (versionCode != null && dismissedAt != null) {
                BannerDismissal(versionCode, Instant.ofEpochMilli(dismissedAt))
            } else {
                null
            }
        }.onFailure { if (it !is IOException) throw it }
            .getOrNull()

    suspend fun setBannerDismissal(dismissal: BannerDismissal) {
        runCatching {
            dataStore.edit {
                it[dismissedVersionCodeKey] = dismissal.versionCode
                it[dismissedAtKey] = dismissal.dismissedAt.toEpochMilli()
            }
        }.onFailure { if (it !is IOException) throw it }
    }
}
