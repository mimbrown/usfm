import { writeFileSync } from "node:fs";
import { configurationSchema } from "./client/types/configurationSchema.ts";
import { toJSONSchema } from "zod";

writeFileSync(
  "configuration-schema.json",
  JSON.stringify(
    toJSONSchema(configurationSchema, {
      target: "draft-7",
    }),
    null,
    2
  )
);
