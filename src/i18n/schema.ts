/**
 * i18n key-parity schema check.
 *
 * This file exists solely as a TypeScript compile-time check that the
 * Chinese (`zh.ts`) and English (`en.ts`) translation objects share
 * EXACTLY the same key tree. It is never imported by runtime code, so
 * vite will not bundle it into the production build; the tsconfig
 * `include: ["src"]` still picks it up so `tsc --noEmit` will report
 * any drift as a TS2322 type error.
 *
 * Add a new key? Update both languages — tsc tells you which side is
 * missing before the PR can even build.
 */
import zh from "./zh";
import en from "./en";

type Join<K, P> = K extends string
  ? P extends string
    ? P extends ""
      ? K
      : `${P}.${K}`
    : never
  : never;

type Leaves<T, P extends string = ""> = T extends Record<string, unknown>
  ? { [K in keyof T & string]: Leaves<T[K], Join<K, P>> }[keyof T & string]
  : P;

type ZhLeaves = Leaves<typeof zh>;
type EnLeaves = Leaves<typeof en>;

type MissingInEn = Exclude<ZhLeaves, EnLeaves>;
type MissingInZh = Exclude<EnLeaves, ZhLeaves>;

// If either side is missing keys, the constants below get assigned an
// object literal (with the offending key union in its `keys` field)
// instead of `true`, triggering a TS2322 type error at build time.
const _checkMissingInEn: MissingInEn extends never
  ? true
  : { msg: "Missing keys in en.ts"; keys: MissingInEn } = true;
const _checkMissingInZh: MissingInZh extends never
  ? true
  : { msg: "Missing keys in zh.ts"; keys: MissingInZh } = true;

void _checkMissingInEn;
void _checkMissingInZh;