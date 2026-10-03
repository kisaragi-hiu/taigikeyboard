package com.siansiansu.taigikeyboard.ui.components

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable

@Composable
fun ResultDialog(
    message: String,
    confirmLabel: String,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        text = { Text(message) },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text(confirmLabel)
            }
        },
    )
}

/** A result dialog's message: [title], then [detail] (the engine's words) after a blank line when there is one. */
fun resultMessage(
    title: String,
    detail: String?,
): String = if (detail.isNullOrBlank()) title else "$title\n\n$detail"
