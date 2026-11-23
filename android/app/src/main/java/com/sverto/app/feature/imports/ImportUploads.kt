package com.sverto.app.feature.imports

import android.content.ContentResolver
import android.net.Uri
import android.provider.OpenableColumns
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.IOException

const val MAX_IMPORT_FILE_BYTES = 20L * 1024 * 1024
private const val MAX_FILE_NAME_LENGTH = 255
private const val UNKNOWN_MIME = "application/octet-stream"

class PickedFile(
    val name: String,
    val mimeType: String,
    val bytes: ByteArray,
)

data class LocalUpload(
    val id: String,
    val name: String,
    val failed: Boolean = false,
    val error: String? = null,
    val fileId: String? = null,
    val attachedFileId: String? = null,
)

sealed interface PickResult {
    class Picked(
        val file: PickedFile,
    ) : PickResult

    data class TooLarge(
        val name: String,
    ) : PickResult

    data class Unreadable(
        val name: String,
    ) : PickResult
}

suspend fun readPickedFile(
    resolver: ContentResolver,
    uri: Uri,
): PickResult =
    withContext(Dispatchers.IO) {
        var name: String? = null
        var size: Long? = null
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use {
            if (it.moveToFirst()) {
                name = it.getString(0)
                size = if (it.isNull(1)) null else it.getLong(1)
            }
        }
        val safeName =
            (name ?: uri.lastPathSegment ?: "statement")
                .replace('/', '_')
                .replace('\\', '_')
                .ifBlank { "statement" }
                .take(MAX_FILE_NAME_LENGTH)
        if ((size ?: 0L) > MAX_IMPORT_FILE_BYTES) return@withContext PickResult.TooLarge(safeName)
        val bytes =
            try {
                resolver.openInputStream(uri)?.use { it.readBytes() }
            } catch (_: IOException) {
                null
            } catch (_: SecurityException) {
                null
            }
        when {
            bytes == null || bytes.isEmpty() -> PickResult.Unreadable(safeName)
            bytes.size > MAX_IMPORT_FILE_BYTES -> PickResult.TooLarge(safeName)
            else -> PickResult.Picked(PickedFile(safeName, mimeTypeOf(resolver, uri), bytes))
        }
    }

private fun mimeTypeOf(
    resolver: ContentResolver,
    uri: Uri,
): String =
    resolver
        .getType(uri)
        ?.substringBefore(';')
        ?.trim()
        ?.takeIf { type -> type.count { it == '/' } == 1 }
        ?: UNKNOWN_MIME
