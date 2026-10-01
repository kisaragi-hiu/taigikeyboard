Port a feature between iOS and Android platforms.

Arguments: $ARGUMENTS (e.g., "ios Composing" or "android NextWord")

Find the source platform's implementation (iOS↔Android mirror by name: `ios/**/<Name>.swift` ↔ `android/**/<Name>.kt`; phonetics → also `docs/phonetics/taigi-phonetics-reference.md`) and produce a porting plan: files to reference and change, platform adaptations, and the user-visible behavior that must match. Logic both platforms need goes into the engine instead (`AGENTS.md` § Design principles); port only UI and OS integration. Wait for approval before implementing. Align on intended behavior, not API calls.
