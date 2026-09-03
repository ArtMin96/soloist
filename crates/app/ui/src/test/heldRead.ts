/**
 * Points a mocked async read at a promise the test settles by hand, so the caller can be observed
 * in the state it is in before its first read has answered. Returns the settle function: calling it
 * with a value resolves that read and lets the test watch the same caller arrive at its data.
 */
export function holdRead<T>(read: {
  mockReturnValue: (value: Promise<T>) => unknown;
}): (value: T) => void {
  let settle!: (value: T) => void;
  read.mockReturnValue(
    new Promise<T>((resolve) => {
      settle = resolve;
    }),
  );
  return settle;
}
