import { Configuration, FrontendApi, IdentityApi } from "@ory/client"

export const ory = new FrontendApi(
  new Configuration({
    // next.config.mjs'deki rewrite'a yönlendirir (Gateway / Kratos)
    basePath: "/api/auth",
    baseOptions: {
      withCredentials: true, // Kratos cookie'lerinin iletilmesi için şart
    },
  })
)

export const oryAdmin = new IdentityApi(
  new Configuration({
    basePath: process.env.KRATOS_ADMIN_URL || "http://127.0.0.1:4434"
  })
)

import { PermissionApi, RelationshipApi } from "@ory/client"

export const ketoRead = new PermissionApi(
  new Configuration({
    basePath: process.env.KETO_READ_URL || "http://127.0.0.1:4466"
  })
)

export const ketoWrite = new RelationshipApi(
  new Configuration({
    basePath: process.env.KETO_WRITE_URL || "http://127.0.0.1:4467"
  })
)

export const ketoRelationRead = new RelationshipApi(
  new Configuration({
    basePath: process.env.KETO_READ_URL || "http://127.0.0.1:4466"
  })
)
