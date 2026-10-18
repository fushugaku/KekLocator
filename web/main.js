import * as audio from './audio-host.js';
globalThis.keklocatorAudio = audio;
try {
  const ui = await import('./pkg/ui/ui.js');
  await ui.default();
  ui.mount();
  globalThis.erikaUi = ui;
} catch (error) {
  document.querySelector('#app').textContent = `Could not load instrument: ${error.message || error}`;
  console.error(error);
}
