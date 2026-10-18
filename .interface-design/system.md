# KekLocator control surface

Musicians load audio or connect a live input, recall a sound, then adjust it while listening. Keep the instrument available without navigating away from its controls.

Domain: tape loops, feedback, stereo space, orbit, signal routing, a small amber OLED. Colors: carbon, graphite, warm ivory, amber, copper, restrained signal green. The signature is a stereo echo timeline beside the main time display, with twelve equally sized rotary controls below it.

Replace dashboard metric cards with one connected instrument panel. Replace an app sidebar with a compact searchable preset rack. Replace decorative hero artwork with the live signal display and transport.

- Canvas `#101213`; panel `#191c1d`; inset `#121515`; overlay `#232728`.
- Text primary `#ecebe3`; secondary `#b6bbb6`; tertiary `#858d88`; muted `#68716c`.
- Amber `#f3bb72`; copper `#b9824f`; signal `#98bda3`; error `#f09483`.
- Border subtle `#252b29`; default `#343c38`; hover `#5a675e`; focus amber.
- Borders and slight surface shifts establish depth. No ornamental gradients or glows.
- 4 px spacing grid; 16 px control gaps; 24 px panel padding; radii 4 and 6 px.
- System sans for actions; compact uppercase labels; monospace/tabular numbers for parameter values and the display.
- All knobs occupy identical grid cells. Controls remain accessible sliders with visible focus, keyboard steps and pointer/touch drag.
- Desktop: preset rack plus instrument. Mobile: preset rack above instrument, knobs in two columns, transport within reach. No horizontal scrolling at 360 px.
- Toggle buttons expose `aria-pressed`; pending audio actions and failures have explicit status. User names and imported data are escaped before entering markup.

## Performance editor 1.1

Native plugin adds an Original/Extended segmented control, a compact preset/history toolbar, an inline bank drawer, and a selected-step inspector. Sound A/B keeps the pattern unchanged. Separate amber outlines mark selection and a top amber bar marks the playing step. Extended controls collapse in Original. Wet ducking has four equal columns, two on mobile. The step inspector has three groove columns and five lock columns; mobile uses three lock columns and a two-column Freeze cell. Range tracks/thumbs are styled explicitly for WKWebView. Bank rows use an independent star button. User names are escaped before HTML rendering.
