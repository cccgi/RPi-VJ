# G-08 — WebHID Stream Deck XL Driver
**Model:** mid · **Blocked by:** G-09 (permissions) · **Blocks:** G-10

## Deliverable
`gig/src/streamdeck.js` — ES module, no dependencies.

## Device facts (verified constants, use as given)
```
vendorId        0x0fd9   (Elgato)
productId       0x006c   (Stream Deck XL) / 0x008f (XL V2)
keys            32  (4 rows x 8 cols)
icon            96 x 96 JPEG
report size     1024 bytes
image header    8 bytes
payload/report  1016 bytes
```
Key index 0 is **top-left**; the XL reports keys left-to-right, top-to-bottom,
but the panel is physically mirrored — you will likely need to flip the column
(`col = 7 - col`) and rotate the JPEG 180°. Verify empirically and write down
what you found; every Stream Deck library disagrees about this.

## API
```js
export class StreamDeckXL {
  static async request()        // navigator.hid.requestDevice(...) — needs a user gesture
  static async getPaired()      // navigator.hid.getDevices() — silent on reconnect
  async open()
  onKey(cb)                     // cb(index, pressed:boolean) — edge-triggered only
  async setIcon(index, jpegBytes)   // Uint8Array
  async clear(index)
  async setBrightness(pct)
  async reset()
  close()
}
```

## Input report parsing
Input reports arrive on `inputreport` events. The XL sends a report with a
4-byte header followed by 32 bytes of key state (`0` or `1`). Diff against the
previous state and emit only transitions. Never emit repeats — a held key must
fire once.

## Image report protocol
Split the JPEG across as many 1024-byte reports as needed. Each report:
```
byte 0    : 0x02   (report id / command)
byte 1    : 0x07   (set image)
byte 2    : key index
byte 3    : 0x01 if this is the final chunk else 0x00
bytes 4-5 : chunk payload length, little-endian uint16
bytes 6-7 : chunk sequence number, little-endian uint16
bytes 8+  : payload, zero-padded to 1024
```
Send with `device.sendReport(0x02, buffer)`.

## Robustness (this is a live gig)
- Wrap every `sendReport` in try/catch. A failed icon write must not stop the
  key-read loop.
- Handle `navigator.hid.ondisconnect` → mark closed, and poll `getPaired()`
  every 2s to auto-reopen. Someone will kick the USB cable.
- Icon writes are slow (~34 reports per key, 32 keys). **Never rewrite all 32
  icons in a frame.** Queue writes, cap at ~2 keys per frame, and skip a write
  whose JPEG bytes hash equal to what's already on that key. (This diffing idea
  is already correct in `vj/crates/vj-surface-hid/src/device.rs` — reuse the
  approach, not the code.)

## Observable
On the **Pi**: 32 keys light with labels, pressing any key logs its index
within 50 ms, unplug/replug recovers automatically. Report the actual key
index↔physical position mapping you found in `../RESULTS.md`.
