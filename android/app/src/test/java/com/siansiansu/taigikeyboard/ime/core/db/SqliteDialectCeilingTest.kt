// SQLite dialect ceiling gate: minSdk 28 bundles SQLite 3.22, and newer syntax
// fails at prepare time on the device only (`.claude/rules/android-guidelines.md`
// §8a). Scans every string literal under src/main for constructs 3.22 rejects.
// The list mirrors §8a "Unavailable at 3.22" — extend both together.

package com.siansiansu.taigikeyboard.ime.core.db

import com.siansiansu.taigikeyboard.locateMainSourceRoot
import com.siansiansu.taigikeyboard.mainKotlinSources
import org.junit.Assert.assertEquals
import org.junit.Test

class SqliteDialectCeilingTest {
    private fun sql(pattern: String) = Regex(pattern, RegexOption.IGNORE_CASE)

    /** Syntax newer than SQLite 3.22, by the version that added it. */
    private val forbidden =
        listOf(
            "UPSERT (3.24)" to sql("""\bON\s+CONFLICT\s*\([^)]*\)\s*DO\s+(UPDATE|NOTHING)\b"""),
            "window functions (3.25)" to sql("""\bOVER\s*\("""),
            "RENAME COLUMN (3.25)" to sql("""\bRENAME\s+COLUMN\b"""),
            "FILTER clause (3.30)" to sql("""\bFILTER\s*\(\s*WHERE\b"""),
            "NULLS FIRST/LAST (3.30)" to sql("""\bNULLS\s+(FIRST|LAST)\b"""),
            "generated columns (3.31)" to sql("""\bGENERATED\s+ALWAYS\b"""),
            "IIF() (3.32)" to sql("""\bIIF\s*\("""),
            "RETURNING (3.35)" to sql("""\bRETURNING\b"""),
            "DROP COLUMN (3.35)" to sql("""\bDROP\s+COLUMN\b"""),
            "MATERIALIZED CTE hint (3.35)" to sql("""\bMATERIALIZED\b"""),
            "STRICT tables (3.37)" to sql("""\)\s*STRICT\b"""),
        )

    // Raw `"""…"""` blocks and plain `"…"` literals; comments are skipped so
    // prose may still name the syntax.
    private val stringLiteral = Regex("\"\"\"[\\s\\S]*?\"\"\"|\"(?:[^\"\\\\\\n]|\\\\.)*\"")

    @Test
    fun `no SQL newer than SQLite 3_22 in src main string literals`() {
        val root = locateMainSourceRoot()
        val hits =
            mainKotlinSources(root)
                .flatMap { file ->
                    stringLiteral.findAll(file.readText()).flatMap { literal ->
                        forbidden
                            .asSequence()
                            .filter { (_, regex) -> regex.containsMatchIn(literal.value) }
                            .map { (name, _) -> "${file.relativeTo(root)}: $name" }
                    }
                }.toList()

        assertEquals(
            "SQL syntax above the SQLite 3.22 ceiling (minSdk 28); see android-guidelines.md §8a",
            emptyList<String>(),
            hits,
        )
    }
}
