// Stores user-picked theme photos as downsampled JPEGs in the app-private files directory.

package com.siansiansu.taigikeyboard.ime.core

import android.content.ContentResolver
import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.util.UUID
import kotlin.math.max
import kotlin.math.roundToInt

/**
 * Theme photos live in `filesDir/theme_images/<uuid>.jpg`, app-private so the settings app
 * (writer, via the photo picker) and the IME (reader) share them without any permission. A
 * picked photo is downsampled while decoding (`ImageDecoder.setTargetSize`, so a 48 MP
 * original never becomes a full bitmap) to [MAX_LONG_EDGE] and re-encoded on save, which
 * bounds the IME's decode cost regardless of the original's size. Mirrors iOS
 * ThemeImageStore.
 */
class ThemeImageStore(
    private val directory: File,
) {
    fun file(name: String): File = File(directory, name)

    /**
     * Downsamples, encodes and writes the photo at [uri] (on [Dispatchers.IO]); returns the
     * new file name, or null when the content cannot be decoded or the write fails.
     */
    suspend fun save(
        resolver: ContentResolver,
        uri: Uri,
    ): String? = withContext(Dispatchers.IO) { saveBlocking(resolver, uri) }

    private fun saveBlocking(
        resolver: ContentResolver,
        uri: Uri,
    ): String? {
        val bitmap =
            try {
                ImageDecoder.decodeBitmap(ImageDecoder.createSource(resolver, uri)) { decoder, info, _ ->
                    val (width, height) = targetSize(info.size.width, info.size.height, MAX_LONG_EDGE)
                    decoder.setTargetSize(width, height)
                    // Software bitmap: needed for JPEG compression (a hardware bitmap cannot be read back).
                    decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                }
            } catch (e: Exception) {
                return null
            }
        val name = UUID.randomUUID().toString() + ".jpg"
        return try {
            directory.mkdirs()
            file(name).outputStream().use { bitmap.compress(Bitmap.CompressFormat.JPEG, JPEG_QUALITY, it) }
            name
        } catch (e: Exception) {
            null
        } finally {
            bitmap.recycle()
        }
    }

    /** Removes every stored photo whose name is not in [referenced] (the files the saved user themes still point at). */
    fun sweep(referenced: Set<String>) {
        directory.listFiles()?.forEach { file ->
            if (file.name !in referenced) file.delete()
        }
    }

    companion object {
        const val DIRECTORY_NAME = "theme_images"

        /** Longest edge after downsampling — wider than any phone keyboard at 3× yet ~5 MB decoded. */
        const val MAX_LONG_EDGE = 1280
        const val JPEG_QUALITY = 85

        /** The app-wide store under `filesDir` (the IME and the settings app run in one process sandbox). */
        fun forApp(context: Context): ThemeImageStore = ThemeImageStore(File(context.filesDir, DIRECTORY_NAME))

        /** The decode target so the longer edge is at most [maxLongEdge] (never upscaled), aspect kept. */
        fun targetSize(
            width: Int,
            height: Int,
            maxLongEdge: Int,
        ): Pair<Int, Int> {
            val longEdge = max(width, height)
            if (longEdge <= maxLongEdge) return width to height
            val scale = maxLongEdge.toFloat() / longEdge
            return max(1, (width * scale).roundToInt()) to max(1, (height * scale).roundToInt())
        }
    }
}
