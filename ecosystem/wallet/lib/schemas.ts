import { z } from "zod";
import { identityFromMnemonic, parseL2Address } from "@/lib/enwallet";

const seedWord = /^[a-z]+$/i;

export const seedImportSchema = z.object({
  seedPhrase: z
    .string()
    .trim()
    .transform((value) => value.split(/\s+/).filter(Boolean))
    .pipe(
      z
        .array(z.string().regex(seedWord, "Seed words can contain English BIP39 letters only."))
        .refine((words) => words.length === 24, "Enter a 24-word EnWallet seed phrase.")
    )
    .transform((words) => words.join(" "))
    .refine((seedPhrase) => {
      try {
        identityFromMnemonic(seedPhrase);
        return true;
      } catch {
        return false;
      }
    }, "Enter a valid BIP39 seed phrase.")
});

export const sendTransferSchema = z.object({
  recipient: z
    .string()
    .trim()
    .refine((value) => {
      try {
        parseL2Address(value);
        return true;
      } catch {
        return false;
      }
    }, "Use a valid EX or raw 8: address."),
  amount: z
    .string()
    .trim()
    .regex(/^\d+(\.\d{1,9})?$/, "Use a positive amount with up to 9 decimals.")
    .refine((value) => Number(value) > 0, "Amount must be greater than zero."),
  memo: z.string().trim().max(120, "Memo must be 120 characters or fewer.").optional()
});
