import { z } from "zod";

const seedWord = /^[a-z]+$/i;

export const seedImportSchema = z.object({
  seedPhrase: z
    .string()
    .trim()
    .transform((value) => value.split(/\s+/).filter(Boolean))
    .pipe(
      z
        .array(z.string().regex(seedWord, "Seed words can contain letters only in this mock UI."))
        .refine((words) => words.length === 12 || words.length === 24, "Enter a mock 12 or 24 word seed phrase.")
    )
});

export const sendTransferSchema = z.object({
  recipient: z
    .string()
    .trim()
    .min(16, "Recipient address is too short.")
    .max(80, "Recipient address is too long.")
    .refine((value) => value.startsWith("EX") || value.startsWith("8:"), "Use an EX or raw 8: address."),
  amount: z
    .string()
    .trim()
    .regex(/^\d+(\.\d{1,6})?$/, "Use a positive amount with up to 6 decimals.")
    .refine((value) => Number(value) > 0, "Amount must be greater than zero."),
  memo: z.string().trim().max(120, "Memo must be 120 characters or fewer.").optional()
});
