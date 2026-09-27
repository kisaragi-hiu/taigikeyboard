# Theme Color Roles Roadmap

Brings every mobile keyboard element under the custom-theme color tiers in `docs/ui/theme.md` § Custom Theme Color Roles (USER 2026-09-27). Scope: iOS + Android keyboard (not host-app screens). Audit source: 2026-09-27 read of both platforms; items lettered as in that audit.

## Phases

| Phase | Items | Platforms | Target role | Status |
|---|---|---|---|---|
| P0 | This roadmap + tier table + memory | docs | — | Merged |
| — | Grid lines + strip expand-toggle divider (#252) | iOS + Android | candidate text | Merged `14fbc135` |
| — | One-handed menu background / text (folded into #252) | Android | key fill / key text | Merged `14fbc135` |
| P1 | A — key pressed state (#252) | iOS + Android | key fill: dark lightened / light deepened | Merged `14fbc135` |
| P2 | B popup selected cell · C expanded-overlay control pressed · F toolbar pressed | iOS + Android (F Android) | B key pressed fill (USER 2026-09-27); C / F candidate pressed tint | Merged `5fabb929` (#255) |
| P3 | E — English autocorrect highlight | — | — | Dropped 2026-09-27: premise false — neither platform ever emits an autocorrect suggestion (iOS `EnglishAutocompleteService` builds `AutocompleteSuggestion(text:)` only), so no highlight exists to theme |
| P4 | D English strip dividers · I tab-row dividers | iOS + Android (I Android) | candidate text | Merged `1a041cca` (#257; emoji tab divider = key text, matching emoji glyphs) |
| P5 | G translate-key active · H caps-lock distinguishable · J emoji ABC / delete pressed · M nav-bar icons | Android | key fill deepened / fixed accent / background luminance | Pending |
| P6 | L — settings-panel switch off-track | iOS + Android | key fill | Pending |
| P7 | K — emoji keyboard chrome (vendored ISEmojiView) | iOS | key text / key fill / candidate highlight | Pending |

## Rules per phase

- Adaptive (built-in `default`) themes stay visually unchanged: a derived color applies only when its source role is set.
- Each mirrored derivation carries a `CROSS-PLATFORM INVARIANT` comment citing the other platform.
- Tier 3 fixed colors are not touched; H only stops key text from overriding the caps-lock accent.
