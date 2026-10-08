package com.siansiansu.taigikeyboard.ime.text.keyboard

import com.siansiansu.taigikeyboard.ime.text.key.KeyCode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pure-JVM tests for [KeyboardLayoutSolver]. */
class KeyboardLayoutSolverTest {
    // --- solveKeyDimensions ---

    @Test
    fun `solveKeyDimensions divides container by 10 and subtracts twice keyMarginH`() {
        val result = KeyboardLayoutSolver.solveKeyDimensions(
            KeyDimensionsInput(
                containerWidth = 1080,
                keyMarginH = 5,
                keyMarginV = 0,
                baseKeyHeight = 200f,
                isLandscape = false,
                keyHeightScale = 1.0f,
            ),
        )

        assertEquals(98, result.desiredKeyWidth)
        assertEquals(200, result.desiredKeyHeight)
        assertEquals(1.0f, result.keyHeightFactor)
    }

    @Test
    fun `solveKeyDimensions applies the landscape orientation multiplier`() {
        val result = KeyboardLayoutSolver.solveKeyDimensions(
            KeyDimensionsInput(
                containerWidth = 2000,
                keyMarginH = 0,
                keyMarginV = 0,
                baseKeyHeight = 200f,
                isLandscape = true,
                keyHeightScale = 1.0f,
            ),
        )

        assertEquals(0.85f, result.keyHeightFactor)
        assertEquals(170, result.desiredKeyHeight)
    }

    @Test
    fun `solveKeyDimensions multiplies orientation factor by the height scale`() {
        val result = KeyboardLayoutSolver.solveKeyDimensions(
            KeyDimensionsInput(
                containerWidth = 1000,
                keyMarginH = 0,
                keyMarginV = 0,
                baseKeyHeight = 100f,
                isLandscape = false,
                keyHeightScale = 1.38f,
            ),
        )

        // 1.0 * 1.38 = 1.38
        assertEquals(1.38f, result.keyHeightFactor, 1e-5f)
        assertEquals(138, result.desiredKeyHeight)
    }

    @Test
    fun `solveKeyDimensions truncates fractional desiredKeyHeight via toInt`() {
        val result = KeyboardLayoutSolver.solveKeyDimensions(
            KeyDimensionsInput(
                containerWidth = 0,
                keyMarginH = 0,
                keyMarginV = 0,
                baseKeyHeight = 7f,
                isLandscape = false,
                keyHeightScale = 0.95f,
            ),
        )

        // 7 * 0.95 = 6.65 → toInt = 6
        assertEquals(6, result.desiredKeyHeight)
    }

    @Test
    fun `solveKeyDimensions preserves fractional baseKeyHeight precision through one truncation`() {
        // Regression for Codex PR #227 r3202664511: a previous draft truncated
        // baseKeyHeight to Int before applying multipliers, which on densities
        // where R.dimen.key_height resolves to a non-integer Float can drift
        // by 1px versus the legacy `(getDimension * factor).toInt()` shape.
        val result = KeyboardLayoutSolver.solveKeyDimensions(
            KeyDimensionsInput(
                containerWidth = 0,
                keyMarginH = 0,
                keyMarginV = 0,
                baseKeyHeight = 200.99f,
                isLandscape = false,
                keyHeightScale = 1.15f,
            ),
        )

        // Float math: 200.99 * 1.15 = 231.1385 → toInt = 231 (legacy).
        // With the bug:    200    * 1.15 = 230.0    → toInt = 230.
        assertEquals(231, result.desiredKeyHeight)
    }

    @Test
    fun `solveKeyDimensions echoes margins, width and orientation for the layout`() {
        val result = dimensions(isLandscape = true)

        assertEquals(1000, result.containerWidth)
        assertEquals(5, result.keyMarginH)
        assertEquals(6, result.keyMarginV)
        assertTrue(result.isLandscape)
    }

    // --- keyWidth (iOS LayoutConstants slot fractions) ---
    // trace: width 1000, keyMarginH 5 → letter = 1000/10 - 10 = 90; slot key = round(1000 * f) - 10

    @Test
    fun `keyWidth gives letters a tenth of the row minus margins`() {
        assertEquals(90, width('a'.code))
    }

    @Test
    fun `keyWidth gives shift and delete a 13 percent slot`() {
        assertEquals(120, width(KeyCode.SHIFT))
        assertEquals(120, width(KeyCode.DELETE))
    }

    @Test
    fun `keyWidth gives keyboard switch, emoji and globe keys 13 percent portrait and 10 percent landscape`() {
        for (code in listOf(
            KeyCode.VIEW_SYMBOLS,
            KeyCode.VIEW_CHARACTERS,
            KeyCode.VIEW_SYMBOLS2,
            KeyCode.SWITCH_TO_MEDIA_CONTEXT,
            KeyCode.LANGUAGE_SWITCH,
        )) {
            assertEquals(120, width(code))
            assertEquals(90, width(code, isLandscape = true))
        }
    }

    @Test
    fun `keyWidth gives enter 15 percent portrait and 9_5 percent landscape`() {
        assertEquals(140, width(KeyCode.ENTER))
        assertEquals(85, width(KeyCode.ENTER, isLandscape = true))
    }

    @Test
    fun `keyWidth gives translate two letter slots on phahTaigi and one and a half elsewhere`() {
        assertEquals(190, width(KeyCode.TRANSLATE, layoutType = "phahTaigi"))
        assertEquals(190, width(KeyCode.TRANSLATE, layoutType = "moe1"))
        assertEquals(140, width(KeyCode.TRANSLATE, layoutType = "qwerty"))
    }

    @Test
    fun `keyWidth keeps the legacy symbols space and advanced number pad multipliers`() {
        // trace: 90 * 0.56 = 50.4 → 50; 90 * 1.34 = 120.6 → 120; 90 * 1.56 = 140.4 → 140
        assertEquals(50, width(KeyCode.SPACE, mode = KeyboardMode.SYMBOLS))
        assertEquals(90, width(KeyCode.SPACE))
        assertEquals(120, width(KeyCode.VIEW_SYMBOLS, mode = KeyboardMode.NUMERIC_ADVANCED))
        assertEquals(140, width(KeyCode.DELETE, mode = KeyboardMode.NUMERIC_ADVANCED))
    }

    @Test
    fun `keyWidth gives every number pad key the letter width so the row grows evenly`() {
        assertEquals(90, width(KeyCode.DELETE, mode = KeyboardMode.PHONE))
        assertEquals(90, width('1'.code, mode = KeyboardMode.NUMERIC))
    }

    @Test
    fun `isFixedWidth is true only for slot-sized keys`() {
        assertTrue(KeyboardLayoutSolver.isFixedWidth(KeyCode.SHIFT, "qwerty", isLandscape = false))
        assertTrue(KeyboardLayoutSolver.isFixedWidth(KeyCode.SWITCH_TO_MEDIA_CONTEXT, "qwerty", isLandscape = false))
        assertFalse(KeyboardLayoutSolver.isFixedWidth('a'.code, "qwerty", isLandscape = false))
        assertFalse(KeyboardLayoutSolver.isFixedWidth(KeyCode.SPACE, "qwerty", isLandscape = false))
    }

    private fun dimensions(isLandscape: Boolean = false): KeyDimensions =
        KeyboardLayoutSolver.solveKeyDimensions(
            KeyDimensionsInput(
                containerWidth = 1000,
                keyMarginH = 5,
                keyMarginV = 6,
                baseKeyHeight = 100f,
                isLandscape = isLandscape,
                keyHeightScale = 1.0f,
            ),
        )

    private fun width(
        keyCode: Int,
        mode: KeyboardMode = KeyboardMode.CHARACTERS,
        layoutType: String = "phahTaigi",
        isLandscape: Boolean = false,
    ): Int = KeyboardLayoutSolver.keyWidth(keyCode, mode, layoutType, dimensions(isLandscape))

    // --- solvePopupDimensions ---

    @Test
    fun `solvePopupDimensions in portrait scales width and height by the portrait multipliers`() {
        val result = KeyboardLayoutSolver.solvePopupDimensions(
            PopupDimensionsInput(
                desiredKeyWidth = 100,
                desiredKeyHeight = 200,
                keyViewMeasuredWidth = 100,
                isLandscape = false,
            ),
        )

        assertEquals(110, result.popupWidth)
        assertEquals(500, result.popupHeight)
        // diffX = (100 - 110) / 2 = -5
        assertEquals(-5, result.popupDiffX)
        assertEquals(-5, result.popupX)
        assertEquals(-500, result.popupY)
    }

    @Test
    fun `solvePopupDimensions in landscape scales width and height by the landscape multipliers`() {
        val result = KeyboardLayoutSolver.solvePopupDimensions(
            PopupDimensionsInput(
                desiredKeyWidth = 100,
                desiredKeyHeight = 200,
                keyViewMeasuredWidth = 100,
                isLandscape = true,
            ),
        )

        assertEquals(60, result.popupWidth)
        assertEquals(600, result.popupHeight)
        // diffX = (100 - 60) / 2 = 20
        assertEquals(20, result.popupDiffX)
        assertEquals(20, result.popupX)
        assertEquals(-600, result.popupY)
    }

    // --- solveExtendedPopupGeometry — row split ---

    @Test
    fun `solveExtendedPopupGeometry with popupCount up to 10 stays single-row`() {
        for (count in 0..10) {
            val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
                ExtendedPopupGeometryInput(
                    popupCount = count,
                    keyViewX = 0f,
                    keyboardViewMeasuredWidth = 1080,
                    keyViewMeasuredWidth = 100,
                    keyViewMeasuredHeight = 200,
                    keyPopupWidth = 110,
                    keyPopupHeight = 500,
                ),
            )

            assertEquals("popupCount=$count", count, result.row0count)
            assertEquals("popupCount=$count", 0, result.row1count)
        }
    }

    @Test
    fun `solveExtendedPopupGeometry with odd count above 10 puts extra key on row0`() {
        val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 11,
                keyViewX = 0f,
                keyboardViewMeasuredWidth = 1080,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        assertEquals(6, result.row0count)
        assertEquals(5, result.row1count)
    }

    @Test
    fun `solveExtendedPopupGeometry with even count above 10 splits evenly`() {
        val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 12,
                keyViewX = 0f,
                keyboardViewMeasuredWidth = 1080,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        assertEquals(6, result.row0count)
        assertEquals(6, result.row1count)
    }

    // --- solveExtendedPopupGeometry — anchor side ---

    @Test
    fun `solveExtendedPopupGeometry anchors LEFT when keyViewX is in left half`() {
        val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 3,
                keyViewX = 100f,
                keyboardViewMeasuredWidth = 1080,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        assertEquals(AnchorSide.LEFT, result.anchorSide)
    }

    @Test
    fun `solveExtendedPopupGeometry anchors RIGHT when keyViewX is at or past midpoint`() {
        val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 3,
                keyViewX = 540f,
                keyboardViewMeasuredWidth = 1080,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        assertEquals(AnchorSide.RIGHT, result.anchorSide)
    }

    // --- solveExtendedPopupGeometry — anchor offset clamp ---

    @Test
    fun `solveExtendedPopupGeometry anchorOffset is zero when row0 holds at most one key`() {
        val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 1,
                keyViewX = 100f,
                keyboardViewMeasuredWidth = 1080,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        assertEquals(0, result.anchorOffset)
    }

    @Test
    fun `solveExtendedPopupGeometry anchorOffset clamps down when available space is tight`() {
        // row0count=5 (popupCount=5), default offset=2, anchor LEFT.
        // Derived diffX = (100 - 110) / 2 = -5.
        // availableSpace = keyViewX + diffX = 100 + (-5) = 95.
        // 95 < 2*110 → offset=1; 95 < 1*110 → offset=0; loop exits.
        val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 5,
                keyViewX = 100f,
                keyboardViewMeasuredWidth = 1080,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        assertEquals(0, result.anchorOffset)
    }

    @Test
    fun `solveExtendedPopupGeometry anchorOffset keeps default when space is ample`() {
        // row0count=5, default offset=2, LEFT anchor, large availableSpace.
        val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 5,
                keyViewX = 500f,
                keyboardViewMeasuredWidth = 2000,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        // availableSpace = 500 + (-5) = 495; 495 >= 2*110=220 → offset stays at 2.
        assertEquals(2, result.anchorOffset)
    }

    @Test
    fun `solveExtendedPopupGeometry anchorOffset for even row0count uses size-half-minus-one default`() {
        // popupCount=12 → row0count=6, row1count=6. Even-length default
        // offset = (6/2) - 1 = 2.
        val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 12,
                keyViewX = 800f,
                keyboardViewMeasuredWidth = 2000,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        // availableSpace = 800 + (-5) = 795 >= 2*110=220 → offset stays at 2.
        assertEquals(2, result.anchorOffset)
    }

    @Test
    fun `solveExtendedPopupGeometry RIGHT anchor uses right-edge availableSpace`() {
        // popupCount=5 → row0count=5, default offset=2. RIGHT anchor.
        // Derived diffX = (100 - 110) / 2 = -5.
        // availableSpace = kbdWidth - (keyViewX + diffX + keyPopupWidth)
        //                = 1080 - (900 + (-5) + 110) = 1080 - 1005 = 75.
        // 75 < 2*110=220 → offset=1; 75 < 1*110=110 → offset=0.
        val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 5,
                keyViewX = 900f,
                keyboardViewMeasuredWidth = 1080,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        assertEquals(AnchorSide.RIGHT, result.anchorSide)
        assertEquals(0, result.anchorOffset)
    }

    // --- solveExtendedPopupGeometry — extWidth / extHeight ---

    @Test
    fun `solveExtendedPopupGeometry extHeight doubles when row1 is non-empty`() {
        val singleRow = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 5,
                keyViewX = 500f,
                keyboardViewMeasuredWidth = 2000,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )
        val doubleRow = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 12,
                keyViewX = 500f,
                keyboardViewMeasuredWidth = 2000,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        assertEquals(200, singleRow.extHeight)
        assertEquals(400, doubleRow.extHeight)
        assertEquals(5 * 110, singleRow.extWidth)
        assertEquals(6 * 110, doubleRow.extWidth)
    }

    // --- solveExtendedPopupGeometry — placement ---

    @Test
    fun `solveExtendedPopupGeometry LEFT anchor shifts popupX left by anchorOffset times keyPopupWidth`() {
        // popupCount=5 → offset default=2 with ample space; LEFT anchor.
        // popupX = (100-110)/2 + (-2*110) = -5 + -220 = -225.
        val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 5,
                keyViewX = 500f,
                keyboardViewMeasuredWidth = 2000,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        assertTrue("expected LEFT", result.anchorSide == AnchorSide.LEFT)
        assertEquals(-225, result.popupX)
        assertEquals(-500, result.popupY)
    }

    @Test
    fun `solveExtendedPopupGeometry RIGHT anchor uses negative-extWidth-plus-keyPopupWidth-plus-offset shift`() {
        // popupCount=5 → row0count=5, extWidth=550, default offset=2.
        // keyViewX=1900, kbdWidth=2000 → RIGHT.
        // availableSpace = 2000 - (1900-5+110) = 2000 - 2005 = -5; tight.
        // -5 < 2*110 → offset=1; -5 < 1*110 → offset=0.
        // popupX = (100-110)/2 + (-550 + 110 + 0*110) = -5 + -440 = -445.
        val result = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 5,
                keyViewX = 1900f,
                keyboardViewMeasuredWidth = 2000,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        assertEquals(AnchorSide.RIGHT, result.anchorSide)
        assertEquals(0, result.anchorOffset)
        assertEquals(-445, result.popupX)
    }

    @Test
    fun `solveExtendedPopupGeometry popupY accounts for the second row when present`() {
        val doubleRow = KeyboardLayoutSolver.solveExtendedPopupGeometry(
            ExtendedPopupGeometryInput(
                popupCount = 12,
                keyViewX = 500f,
                keyboardViewMeasuredWidth = 2000,
                keyViewMeasuredWidth = 100,
                keyViewMeasuredHeight = 200,
                keyPopupWidth = 110,
                keyPopupHeight = 500,
            ),
        )

        // popupY = -500 - 200 = -700
        assertEquals(-700, doubleRow.popupY)
    }
}
