package com.sverto.app.feature.transactions.quickupload

import android.content.ContentResolver
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.net.Uri
import android.os.Build
import androidx.core.graphics.scale
import com.sverto.app.feature.imports.PickResult
import com.sverto.app.feature.imports.PickedFile
import com.sverto.app.feature.imports.readPickedFile
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.nio.ByteBuffer

private const val THUMBNAIL_WIDTH = 200
private const val THUMBNAIL_QUALITY = 60
private const val CONVERTED_JPEG_QUALITY = 85
private const val MAX_CONVERTED_EDGE = 3072

// The AI provider rejects images over 5 MB and reads only these formats as they are.
private const val MAX_IMAGE_BYTES = 5 * 1024 * 1024
private val READABLE_IMAGE_TYPES = setOf("image/jpeg", "image/png", "image/gif", "image/webp")

suspend fun prepareQuickUpload(
    resolver: ContentResolver,
    uri: Uri,
): PreparedQuickUpload {
    if (uri.scheme != ContentResolver.SCHEME_CONTENT) return PreparedQuickUpload.Rejected("Couldn't add that file")
    val file =
        when (val picked = readPickedFile(resolver, uri)) {
            is PickResult.Picked -> picked.file
            is PickResult.TooLarge -> return PreparedQuickUpload.Rejected("“${picked.name}” is over 20 MB")
            is PickResult.Unreadable -> return PreparedQuickUpload.Rejected("Couldn't open “${picked.name}”")
        }
    return withContext(Dispatchers.Default) {
        if (file.mimeType == "application/pdf" || file.mimeType.startsWith("text/")) {
            PreparedQuickUpload.Ready(file.bytes, ByteArray(0), file.mimeType)
        } else {
            prepareImage(file)
        }
    }
}

private fun prepareImage(file: PickedFile): PreparedQuickUpload {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeByteArray(file.bytes, 0, file.bytes.size, bounds)
    val imageType = bounds.outMimeType
    if (imageType == null || bounds.outWidth <= 0 || bounds.outHeight <= 0) {
        return if (file.mimeType.startsWith("image/")) {
            PreparedQuickUpload.Rejected("Couldn't read “${file.name}”")
        } else {
            PreparedQuickUpload.Rejected("“${file.name}” isn't a photo, PDF or text file")
        }
    }
    val sendAsIs = imageType in READABLE_IMAGE_TYPES && file.bytes.size <= MAX_IMAGE_BYTES
    val bytes = if (sendAsIs) file.bytes else toJpeg(file.bytes, bounds.outWidth, bounds.outHeight)
    val thumbnail = bytes?.let(::buildThumbnail)
    return if (bytes != null && thumbnail != null) {
        PreparedQuickUpload.Ready(bytes, thumbnail, if (sendAsIs) imageType else "image/jpeg")
    } else {
        PreparedQuickUpload.Rejected("Couldn't read “${file.name}”")
    }
}

private fun toJpeg(
    bytes: ByteArray,
    width: Int,
    height: Int,
): ByteArray? {
    val longEdge = maxOf(width, height)
    val bitmap =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            try {
                ImageDecoder.decodeBitmap(ImageDecoder.createSource(ByteBuffer.wrap(bytes))) { decoder, info, _ ->
                    decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                    if (longEdge > MAX_CONVERTED_EDGE) {
                        decoder.setTargetSize(
                            info.size.width * MAX_CONVERTED_EDGE / longEdge,
                            info.size.height * MAX_CONVERTED_EDGE / longEdge,
                        )
                    }
                }
            } catch (_: IOException) {
                null
            }
        } else {
            val options = BitmapFactory.Options().apply { inSampleSize = sampleSizeFor(longEdge, MAX_CONVERTED_EDGE) }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)
        } ?: return null
    val out = ByteArrayOutputStream()
    bitmap.compress(Bitmap.CompressFormat.JPEG, CONVERTED_JPEG_QUALITY, out)
    bitmap.recycle()
    return out.toByteArray()
}

private fun buildThumbnail(imageBytes: ByteArray): ByteArray? {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeByteArray(imageBytes, 0, imageBytes.size, bounds)
    if (bounds.outWidth <= 0) return null

    val options = BitmapFactory.Options().apply { inSampleSize = sampleSizeFor(bounds.outWidth, THUMBNAIL_WIDTH) }
    val decoded = BitmapFactory.decodeByteArray(imageBytes, 0, imageBytes.size, options) ?: return null
    val thumbHeight = (decoded.height * THUMBNAIL_WIDTH / decoded.width.coerceAtLeast(1)).coerceAtLeast(1)
    val thumbnail = decoded.scale(THUMBNAIL_WIDTH, thumbHeight)

    val out = ByteArrayOutputStream()
    thumbnail.compress(Bitmap.CompressFormat.JPEG, THUMBNAIL_QUALITY, out)
    if (thumbnail !== decoded) decoded.recycle()
    thumbnail.recycle()
    return out.toByteArray()
}

private fun sampleSizeFor(
    srcSize: Int,
    targetSize: Int,
): Int {
    var sample = 1
    var size = srcSize
    while (size / 2 >= targetSize) {
        size /= 2
        sample *= 2
    }
    return sample
}
