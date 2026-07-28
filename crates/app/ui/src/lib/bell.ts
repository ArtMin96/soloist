// The in-app bell: one short tone, synthesised rather than shipped, so the alert costs the bundle
// nothing. A desktop notification carries a sound hint the OS plays for it, but an alert that lands
// as a toast never reaches the OS — without this the bell would be silent exactly when the user is
// at the keyboard.
//
// It is decoration and behaves like it: a platform with no Web Audio, an audio policy that refuses
// to start the context, or any failure inside it leaves the toast on screen and says nothing.
const FREQUENCY_HZ = 880;
const ATTACK_S = 0.008;
const DURATION_S = 0.12;
// Well under a system alert sound. This fires while the user is looking at the screen and already
// has a toast telling them the same thing; it only has to be noticed, not obeyed.
const PEAK_GAIN = 0.06;
// exponentialRampToValueAtTime cannot reach zero, so the decay lands just below hearing instead.
const SILENCE_GAIN = 0.0001;

let shared: AudioContext | null = null;
let unavailable = false;

// One context for the app: each one holds an audio thread, and browsers cap how many a page may
// open, so a bell per alert would eventually stop making any sound at all.
function context(): AudioContext | null {
  if (shared || unavailable) return shared;
  try {
    shared = new AudioContext();
  } catch {
    unavailable = true;
  }
  return shared;
}

export function playBell(): void {
  if (typeof AudioContext === "undefined") return;
  const audio = context();
  if (!audio) return;

  try {
    // An autoplay policy can hand back a suspended context; it resumes once the window has been
    // interacted with, and until then this simply makes no sound.
    void audio.resume().catch(() => {});

    const oscillator = audio.createOscillator();
    const gain = audio.createGain();
    oscillator.type = "sine";
    oscillator.frequency.value = FREQUENCY_HZ;

    const start = audio.currentTime;
    gain.gain.setValueAtTime(SILENCE_GAIN, start);
    gain.gain.exponentialRampToValueAtTime(PEAK_GAIN, start + ATTACK_S);
    gain.gain.exponentialRampToValueAtTime(SILENCE_GAIN, start + DURATION_S);

    oscillator.connect(gain).connect(audio.destination);
    oscillator.onended = () => {
      oscillator.disconnect();
      gain.disconnect();
    };
    oscillator.start(start);
    oscillator.stop(start + DURATION_S);
  } catch {
    // Nothing to recover: the alert itself is already on screen.
  }
}
