export type AppCollection = {
  id: string
  cartId: string
  currencyCode: string
  amount: number
  sessionId?: string
}

export type AppSession = {
  id: string
  collectionId: string
  cartId: string
  connector: string
  data: Record<string, unknown>
  status: string
  connectorTransactionId?: string
  minorAmount?: number
  currency?: string
}

const collections = new Map<string, AppCollection>()
const sessions = new Map<string, AppSession>()
const cartToSession = new Map<string, string>()

export function createCollection(
  cartId: string,
  currencyCode: string,
  amount: number
): AppCollection {
  const id = `pc_${Date.now()}`
  const collection: AppCollection = { id, cartId, currencyCode, amount }
  collections.set(id, collection)
  return collection
}

export function getCollection(id: string): AppCollection | undefined {
  return collections.get(id)
}

export function createSession(
  opts: Omit<AppSession, "id">
): AppSession {
  const id = `payses_${Date.now()}`
  const session: AppSession = { id, ...opts }
  sessions.set(id, session)
  cartToSession.set(opts.cartId, id)
  const collection = collections.get(opts.collectionId)
  if (collection) {
    collection.sessionId = id
    collections.set(opts.collectionId, collection)
  }
  return session
}

export function getSession(id: string): AppSession | undefined {
  return sessions.get(id)
}

export function getSessionByCart(cartId: string): AppSession | undefined {
  const sessionId = cartToSession.get(cartId)
  return sessionId ? sessions.get(sessionId) : undefined
}

export function updateSession(
  id: string,
  patch: Partial<AppSession>
): AppSession | undefined {
  const session = sessions.get(id)
  if (!session) return undefined
  const updated: AppSession = { ...session, ...patch }
  sessions.set(id, updated)
  return updated
}
