import { Router } from "express";

import { validate } from "../../middleware/validate.js";
import { createGroup, deleteGroup, getGroup, listGroups, updateGroup } from "./groups.controller.js";
import {
  createGroupSchema,
  groupIdParamSchema,
  listGroupsSchema,
  updateGroupSchema,
} from "./groups.schema.js";

// `mergeParams: true` WAJIB — router ini di-mount di app.ts pada path
// yang punya `:vaultId` (mis. `/api/v1/vaults/:vaultId/groups`), tanpa
// ini `req.params.vaultId` TIDAK KELIHATAN di dalam sini (Express
// tidak otomatis meneruskan param parent ke sub-router).
export const groupsRouter = Router({ mergeParams: true });

groupsRouter.get("/", validate(listGroupsSchema), listGroups);
groupsRouter.post("/", validate(createGroupSchema), createGroup);
groupsRouter.get("/:id", validate(groupIdParamSchema), getGroup);
groupsRouter.put("/:id", validate(updateGroupSchema), updateGroup);
groupsRouter.delete("/:id", validate(groupIdParamSchema), deleteGroup);
