import { Router } from "express";

import { validate } from "../../middleware/validate.js";
import { createHost, deleteHost, getHost, getHostSecret, listHosts, setHostSecret, updateHost } from "./hosts.controller.js";
import { createHostSchema, hostIdParamSchema, listHostsSchema, setHostSecretSchema, updateHostSchema } from "./hosts.schema.js";

// `mergeParams: true` — lihat komentar sama di groups.routes.ts,
// alasannya identik (router ini di-mount di path yang punya `:vaultId`).
export const hostsRouter = Router({ mergeParams: true });

hostsRouter.get("/", validate(listHostsSchema), listHosts);
hostsRouter.post("/", validate(createHostSchema), createHost);
hostsRouter.get("/:id", validate(hostIdParamSchema), getHost);
hostsRouter.put("/:id", validate(updateHostSchema), updateHost);
hostsRouter.delete("/:id", validate(hostIdParamSchema), deleteHost);
hostsRouter.put("/:id/secret", validate(setHostSecretSchema), setHostSecret);
hostsRouter.get("/:id/secret", validate(hostIdParamSchema), getHostSecret);
