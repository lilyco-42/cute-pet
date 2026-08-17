// cute_pet_host NAPI 类型声明
export const petStart: (surfaceId: bigint, width: number, height: number) => number;
export const petStop: () => number;
export const petResize: (width: number, height: number) => number;
export const petTouch: (x: number, y: number, touchId: number, down: boolean) => number;
