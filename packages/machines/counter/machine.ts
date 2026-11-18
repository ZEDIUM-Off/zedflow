import { createMachine } from 'xstate';
export default createMachine({
  id: 'counter',
  version: '1',
  context: { count: 0 },
  initial: 'ready',
  states: {
    ready: {
      on: {
        INCREMENT: ({ context }) => ({ context: { count: context.count + 1 } }),
        RESET: () => ({ context: { count: 0 } }),
      },
    },
  },
});
