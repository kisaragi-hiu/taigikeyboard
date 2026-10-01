package com.siansiansu.taigikeyboard.ime.theme

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.io.File
import java.nio.file.Files

/**
 * Tests for [ThemeImageStore]'s pure parts — the decode target size and the orphan sweep
 * (the ImageDecoder / Bitmap path needs a device). Mirrors iOS ThemeImageStoreTests.
 */
class ThemeImageStoreTest {
    private lateinit var directory: File

    @Before
    fun setUp() {
        directory = Files.createTempDirectory("ThemeImageStoreTest").toFile()
    }

    @After
    fun tearDown() {
        directory.deleteRecursively()
    }

    // A 4000×2000 photo decodes to a 1280×640 target; a small photo is never upscaled; aspect is kept.
    @Test
    fun targetSize_capsLongEdge_neverUpscales() {
        assertEquals(1280 to 640, ThemeImageStore.targetSize(4000, 2000, 1280))
        assertEquals(640 to 1280, ThemeImageStore.targetSize(2000, 4000, 1280))
        assertEquals(300 to 200, ThemeImageStore.targetSize(300, 200, 1280))
        assertEquals(1280 to 1280, ThemeImageStore.targetSize(1280, 1280, 1280))
    }

    // Sweep keeps the referenced file and removes the orphan.
    @Test
    fun sweep_removesUnreferencedFiles() {
        val store = ThemeImageStore(directory)
        val keep = store.file("keep.jpg").also { it.writeBytes(byteArrayOf(1)) }
        val drop = store.file("drop.jpg").also { it.writeBytes(byteArrayOf(2)) }

        store.sweep(setOf("keep.jpg"))

        assertTrue(keep.exists())
        assertFalse(drop.exists())
    }

    // Sweep on a directory that does not exist yet is a no-op.
    @Test
    fun sweep_missingDirectory_noOps() {
        ThemeImageStore(File(directory, "missing")).sweep(emptySet())
        assertFalse(File(directory, "missing").exists())
    }

    // The referenced set is exactly the photo files the saved themes point at.
    @Test
    fun referencedPhotoFiles_collectsPhotoThemesOnly() {
        val photo = UserTheme("a", "A", ThemeAppearance.USER_THEME_SEED.copy(colors = UserThemeSeed.colors.copy(background = ThemeBackground.Image(ThemeImageBackground("p.jpg")))), 0L, 0L)
        val solid = UserTheme("b", "B", ThemeAppearance.USER_THEME_SEED, 0L, 0L)
        assertEquals(setOf("p.jpg"), listOf(photo, solid).referencedPhotoFiles())
    }
}
