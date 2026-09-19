export function debounce(fn: () => any, timeout: number) {
  let timerId: ReturnType<typeof setTimeout>;
  return () => {
    if (timerId) {
      clearTimeout(timerId);
    }
    timerId = setTimeout(fn, timeout);
  }
}
