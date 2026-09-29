// #927: Pagination helpers for async iteration
export async function* iterateOpenBounties(sdk: any, pageSize: number = 10) {
  let offset = 0;
  while (true) {
    const page = await sdk.bounty.list({ offset, limit: pageSize });
    if (!page.length) break;
    for (const bounty of page) yield bounty;
    offset += pageSize;
  }
}
