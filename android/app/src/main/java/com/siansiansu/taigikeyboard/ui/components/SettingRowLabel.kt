package com.siansiansu.taigikeyboard.ui.components

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp

// A settings row's label and, when it has one, its info button. Takes the row's remaining
// width so a localized label wraps before the trailing control; shared by SwitchRow and
// SettingNavigationRow.
@Composable
internal fun RowScope.SettingRowLabel(
    label: String,
    color: Color,
    infoText: String?,
    fontFamily: FontFamily? = null,
) {
    Row(
        modifier = Modifier.weight(1f),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = label,
            modifier = Modifier.weight(1f, fill = false),
            color = color,
            fontFamily = fontFamily,
            style = MaterialTheme.typography.bodyLarge,
        )
        if (infoText != null) {
            Spacer(Modifier.width(6.dp))
            SettingInfoButton(description = infoText)
        }
    }
}
