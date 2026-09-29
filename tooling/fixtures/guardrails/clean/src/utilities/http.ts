// The one http client. Bare fetch is legitimate in this file only.
export const get = (u: string) => fetch(u);
