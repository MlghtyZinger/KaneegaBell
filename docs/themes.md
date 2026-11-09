# KaneegaBell themes

A theme (KB calls them *styles*) is a small JSON file: colors, a corner radius, how the
focused game is marked, and which bull icon the window uses. It holds no CSS and no script,
so a theme can't break the layout, the controller focus or performance. KB turns the colors
into the CSS variables its stylesheets already use (`src/styles/tokens.css`).

The built-in styles are in [`src-tauri/themes/`](../src-tauri/themes/) and follow this exact
format. The schema is [`theme.schema.json`](theme.schema.json).

KB 0.6.0 includes Forge, Starlight, Obsidian, Aurora, PS3, Wii and PS2. Their bundled backgrounds, fonts and frames live in the application; color data still uses format 1. There is no theme editor or import/export. Desktop and KaneegaBell TV share the Style choice, including Random (changes each app launch, excluding the previous choice). Reduced and Off keep backgrounds still; background motion pauses while hidden or unfocused.

## Example

```json
{
  "format": 1, "name": "Forge", "author": "KaneegaBell", "version": "1.0", "minAppVersion": "0.5.5",
  "scheme": "dark", "radius": 4, "focusLook": "brackets", "icon": "forge",
  "colors": {
    "bg": "#100e0d", "bg-raised": "#1c1815", "bg-sunken": "#080706", "surface": "#29221d", "surface-hover": "#3a2c22", "border": "#524033",
    "text": "#f6ead8", "text-dim": "#c8b49b", "text-faint": "#a18b73", "accent": "#ed783f", "accent-strong": "#ffb076", "on-accent": "#160b07",
    "focus": "#ffbd80", "gold": "#e8b65e", "danger": "#ff8979", "ok": "#a2cc8e"
  }
}
```

## Fields

| Field | What it is |
|---|---|
| `format` | Always `1` for now. |
| `name`, `author`, `version` | 1 to 40 characters each. |
| `minAppVersion` | The oldest KB that can use it, like `"0.3.11"`. |
| `scheme` | `"dark"` or `"light"`: scrollbars and text boxes follow it. |
| `radius` | Corner rounding in pixels, 0 to 24. |
| `focusLook` | How the focused cover is marked: `glow`, `ring` (with a gap), `lift`, `brackets`, `bar`, `spotlight` (the other covers dim), `halo` or `wii`. Buttons and rows always get a ring in the `focus` color. |
| `icon` | The bull used for the window and taskbar: `forge`, `starlight`, `obsidian`, `aurora`, `ps3`, `wii` or `ps2`. Legacy icon IDs remain readable. |
| `colors` | All 16 colors below, each `#rrggbb`. |
| `tv` | Optional. Colors that differ in TV Mode, any of the 16. |

| Color | Used for |
|---|---|
| `bg` | Window background |
| `bg-raised` | Sidebar, panels, menus |
| `bg-sunken` | Platform list, TV Mode's background and shading |
| `surface`, `surface-hover` | Buttons and text boxes, and while hovered |
| `border` | Lines and edges |
| `text`, `text-dim`, `text-faint` | Main text, secondary text, counts and hints |
| `accent`, `accent-strong` | Selected tab, Play button; accent-colored text and tags |
| `on-accent` | Text on the accent color |
| `focus` | The focus ring |
| `gold` | Favorites and achievements |
| `danger`, `ok` | Errors and success |

Covers, and the tinted cards shown while a cover is missing, look the same in every style,
so the text on them always stays light.

## What KB checks

KB refuses a theme with unknown fields, missing or malformed colors (anything but
`#rrggbb`), values out of range, a newer `format`, or a `minAppVersion` above its own. It
also refuses one that's hard to read from the couch:

| Pair | Needs |
|---|---|
| `text` on `bg`, `bg-raised` and `surface` | 4.5:1 |
| `text-dim` on `bg` and `bg-raised` | 4.5:1 |
| `text-faint` on `bg` | 3:1 |
| `on-accent` on `accent` | 4.5:1 |
| `focus` on `bg` and `bg-raised` | 3:1 |

Ratios are WCAG contrast ratios. With `tv` colors, the same checks run again for TV Mode.

## Asking Claude for a theme

Paste this file into a chat and describe the look, for example "deep red with gold accents,
for a dark room". Ask for a `theme.json` that passes the checks above.

The single Style picker in Settings > General controls desktop and KaneegaBell TV UI colors. KB TV uses its original game artwork background without decorative desktop scenery. TV Menu Style retains the KB, Wii, PS3 and PS2 layout choices. PS3 uses one shared flowing-ribbon background with daily local-calendar colors, refreshing at midnight or after sleep. Desktop PS3 keeps the cover grid.
