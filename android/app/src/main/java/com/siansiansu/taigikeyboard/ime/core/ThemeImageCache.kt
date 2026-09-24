// Process-wide decoded theme-photo cache, keyed by the theme image file name.

package com.siansiansu.taigikeyboard.ime.core

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache

/**
 * Decodes a theme photo from the [ThemeImageStore] directory once per process and keeps it
 * decoded as a hardware bitmap (pixels in GPU memory, not the Java heap; both the View
 * `drawBitmap` and Compose `drawImage` paths accept it). [LruCache] evicts by byte size (one
 * 1280 px photo is ~5 MB, the limit holds about three). A replaced photo gets a new file
 * name, so a stale entry is never served. One instance lives on `CompositionRoot`. Mirrors
 * iOS ThemeImageCache.
 */
class ThemeImageCache(
    val store: ThemeImageStore,
) {
    private val cache =
        object : LruCache<String, Bitmap>(TOTAL_BYTES_LIMIT) {
            override fun sizeOf(
                key: String,
                value: Bitmap,
            ): Int = value.byteCount
        }

    /** The decoded photo, or null when the file is missing / undecodable. */
    fun bitmap(file: String): Bitmap? {
        cache.get(file)?.let { return it }
        val options = BitmapFactory.Options().apply { inPreferredConfig = Bitmap.Config.HARDWARE }
        val decoded = BitmapFactory.decodeFile(store.file(file).path, options) ?: return null
        cache.put(file, decoded)
        return decoded
    }

    /**
     * Removes every stored photo no theme in [themes] references and drops their decoded
     * bitmaps. Wired as the store's mutation hook and run when the editor closes.
     */
    fun sweep(themes: List<UserTheme>) {
        val referenced = themes.referencedPhotoFiles()
        store.sweep(referenced)
        cache
            .snapshot()
            .keys
            .filter { it !in referenced }
            .forEach { cache.remove(it) }
    }

    companion object {
        private const val TOTAL_BYTES_LIMIT = 16 * 1024 * 1024
    }
}
