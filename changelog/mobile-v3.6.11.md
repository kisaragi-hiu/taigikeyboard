## v3.6.11

Mobile patch for iOS and Android. Headline: on Android, users who upgraded
from v3.6.8 to v3.6.10 get their **custom dictionary and word frequency back** —
v3.6.10 could not open the existing files, so the custom dictionary looked
empty and import / export failed. The data was never deleted; the first launch
of v3.6.11 takes the files over in place.

### Shared (iOS + Android)

#### Dictionary

- **Word-association data v2.** `association.bin` carries corpus word-pair
  counts beside the existing character keys (3.3 → 6.0 MB), groundwork for
  word-level next-word predictions. (#267, #268)

### Android

#### New

- **Collapsible theme preview.** The live keyboard preview pinned under the
  custom-theme editor folds away with its chevron (Collapse Preview / Expand
  Preview), freeing room to scroll the settings; the preview always renders
  light, as the keyboard does under a custom theme. (#266)

#### Bug Fixes

- **Custom dictionary empty, import / export failing after upgrading.** On a
  first launch after upgrading from v3.6.8, the engine's one-time safety copy
  of each existing store was published with a hard link, which Android forbids
  apps; the custom dictionary and word-frequency stores then stayed closed —
  an empty word list and `the engine did not answer …` on export / import.
  The copy is now published by rename on Android. (#269)
