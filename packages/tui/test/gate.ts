export interface Gate<T> {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
}
export const gate = <T>(): Gate<T> => {
  const holder: { settle?: (value: T) => void } = {};
  const promise = new Promise<T>((resolvePromise) => {
    holder.settle = resolvePromise;
  });
  return { promise, resolve: (value) => holder.settle?.(value) };
};
