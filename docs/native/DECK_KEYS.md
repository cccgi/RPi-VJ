# Stream Deck XL: system keys

The top three rows (keys 0–23) belong to the page: 24 shaders, clips or controls from
`show.json`. The bottom row (keys 24–31) is drawn by the controller on every page.
Keys 26–29 give way to a page that binds its own key there.

| Key | Label | What it does |
|---|---|---|
| 24 | page number, left arrow | Previous page. |
| 25 | page number, right arrow | Next page. |
| 26 | SHUFFLE | Auto shuffle on or off. While on, the next shader is dealt at each new section of the music, and the deck turns to the page that holds it so its key shows as LIVE. |
| 27 | MIC / LINE | Input gain at the sound card for a microphone or a line source. Shows the current setting. |
| 28 | PUMP / STROBE / STEP / RIDE / TWIST / AUTO | How shaders react to the music. Shows the current mode; a press steps to the next. |
| 29 | DELETE (hidden) | See below. |
| 30 | RESET | Returns the look to the show's defaults. Keeps the input setting, gain and delete marks. |
| 31 | BLACK | Blackout on or off. |

## DELETE (key 29, hidden)

DELETE is built but switched off. To bring it back, set `DELETE_KEY` to `true` in
`engine/src/surface.rs`, rebuild and install the controller.

When shown:

- A press marks the shader on screen as unfit. Nothing is removed from the show.
- On a marked shader the key reads UNMARK; a press takes the mark off.
- The key's foot counts the marks ("3 MARKED").
- A marked shader's key carries a large red X and the word MARKED. It still plays, so
  the choice can be reviewed.
- Auto shuffle skips marked shaders, and leaves one at once when it is marked.

The marks are `deleted_shaders` in the controller's state file
(`/var/lib/rpi-vj-controller/state.json`): one true/false per shader, in show order.
They survive RESET and restarts. They are stored by position, so do not add or remove
shaders in `show.json` while marks are waiting.

Removing the marked shaders is a manual clean-up of the show, not something a key
does: take them out of `show.json`, renumber and repack the page bindings, drop the
same positions from the per-shader lists in the state file, and add Shadertoy ids to
`engine/tools/shader_bank/bank_exclude.txt` so a later import does not bring them back.
While the key is hidden, marks that already exist keep their X and are still skipped.
