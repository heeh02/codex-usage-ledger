type Validators<T> = Partial<Record<keyof T, (value: unknown) => boolean>>;
type PreferenceStorage = Pick<Storage, 'getItem' | 'setItem'>;
type StorageSource = () => PreferenceStorage;

const sessionStorage: StorageSource = () => window.sessionStorage;

export const oneOf = (...values: string[]) => (value: unknown): boolean =>
  typeof value === 'string' && values.includes(value);

export const boundedText = (value: unknown): value is string => typeof value === 'string' && value.length <= 256;

export const integerBetween = (min: number, max: number) => (value: unknown): boolean =>
  typeof value === 'number' && Number.isSafeInteger(value) && value >= min && value <= max;

export function readSessionObject<T extends object>(key: string, fallback: T,
  validators: Validators<T> = {}, storage: StorageSource = sessionStorage): T {
  const result = { ...fallback };
  try {
    const encoded = storage().getItem(key);
    if (!encoded) return result;
    const decoded: unknown = JSON.parse(encoded);
    if (!decoded || typeof decoded !== 'object' || Array.isArray(decoded)) return result;
    const record = decoded as Record<string, unknown>;
    const allowed = new Set([...Object.keys(fallback), ...Object.keys(validators)]);
    for (const name of allowed) {
      if (!Object.hasOwn(record, name)) continue;
      const field = name as keyof T;
      const value = record[name];
      const validate = validators[field] ?? ((candidate: unknown) =>
        fallback[field] === null ? candidate === null || boundedText(candidate)
          : typeof candidate === typeof fallback[field] && (typeof candidate !== 'string' || boundedText(candidate)));
      if (validate(value)) result[field] = value as T[keyof T];
    }
  } catch {
    // Preferences are optional. Malformed/blocked storage must not stop rendering.
  }
  return result;
}

export function writeSessionObjects(values: Record<string, unknown>, storage: StorageSource = sessionStorage): void {
  for (const [key, value] of Object.entries(values)) {
    try { storage().setItem(key, JSON.stringify(value)); } catch {
      // Keep current in-memory navigation if browser storage is unavailable/full.
    }
  }
}
