package com.siansiansu.taigikeyboard.ime.dictionary

import com.siansiansu.taigikeyboard.engine.RustEngineBridge
import com.siansiansu.taigikeyboard.engine.externalLookupDigitForm
import java.net.URLEncoder

/**
 * Builds MOE / Chhoe Taigi external dictionary lookup URLs from TL display form.
 * Parallels iOS `ExternalLookupURLBuilder.swift`. The digit-tone reading the
 * dictionaries search by (tone 1 / tone 4 omitted) is the engine's
 * (`RustEngineBridge.externalLookupDigitForm`).
 *
 * Platform-owned by design — which query parameters each dictionary takes and
 * the percent-encoding rules belong with the platform that knows about
 * `URLEncoder`.
 */
object ExternalLookupURLBuilder {
    /** Build Chhoe Taigi lookup URL. Returns null if the TL string produces an empty digit form. */
    fun chhoeURL(tl: String): String? {
        val tlDigit = RustEngineBridge.externalLookupDigitForm(tl)
        if (tlDigit.isEmpty()) return null
        val encoded = URLEncoder.encode(tlDigit, "UTF-8")
        return "https://chhoe.taigi.info/s?s=su&f=e&lmjf=ki&lmj=$encoded"
    }

    /** Build MOE Dictionary lookup URL. Returns null if the TL string produces an empty digit form. */
    fun moeURL(tl: String): String? {
        val tlDigit = RustEngineBridge.externalLookupDigitForm(tl)
        if (tlDigit.isEmpty()) return null
        val encoded = URLEncoder.encode(tlDigit, "UTF-8")
        return "https://sutian.moe.edu.tw/zh-hant/tshiau/?lui=tai_su&tsha=$encoded"
    }
}
