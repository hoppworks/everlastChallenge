---
name: Everlast Coding Challenge
description: Compact native dictation palette with progressive disclosure
colors:
  text: "light-dark(#252824, #f3f7f1)"
  background: "light-dark(#fbfcfa, #252d29)"
  surface: "light-dark(#ffffff, #354038)"
  muted: "light-dark(#62685f, #bac6bc)"
  line: "light-dark(#dee2d9, #505c53)"
  control-line: "light-dark(#a8b2a2, #74836e)"
  accent: "light-dark(#293d2e, #d5ebd8)"
  accent-text: "light-dark(#ffffff, #1f3b28)"
  soft: "light-dark(#e8eee5, #37463b)"
  signal: "light-dark(#3b6346, #b6d9bb)"
  danger: "light-dark(#ad3c30, #f6c6bf)"
  danger-text: "light-dark(#ffffff, #712e24)"
typography:
  body:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "14px"
    lineHeight: 1.55
  status:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "16px"
    fontWeight: 600
    lineHeight: 1.35
    letterSpacing: "-0.02em"
  label:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "12px"
rounded:
  control: "7px"
  field: "6px"
  transcript: "8px"
  level: "11px"
spacing:
  tight: "8px"
  group: "16px"
  section: "24px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-text}"
    rounded: "{rounded.control}"
    padding: "7px 12px"
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "7px 12px"
  button-stop:
    backgroundColor: "{colors.danger}"
    textColor: "{colors.danger-text}"
    rounded: "{rounded.control}"
    padding: "7px 12px"
  level:
    backgroundColor: "{colors.soft}"
    textColor: "{colors.signal}"
    rounded: "{rounded.level}"
    width: "44px"
    height: "44px"
---

# Design System: Everlast Coding Challenge

## Overview

The approved direction is a small dictation palette: microphone level, current
status and main action form the primary row. Transcript, recording details and
advanced settings appear when needed. Preserve the existing macOS system font.
This records the implementation in `src/features/app/styles.css`, `AppView.tsx`,
`LevelVisual.tsx` and `src/features/settings/styles.css`; it is not a claim of
completed native capture, build verification or technical acceptance.

## Colors

Muted green neutrals support a restrained utility. Accent identifies the main
action; signal colors the microphone indicator and focus outline. Danger marks
Stop while recording. Surface, soft and line distinguish controls and sections.
The canonical values above preserve CSS `light-dark()` pairs and the root
`color-scheme: light dark`; the OS supplies the active appearance. Higher-contrast
preferences strengthen muted text, dividers and button borders in source.

## Typography

Use system typography throughout. Status titles use the status role; compact
metadata uses the label role. Body and transcript text remain readable at the
body size, with the transcript using a line height of 1.65. Timers use tabular
numerals. Do not introduce display fonts or decorative headings.

## Layout

The app fills its native content area, with internal scrolling and main padding
(12px 20px 16px). The primary row uses a 16px gap and a fixed 44px level indicator.
At widths up to 460px, the row wraps and actions move below the status; side
padding becomes 16px. The transcript expands below a divider. Settings use a
separate scrolling content area, a persistent header/navigation and 24px padding
(16px at widths up to 400px), with a maximum width of 680px.

## Elevation & Depth

The app CSS uses flat tonal surfaces and dividers, with no custom shadows.
Native window framing belongs to the OS shell. Do not copy the browser study's
mock desktop, traffic lights or drop shadows into content.

## Shapes

Controls are gently rounded; the larger radius on the level indicator gives it
a distinct compact silhouette. Transcript fields use their own radius. Keep
divider sections flat instead of wrapping every section in a card.

## Components

- **Actions:** one prominent recording action; recording Stop uses danger.
  Secondary controls use surface and a visible border. Hover darkens the primary
  action through brightness; disabled actions remain visibly unavailable.
- **Level:** seven shaped bars derive from a single clamped native RMS value.
  The bar container is 26px square, with 2px bars and 2px gaps inside the 44px
  indicator. It is a level display, not a frequency spectrum or random animation.
  An accessible meter exposes the value; idle state shows the microphone icon.
- **Transcript:** a disclosure opens a read-only, selectable text area with Copy
  and Delete. New text opens the section; telemetry does not reopen it.
- **Settings:** navigation buttons reveal focused settings sections. Selects and
  modifier inputs use native controls; advanced details remain disclosed on demand.
- **Feedback:** permission notices and errors remain visible when needed.
  Focus outlines use 3px with a 3px offset; errors use the source's paired warm
  background and text colors. State labels distinguish sending a paste command
  from observing text in the target application.

## Do's and Don'ts

- **Do** keep dictation primary and reveal supporting detail progressively.
- **Do** preserve light/dark pairs and honor system contrast preferences.
- **Do** keep errors, permission needs and keyboard focus visible.
- **Don't** present RMS bars as spectral analysis or animate invented telemetry.
- **Don't** treat browser fixtures or screenshots as proof of native behavior.
