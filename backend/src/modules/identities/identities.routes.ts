import { Router } from "express";

import { validate } from "../../middleware/validate.js";
import { createIdentity, deleteIdentity, getIdentity, getIdentitySecret, listIdentities, updateIdentity } from "./identities.controller.js";
import { createIdentitySchema, identityIdParamSchema, listIdentitiesSchema, updateIdentitySchema } from "./identities.schema.js";

// `mergeParams: true` — lihat komentar sama di groups.routes.ts.
export const identitiesRouter = Router({ mergeParams: true });

identitiesRouter.get("/", validate(listIdentitiesSchema), listIdentities);
identitiesRouter.post("/", validate(createIdentitySchema), createIdentity);
identitiesRouter.get("/:id", validate(identityIdParamSchema), getIdentity);
identitiesRouter.put("/:id", validate(updateIdentitySchema), updateIdentity);
identitiesRouter.delete("/:id", validate(identityIdParamSchema), deleteIdentity);
identitiesRouter.get("/:id/secret", validate(identityIdParamSchema), getIdentitySecret);
