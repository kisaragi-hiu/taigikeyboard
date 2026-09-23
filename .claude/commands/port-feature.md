Port a feature between iOS and Android platforms.

Arguments: $ARGUMENTS (e.g., "ios Tone" or "android Composing")

Find the source platform's implementation (iOS↔Android mirror by name: `ios/**/<Name>.swift` ↔ `android/**/<Name>.kt`; phonetics → also `knowledge/taigi-phonetics-reference.md`) and produce a porting plan: files to reference and change, platform adaptations, and the user-visible behavior that must match. Wait for approval before implementing. Align on intended behavior, not API calls.
