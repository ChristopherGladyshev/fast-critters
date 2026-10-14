export function empty<T>(
  value: T | null | undefined
): value is null | undefined {
  // false, null, undefined, 0, ''
  if (!value) return true;

  // считаем что массив пустой, если в нем нет элементов ([])
  if (Array.isArray(value)) return value.length === 0;

  // считаем что объект пустой, если в нем нет свойств ({})
  if (typeof value === 'object') return Object.keys(value).length === 0;

  // в остальных случаях посто приводим к boolean
  // 'foo', 100, -100
  return !value;
}

export function notEmpty<T>(value: T | null | undefined): value is T {
  return !empty(value);
}

export function firstNotEmpty<T>(...values: T[]): T | undefined {
  return values.find((value) => !empty(value));
}
