import { Namespace, Context } from "@ory/keto-namespace-types"

class User implements Namespace {}

class Workspace implements Namespace {
  related: {
    owners: User[]
    members: User[]
  }

  permits = {
    view: (ctx: Context): boolean =>
      this.related.owners.includes(ctx.subject) ||
      this.related.members.includes(ctx.subject),
    edit: (ctx: Context): boolean =>
      this.related.owners.includes(ctx.subject),
  }
}

class Video implements Namespace {
  related: {
    viewers: User[]
    editors: User[]
    parents: Workspace[]
  }

  permits = {
    view: (ctx: Context): boolean =>
      this.related.viewers.includes(ctx.subject) ||
      this.related.editors.includes(ctx.subject) ||
      this.related.parents.traverse((p) => p.permits.view(ctx)),
    edit: (ctx: Context): boolean =>
      this.related.editors.includes(ctx.subject) ||
      this.related.parents.traverse((p) => p.permits.edit(ctx)),
  }
}
