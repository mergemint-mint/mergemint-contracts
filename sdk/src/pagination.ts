// #927: Pagination helpers for async iteration
// #1285: Cursor-based pagination contract shared with the React SDK.
// Pages are fetched with an opaque `cursor` (null for the first page) and
// return `{ items, nextCursor }` where `nextCursor === null` marks the end.
export interface CursorPage<T> {
  items: T[];
  nextCursor: string | null;
}

export type CursorFetcher<T> = (
  cursor: string | null,
  limit: number,
) => Promise<CursorPage<T>>;

export async function* iterateOpenBounties(sdk: any, pageSize: number = 10) {
  let cursor: string | null = null;
  while (true) {
    const page: CursorPage<any> = await sdk.bounty.list(cursor, pageSize);
    if (!page.items.length) break;
    for (const bounty of page.items) yield bounty;
    if (page.nextCursor === null) break;
    cursor = page.nextCursor;
  }
}
