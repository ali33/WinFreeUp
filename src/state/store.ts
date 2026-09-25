export interface Store<S, A> {
  getState(): S;
  dispatch(a: A): void;
  subscribe(listener: () => void): () => void;
}

export function createStore<S, A>(reducer: (s: S, a: A) => S, initial: S): Store<S, A> {
  let state = initial;
  const listeners = new Set<() => void>();
  return {
    getState: () => state,
    dispatch(a) {
      const next = reducer(state, a);
      if (next !== state) {
        state = next;
        listeners.forEach((l) => l());
      }
    },
    subscribe(l) {
      listeners.add(l);
      return () => {
        listeners.delete(l);
      };
    },
  };
}
