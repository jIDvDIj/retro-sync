/** Converte todas as folhas de `T` para `string` — usada para tipar traduções
 *  não-inglesas e garantir paridade de chaves via TypeScript. */
export type Localized<T> = {
  [K in keyof T]: T[K] extends string ? string : Localized<T[K]>;
};
