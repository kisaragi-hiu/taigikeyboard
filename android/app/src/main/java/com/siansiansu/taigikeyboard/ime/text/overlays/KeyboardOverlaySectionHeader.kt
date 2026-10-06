package com.siansiansu.taigikeyboard.ime.text.overlays

// Section label shared by the toolbar overlays (layout, settings); mirrors iOS KeyboardOverlaySectionHeader.

import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

private const val SECTION_HEADER_ALPHA = 0.6f

@Composable
internal fun KeyboardOverlaySectionHeader(
    text: String,
    color: Color,
    modifier: Modifier = Modifier,
) {
    Text(
        text = text,
        color = color.copy(alpha = SECTION_HEADER_ALPHA),
        fontSize = 11.sp,
        fontWeight = FontWeight.Bold,
        modifier = modifier,
    )
}
