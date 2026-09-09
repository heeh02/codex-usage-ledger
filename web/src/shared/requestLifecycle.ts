/** Ignore obsolete completions even when a transport does not honor abort. */
export async function runScopedRequest<T>(
  signal: AbortSignal,
  load: () => Promise<T>,
  callbacks: { success: (value: T) => void; failure: (error: unknown) => void; settled: () => void },
): Promise<void> {
  try {
    const value = await load();
    if (!signal.aborted) callbacks.success(value);
  } catch (error) {
    if (!signal.aborted) callbacks.failure(error);
  } finally {
    if (!signal.aborted) callbacks.settled();
  }
}
