export const searchErrorReasons = [
  'null-prototype',
  'throwing-conversion',
  'throwing-message',
  'message-object',
  'message-function',
  'ordinary-value',
  'ordinary-error',
] as const;
export type SearchErrorReason = (typeof searchErrorReasons)[number];

const convertibleValue = (observe: (event: string) => void): { toString: () => string } => ({
  toString: (): string => {
    observe('convert');
    return 'caller-owned text';
  },
});

const failConversion = (): never => {
  throw Object.create(null);
};

const messageError = (observe: (event: string) => void, get: () => unknown): Error =>
  Object.defineProperty(new Error(), 'message', {
    get: () => {
      observe('message');
      return get();
    },
  });

const reasonValue = (name: SearchErrorReason, observe: (event: string) => void): unknown => {
  switch (name) {
    case 'null-prototype':
      return Object.create(null);
    case 'throwing-conversion':
      return {
        [Symbol.toPrimitive]: () => {
          observe('convert');
          return failConversion();
        },
      };
    case 'throwing-message':
      return messageError(observe, failConversion);
    case 'message-object':
      return messageError(observe, () => convertibleValue(observe));
    case 'message-function':
      return messageError(observe, () => Object.assign(() => {}, convertibleValue(observe)));
    case 'ordinary-value':
      return convertibleValue(observe);
    case 'ordinary-error':
      return messageError(observe, () => 'caller-owned text');
  }
};

/** Owns caller-realm observation counts without ever transporting the original value. */
export class SearchErrorReasonProbe {
  public readonly observations: string[] = [];
  public readonly value: unknown;
  public readonly expected: string;
  readonly #name: SearchErrorReason;

  public constructor(name: SearchErrorReason) {
    this.#name = name;
    this.value = reasonValue(name, (event) => this.observations.push(event));
    this.expected = ['null-prototype', 'throwing-conversion', 'throwing-message'].includes(name)
      ? 'Could not convert error to text'
      : 'caller-owned text';
  }

  public expectedObservations(): string[] {
    const events: Record<SearchErrorReason, string[]> = {
      'null-prototype': [],
      'throwing-conversion': ['convert'],
      'throwing-message': ['message'],
      'message-object': ['message', 'convert'],
      'message-function': ['message', 'convert'],
      'ordinary-value': ['convert'],
      'ordinary-error': ['message'],
    };
    return events[this.#name];
  }
}
