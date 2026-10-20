# KaneegaBell themes

A theme (KB calls them *styles*) is a small JSON file: colors, a corner radius, how the
focused game is marked, and which bull icon the window uses. It holds no CSS and no script,
so a theme can't break the layout, the controller focus or performance. KB turns the colors
into the CSS variables its stylesheets already use (`src/styles/tokens.css`).

The built-in styles are in [`src-tauri/themes/`](../src-tauri/themes/) and follow this exact
format. The schema is [`theme.schema.json`](theme.schema.json).

## Making, sharing and importing

- **Make one:** Settings → General → Style → Manage… → *New style from this one…*. Change the
  name, look, focus marker, icon, corners and colors; the whole app is the live preview, and Save
  stays off until the theme passes the checks below. *Edit…* changes one of your own later.
- **Share one:** *Export this style…* writes a `.kbtheme` file (a zip of `theme.json` and the
  picture, if any) to a folder. *Import a .kbtheme…* adds one to your styles.
- Your styles live in `data	hemes\` next to the exe. A `.kbtheme` may hold only `theme.json`
  (16 KB at most) and one `background.webp`/`.png`/`.jpg` (8 MB at most); anything else makes the
  file invalid. The picture is resized to 1920x1080 at most and re-encoded as WebP on import.

## Example

```json
{
  "format": 1,
  "name": "Ember",
  "author": "KaneegaBell",
  "version": "1.0",
  "minAppVersion": "0.3.11",
  "scheme": "dark",
  "radius": 6,
  "focusLook": "brackets",
  "icon": "ember",
  "colors": {
    "bg": "#0d0a09", "bg-raised": "#17110e", "bg-sunken": "#070504",
    "surface": "#221915", "surface-hover": "#2d211b", "border": "#33261f",
    "text": "#f2e8d8", "text-dim": "#bfae97", "text-faint": "#8a7a68",
    "accent": "#e2582b", "accent-strong": "#ff8a5c", "on-accent": "#140805",
    "focus": "#f2e8d8", "gold": "#d9a52b", "danger": "#ff6a5a", "ok": "#8fcf7a"
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
| `focusLook` | How the focused cover is marked: `glow`, `ring` (with a gap), `lift`, `brackets`, `bar` or `spotlight` (the other covers dim). Buttons and rows always get a ring in the `focus` color. |
| `icon` | The bull used for the window and taskbar: `ember`, `midnight`, `daylight`, `contrast`, `moss` or `storm`. |
| `colors` | All 16 colors below, each `#rrggbb`. |
| `tv` | Optional. Colors that differ in TV Mode, any of the 16. |
| `background` | Optional, `{ "dim": 0-90 }`. Set when the theme has a picture: it fills the window behind everything, with the `bg` color laid over it at `dim` percent. Panels that aren't see-through cover it. |

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
