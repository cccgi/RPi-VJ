import assert from 'node:assert/strict';

const hidListeners = new Map();
Object.defineProperty(globalThis, 'navigator', { configurable: true, value: {
  hid: {
    async getDevices() { return []; },
    addEventListener(name, cb) { hidListeners.set(name, cb); },
    removeEventListener(name) { hidListeners.delete(name); },
  },
}});
// A graphics stall must not be able to stall HID queue recovery.
globalThis.requestAnimationFrame = () => { throw new Error('HID queue used rAF'); };

const { StreamDeckXL } = await import('../src/streamdeck.js');

let failNext = true;
let reports = 0;
const listeners = new Map();
const device = {
  opened: true,
  productName: 'Mock Stream Deck XL',
  addEventListener(name, cb) { listeners.set(name, cb); },
  removeEventListener(name) { listeners.delete(name); },
  async open() { this.opened = true; },
  async close() { this.opened = false; },
  async sendFeatureReport() {},
  async sendReport() {
    if (failNext) { failNext = false; throw new Error('mock USB failure'); }
    reports++;
  },
};

const deck = new StreamDeckXL(device);
assert.equal(await deck.open(), true);
const jpeg = new Uint8Array([1, 2, 3, 4, 5]);
await assert.rejects(deck.setIcon(0, jpeg), /mock USB failure/);
assert.equal(deck._iconHashes[0], 0, 'failed image must not be cached');
await deck.setIcon(0, jpeg);
assert.ok(reports > 0, 'same image must retry after a failed write');
assert.notEqual(deck._iconHashes[0], 0, 'successful image should be cached');
console.log('PASS: HID writes do not use rAF; failed images remain retryable');
