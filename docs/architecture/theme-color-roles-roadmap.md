# Theme Color Roles Roadmap

Brings every mobile keyboard element under the custom-theme color tiers in `docs/ui/theme.md` § Custom Theme Color Roles (USER 2026-09-27). Scope: iOS + Android keyboard (not host-app screens). Audit source: 2026-09-27 read of both platforms; items lettered as in that audit.

## Phases

| Phase | Items | Platforms | Target role | Status |
|---|---|---|---|---|
| P0 | This roadmap + tier table + memory | docs | — | Merged |
| — | Grid lines + strip expand-toggle divider (#252) | iOS + Android | candidate text | PR open |
| — | One-handed menu background / text (#254) | Android | key fill / key text | PR open |
| P1 | A — key pressed state | iOS + Android | key fill deepened | Pending |
| P2 | B popup selected cell · C expanded-overlay control pressed · F toolbar pressed | iOS + Android (F Android) | candidate highlight | Pending |
| P3 | E — English autocorrect highlight (parity: Android gains it) | iOS + Android | candidate highlight + candidate text | Pending |
| P4 | D English strip dividers · I tab-row dividers | iOS + Android (I Android) | candidate text | Pending |
| P5 | G translate-key active · H caps-lock distinguishable · J emoji ABC / delete pressed · M nav-bar icons | Android | key fill deepened / fixed accent / background luminance | Pending |
| P6 | L — settings-panel switch off-track | iOS + Android | key fill | Pending |
| P7 | K — emoji keyboard chrome (vendored ISEmojiView) | iOS | key text / key fill / candidate highlight | Pending |

## Rules per phase

- Adaptive (built-in `default`) themes stay visually unchanged: a derived color applies only when its source role is set.
- Each mirrored derivation carries a `CROSS-PLATFORM INVARIANT` comment citing the other platform.
- P3 is a parity correction (`.claude/rules/cross-platform-alignment.md` §1b): describe before / after on both platforms in the PR.
- Tier 3 fixed colors are not touched; H only stops key text from overriding the caps-lock accent.
