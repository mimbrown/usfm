import * as z from "zod";

export const previewSchema = z.object({
  title: z.string(),
  includes: z.array(z.string()).optional(),
  style: z.string().optional(),
  replacements: z.array(z.string()).optional(),
  stylesheets: z.array(z.string()).optional(),
  diglot: z
    .object({
      path: z.string(),
      style: z.string().optional(),
      replacements: z.array(z.string()).optional(),
    })
    .optional(),
});

export type Preview = z.infer<typeof previewSchema>;

export const configurationSchema = z
  .object({
    lexicon: z
      .object({
        type: z.literal("sqlite"),
        path: z.url(),
      })
      .optional()
      .meta({
        description: "Where the lexicon information for the project is stored",
      }),
    previews: z.array(previewSchema).optional().meta({
      description: "The various kinds of previews available",
    }),
  })
  .meta({
    title: "USFM Configuration",
    description: "USFM Configuration File",
  });

export type Configuration = z.infer<typeof configurationSchema>;
