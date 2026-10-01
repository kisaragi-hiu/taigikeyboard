package com.siansiansu.taigikeyboard.ime.theme

import java.util.UUID

/**
 * Theme identity helpers. `"default"` = the factory appearance; UUID strings = user themes; other
 * non-UUID strings = built-in theme ids. Mirrors iOS ThemeId.
 */
object ThemeId {
    /** The factory theme. */
    const val DEFAULT = "default"

    /**
     * Whether [id] is a user theme. User-theme ids are UUID strings; the
     * `default` theme and built-in ids are not.
     */
    fun isUserTheme(id: String): Boolean = runCatching { UUID.fromString(id) }.isSuccess
}
